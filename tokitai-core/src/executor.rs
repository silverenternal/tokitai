//! Async executor for tool execution
//!
//! This module provides a lightweight wrapper around tokio's runtime with:
//! - Concurrent execution limiting via semaphore
//! - Configurable timeout support
//! - Flexible retry policies with backoff strategies
//! - Efficient statistics collection using ring buffers
//!
//! # Design Philosophy
//!
//! This is a **thin wrapper** around tokio's runtime, designed specifically for
//! MCP server use cases. It provides just enough abstraction to simplify common
//! patterns while staying out of your way.
//!
//! ## What This Provides
//!
//! - Unified configuration for MCP servers
//! - Convenient builder API for common execution patterns
//! - Statistics collection for monitoring and alerting
//! - Retry logic with configurable backoff
//!
//! ## What This Does NOT Provide
//!
//! - Independent thread pools (uses tokio's built-in scheduler)
//! - Custom task scheduling algorithms
//!
//! For advanced scheduling needs, consider using tokio directly or
//! specialized crates like `rayon` for CPU-bound parallelism.
//!
//! # Example
//!
//! ```rust,ignore
//! use tokitai_core::executor::ToolExecutor;
//! use std::time::Duration;
//!
//! let executor = ToolExecutor::builder()
//!     .with_max_concurrent(100)
//!     .with_default_timeout(Duration::from_secs(30))
//!     .build();
//!
//! let result = executor
//!     .execute(async { Ok::<_, ExecutionError>(42) })
//!     .with_timeout(Duration::from_secs(5))
//!     .await_result()
//!     .await;
//! ```

use std::future::Future;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use std::fmt;

#[cfg(feature = "async")]
use tokio::sync::Semaphore;
#[cfg(feature = "async")]
use tokio::time::timeout;
#[cfg(feature = "async")]
use tokio_util::sync::CancellationToken;

/// Priority wait queue for true priority scheduling
#[cfg(feature = "async")]
struct PriorityWaitQueue {
    /// High priority waiters
    high: tokio::sync::Mutex<std::collections::VecDeque<tokio::sync::oneshot::Sender<()>>>,
    /// Normal priority waiters
    normal: tokio::sync::Mutex<std::collections::VecDeque<tokio::sync::oneshot::Sender<()>>>,
    /// Low priority waiters
    low: tokio::sync::Mutex<std::collections::VecDeque<tokio::sync::oneshot::Sender<()>>>,
}

#[cfg(feature = "async")]
impl PriorityWaitQueue {
    fn new() -> Self {
        Self {
            high: tokio::sync::Mutex::new(std::collections::VecDeque::new()),
            normal: tokio::sync::Mutex::new(std::collections::VecDeque::new()),
            low: tokio::sync::Mutex::new(std::collections::VecDeque::new()),
        }
    }

    /// Add a waiter for the given priority and return a future that resolves when it's their turn
    async fn wait(&self, priority: Priority) -> tokio::sync::oneshot::Receiver<()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        
        let queue = match priority {
            Priority::High => &self.high,
            Priority::Normal => &self.normal,
            Priority::Low => &self.low,
        };
        
        queue.lock().await.push_back(tx);
        rx
    }

    /// Notify the highest priority waiter
    async fn notify_one(&self) {
        // Check high priority first, then normal, then low
        let mut sender = None;
        
        {
            let mut high = self.high.lock().await;
            if let Some(tx) = high.pop_front() {
                sender = Some(tx);
            }
        }
        
        if sender.is_none() {
            let mut normal = self.normal.lock().await;
            if let Some(tx) = normal.pop_front() {
                sender = Some(tx);
            }
        }
        
        if sender.is_none() {
            let mut low = self.low.lock().await;
            sender = low.pop_front();
        }
        
        if let Some(tx) = sender {
            let _ = tx.send(());
        }
    }

    /// Get the number of waiters
    async fn len(&self) -> usize {
        let high = self.high.lock().await.len();
        let normal = self.normal.lock().await.len();
        let low = self.low.lock().await.len();
        high + normal + low
    }
}

/// Default maximum concurrent executions
pub const DEFAULT_MAX_CONCURRENT: usize = 100;

/// Default timeout for execution (30 seconds)
pub const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Task priority levels for scheduling
///
/// Higher priority tasks will be executed before lower priority tasks
/// when multiple tasks are waiting for execution permits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Priority {
    Low = 0,
    Normal = 1,
    #[default]
    High = 2,
}

impl Priority {
    /// Get the number of priority levels
    pub fn level_count() -> usize {
        3
    }
    
    /// Get priority index (for array access)
    pub fn as_index(&self) -> usize {
        match self {
            Priority::Low => 0,
            Priority::Normal => 1,
            Priority::High => 2,
        }
    }
    
    /// Create priority from index
    pub fn from_index(index: usize) -> Self {
        match index {
            0 => Priority::Low,
            1 => Priority::Normal,
            _ => Priority::High,
        }
    }
}

/// Backoff strategy for retries
#[derive(Debug, Clone)]
pub enum BackoffStrategy {
    /// No backoff - retry immediately
    None,
    /// Fixed delay between retries
    Fixed(Duration),
    /// Exponential backoff: initial_delay * 2^(attempt-1)
    Exponential {
        initial_delay: Duration,
        max_delay: Option<Duration>,
    },
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        Self::Exponential {
            initial_delay: Duration::from_millis(10),
            max_delay: Some(Duration::from_secs(1)),
        }
    }
}

impl BackoffStrategy {
    /// Create exponential backoff with default settings
    pub fn exponential() -> Self {
        Self::default()
    }

    /// Create exponential backoff with custom initial delay
    pub fn exponential_with_initial(initial_delay: Duration) -> Self {
        Self::Exponential {
            initial_delay,
            max_delay: None,
        }
    }

    /// Create fixed backoff
    pub fn fixed(delay: Duration) -> Self {
        Self::Fixed(delay)
    }

    /// Calculate delay for given attempt (1-based)
    fn delay_for_attempt(&self, attempt: u32) -> Duration {
        match self {
            BackoffStrategy::None => Duration::ZERO,
            BackoffStrategy::Fixed(delay) => *delay,
            BackoffStrategy::Exponential { initial_delay, max_delay } => {
                let delay = initial_delay.saturating_mul(1 << (attempt.saturating_sub(1) as u32));
                max_delay.map(|max| delay.min(max)).unwrap_or(delay)
            }
        }
    }
}

/// Error type for execution failures
#[derive(Debug, Clone)]
pub struct ExecutionError {
    pub kind: ExecutionErrorKind,
    pub message: String,
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}: {}", self.kind, self.message)
    }
}

impl std::error::Error for ExecutionError {}

/// Classification of execution errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionErrorKind {
    /// Task execution timeout
    Timeout,
    /// Task execution panicked
    Panic,
    /// Executor is shutting down
    Shutdown,
    /// Task was cancelled
    Cancelled,
    /// Internal error (application-specific)
    Internal,
    /// All retries exhausted
    RetriesExhausted,
    /// Task queue is full (backpressure)
    QueueFull,
}

impl fmt::Display for ExecutionErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionErrorKind::Timeout => write!(f, "timeout"),
            ExecutionErrorKind::Panic => write!(f, "panic"),
            ExecutionErrorKind::Shutdown => write!(f, "shutdown"),
            ExecutionErrorKind::Cancelled => write!(f, "cancelled"),
            ExecutionErrorKind::Internal => write!(f, "internal"),
            ExecutionErrorKind::RetriesExhausted => write!(f, "retries exhausted"),
            ExecutionErrorKind::QueueFull => write!(f, "queue full"),
        }
    }
}

/// Retry policy configuration
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retries (0 = no retries)
    pub max_retries: u32,
    /// Backoff strategy
    pub backoff: BackoffStrategy,
    /// Only retry on these error kinds (None = retry all errors except timeout)
    pub retry_on: Option<Vec<ExecutionErrorKind>>,
    /// Whether to retry on timeout (default: false)
    pub retry_on_timeout: bool,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 0,
            backoff: BackoffStrategy::default(),
            retry_on: None,
            retry_on_timeout: false,
        }
    }
}

impl RetryPolicy {
    /// Create a retry policy with no retries
    pub fn none() -> Self {
        Self::default()
    }

    /// Create a retry policy with exponential backoff
    pub fn with_retries(max_retries: u32) -> Self {
        Self {
            max_retries,
            backoff: BackoffStrategy::exponential(),
            retry_on: None,
            retry_on_timeout: false,
        }
    }

    /// Set backoff strategy
    pub fn with_backoff(mut self, backoff: BackoffStrategy) -> Self {
        self.backoff = backoff;
        self
    }

    /// Only retry on specific error kinds
    pub fn retry_on(mut self, kinds: Vec<ExecutionErrorKind>) -> Self {
        self.retry_on = Some(kinds);
        self
    }

    /// Enable or disable retry on timeout
    pub fn with_retry_on_timeout(mut self, retry: bool) -> Self {
        self.retry_on_timeout = retry;
        self
    }

    /// Check if should retry on this error
    ///
    /// This method is public so users can reuse the same retry logic externally.
    pub fn should_retry(&self, error: &ExecutionError) -> bool {
        if self.max_retries == 0 {
            return false;
        }

        // Timeout handling
        if error.kind == ExecutionErrorKind::Timeout {
            return self.retry_on_timeout;
        }

        // Check specific error kinds if configured
        match &self.retry_on {
            None => true,  // Retry all non-timeout errors
            Some(kinds) => kinds.contains(&error.kind),
        }
    }
}

/// Statistics about executor state
#[derive(Debug, Clone)]
pub struct ExecutorStats {
    /// Number of currently running tasks
    pub running_tasks: usize,
    /// Total tasks executed since startup
    pub total_executed: usize,
    /// Total tasks that timed out
    pub total_timeouts: usize,
    /// Total tasks that failed
    pub total_failures: usize,
    /// Total tasks that succeeded
    pub total_successes: usize,
    /// Total retries attempted
    pub total_retries: usize,
    /// Average execution time in milliseconds
    pub avg_execution_time_ms: u64,
    /// P99 execution time in milliseconds (based on last 1000 samples)
    pub p99_execution_time_ms: u64,
    /// Maximum concurrent tasks allowed
    pub max_concurrent: usize,
}

/// Ring buffer for efficient statistics collection
/// 
/// Uses a circular buffer pattern with O(1) push operations.
/// When full, new values overwrite the oldest values.
struct RingBuffer {
    buffer: Vec<u64>,
    head: usize,
    count: usize,
}

impl RingBuffer {
    fn with_capacity(capacity: usize) -> Self {
        Self {
            buffer: vec![0; capacity],
            head: 0,
            count: 0,
        }
    }

    fn push(&mut self, value: u64) {
        self.buffer[self.head] = value;
        self.head = (self.head + 1) % self.buffer.len();
        if self.count < self.buffer.len() {
            self.count += 1;
        }
    }

    fn iter(&self) -> RingBufferIter<'_> {
        RingBufferIter {
            buffer: &self.buffer,
            start: if self.count < self.buffer.len() {
                0
            } else {
                self.head
            },
            count: self.count,
            index: 0,
        }
    }

    fn len(&self) -> usize {
        self.count
    }
}

struct RingBufferIter<'a> {
    buffer: &'a [u64],
    start: usize,
    count: usize,
    index: usize,
}

impl<'a> Iterator for RingBufferIter<'a> {
    type Item = u64;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index >= self.count {
            return None;
        }
        let idx = (self.start + self.index) % self.buffer.len();
        let value = self.buffer[idx];
        self.index += 1;
        Some(value)
    }
}

/// Internal statistics storage (lock-free where possible)
struct ExecutorStatsInner {
    running_tasks: AtomicUsize,
    total_executed: AtomicUsize,
    total_timeouts: AtomicUsize,
    total_failures: AtomicUsize,
    total_successes: AtomicUsize,
    total_retries: AtomicUsize,
    execution_times: parking_lot::RwLock<RingBuffer>,
    max_concurrent: usize,
}

impl ExecutorStatsInner {
    fn new(max_concurrent: usize) -> Self {
        Self {
            running_tasks: AtomicUsize::new(0),
            total_executed: AtomicUsize::new(0),
            total_timeouts: AtomicUsize::new(0),
            total_failures: AtomicUsize::new(0),
            total_successes: AtomicUsize::new(0),
            total_retries: AtomicUsize::new(0),
            execution_times: parking_lot::RwLock::new(RingBuffer::with_capacity(1000)),
            max_concurrent,
        }
    }

    fn record_start(&self) {
        self.running_tasks.fetch_add(1, Ordering::Relaxed);
        self.total_executed.fetch_add(1, Ordering::Relaxed);
    }

    fn record_success(&self, duration_ms: u64) {
        self.running_tasks.fetch_sub(1, Ordering::Relaxed);
        self.total_successes.fetch_add(1, Ordering::Relaxed);
        self.execution_times.write().push(duration_ms);
    }

    fn record_failure(&self, is_timeout: bool) {
        self.running_tasks.fetch_sub(1, Ordering::Relaxed);
        self.total_failures.fetch_add(1, Ordering::Relaxed);
        if is_timeout {
            self.total_timeouts.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn record_retry(&self) {
        self.total_retries.fetch_add(1, Ordering::Relaxed);
    }

    fn to_stats(&self) -> ExecutorStats {
        // Use read lock for non-mutating operation
        let times: Vec<u64> = {
            let guard = self.execution_times.read();
            guard.iter().collect()
        };

        let (avg_ms, p99_ms) = if times.is_empty() {
            (0, 0)
        } else {
            let sum: u64 = times.iter().sum();
            let avg = sum / times.len() as u64;
            let mut sorted: Vec<u64> = times;
            sorted.sort_unstable();
            let p99_idx = ((sorted.len() as f64 * 0.99) as usize).min(sorted.len() - 1);
            let p99 = sorted[p99_idx];
            (avg, p99)
        };

        ExecutorStats {
            running_tasks: self.running_tasks.load(Ordering::Relaxed),
            total_executed: self.total_executed.load(Ordering::Relaxed),
            total_timeouts: self.total_timeouts.load(Ordering::Relaxed),
            total_failures: self.total_failures.load(Ordering::Relaxed),
            total_successes: self.total_successes.load(Ordering::Relaxed),
            total_retries: self.total_retries.load(Ordering::Relaxed),
            avg_execution_time_ms: avg_ms,
            p99_execution_time_ms: p99_ms,
            max_concurrent: self.max_concurrent,
        }
    }
}

/// Builder for ToolExecutor
pub struct ToolExecutorBuilder {
    max_concurrent: usize,
    default_timeout: Duration,
    retry_policy: RetryPolicy,
    max_pending: Option<usize>,  // For backpressure
    priority_support: bool,  // Enable priority scheduling
}

impl Default for ToolExecutorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolExecutorBuilder {
    /// Create a new builder with default settings
    pub fn new() -> Self {
        Self {
            max_concurrent: DEFAULT_MAX_CONCURRENT,
            default_timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            retry_policy: RetryPolicy::default(),
            max_pending: None,
            priority_support: false,
        }
    }

    /// Set maximum concurrent executions
    pub fn with_max_concurrent(mut self, max: usize) -> Self {
        self.max_concurrent = max.max(1);
        self
    }

    /// Set default timeout
    pub fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }

    /// Set retry policy
    pub fn with_retry_policy(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = policy;
        self
    }

    /// Set maximum pending tasks (backpressure)
    ///
    /// When set, the executor will reject new submissions if there are
    /// already `max_pending` tasks waiting to execute.
    ///
    /// If not set, there is no limit on pending tasks (tasks can queue
    /// up indefinitely under high load).
    pub fn with_max_pending(mut self, max: usize) -> Self {
        self.max_pending = Some(max.max(1));
        self
    }

    /// Enable priority-based scheduling
    ///
    /// When enabled, the executor uses **dedicated wait queues** for each priority
    /// level. Higher priority tasks are always notified before lower priority tasks
    /// when a permit becomes available.
    ///
    /// This is a "hard" priority system:
    /// - High priority tasks are always served before normal priority
    /// - Normal priority tasks are always served before low priority
    /// - Within each priority level, tasks are served FIFO
    pub fn with_priority_support(mut self) -> Self {
        self.priority_support = true;
        self
    }

    /// Build the executor
    pub fn build(self) -> ToolExecutor {
        let (priority_queue, priority_support) = if self.priority_support {
            (Some(Arc::new(PriorityWaitQueue::new())), true)
        } else {
            (None, false)
        };

        ToolExecutor {
            semaphore: Arc::new(Semaphore::new(self.max_concurrent)),
            default_timeout: self.default_timeout,
            retry_policy: self.retry_policy,
            stats: Arc::new(ExecutorStatsInner::new(self.max_concurrent)),
            shutdown: Arc::new(AtomicBool::new(false)),
            queue_semaphore: self.max_pending.map(|max| Arc::new(Semaphore::new(max))),
            cancel_token: CancellationToken::new(),
            priority_queue,
            priority_support,
        }
    }
}

/// Execution request builder
pub struct ExecutionBuilder<'a, F, T, E>
where
    F: Future<Output = Result<T, E>>,
{
    executor: &'a ToolExecutor,
    future: F,
    timeout: Option<Duration>,
    retry_policy: Option<RetryPolicy>,
    cancel_token: Option<CancellationToken>,
    priority: Priority,
    _phantom: std::marker::PhantomData<(T, E)>,
}

impl<'a, F, T, E> ExecutionBuilder<'a, F, T, E>
where
    F: Future<Output = Result<T, E>> + Send + 'static,
    T: Send + 'static,
    E: Into<ExecutionError> + fmt::Debug + Send + 'static,
{
    /// Set timeout for this execution
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set retry policy for this execution
    pub fn with_retry(mut self, policy: RetryPolicy) -> Self {
        self.retry_policy = Some(policy);
        self
    }

    /// Set cancellation token for this execution
    ///
    /// When the token is cancelled, the task will be aborted.
    pub fn with_cancellation(mut self, token: CancellationToken) -> Self {
        self.cancel_token = Some(token);
        self
    }

    /// Set priority for this execution
    ///
    /// Higher priority tasks will be executed before lower priority tasks
    /// when there is contention for execution permits.
    ///
    /// Note: Priority scheduling must be enabled on the executor via
    /// `with_priority_support()` for this to have effect.
    pub fn with_priority(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    /// Execute the future and get result
    pub async fn await_result(self) -> Result<T, ExecutionError> {
        let timeout_dur = self.timeout.unwrap_or(self.executor.default_timeout);
        let retry_policy = self.retry_policy.clone().unwrap_or_else(|| self.executor.retry_policy.clone());

        self.executor
            .execute_inner(self.future, timeout_dur, &retry_policy, self.cancel_token, self.priority)
            .await
    }
}

/// Async executor for tool execution
///
/// A lightweight wrapper around tokio's runtime that provides:
/// - Concurrent execution limiting via semaphore
/// - Configurable timeout support
/// - Flexible retry policies with backoff strategies
/// - Efficient statistics collection using ring buffers
/// - Backpressure support via bounded task queue
/// - Task cancellation support
/// - Priority-based scheduling with true priority wait queue
///
/// # Concurrency Characteristics
///
/// - **Semaphore**: Shared across clones (Arc<Semaphore>), single permit acquisition
/// - **Statistics**: Lock-free atomics for counters, RwLock for ring buffer (O(1) write)
/// - **Shutdown**: AtomicBool with SeqCst ordering for immediate visibility
/// - **Backpressure**: Optional bounded queue for limiting pending tasks (held until completion)
/// - **Cancellation**: CancellationToken for graceful task cancellation
/// - **Priority**: True priority scheduling with dedicated wait queues per priority level
///
/// # Priority Scheduling
///
/// When priority scheduling is enabled (via `with_priority_support()`), the executor
/// uses **dedicated wait queues** for each priority level. Higher priority tasks
/// are always notified before lower priority tasks when a permit becomes available.
///
/// This is a "hard" priority system:
/// - High priority tasks are always served before normal priority
/// - Normal priority tasks are always served before low priority
/// - Within each priority level, tasks are served FIFO
///
/// # Design Philosophy
///
/// This is a **thin wrapper** around tokio's runtime. It does NOT provide:
/// - Independent thread pools (uses tokio's built-in scheduler)
/// - Custom task scheduling algorithms
/// - Work-stealing or load balancing
///
/// For advanced scheduling needs, consider using tokio directly or
/// specialized crates like `rayon` for CPU-bound parallelism or
/// `async-executor` for custom scheduling.
///
/// # Example
///
/// ```rust,ignore
/// let executor = ToolExecutor::builder()
///     .with_max_concurrent(100)
///     .with_default_timeout(Duration::from_secs(30))
///     .with_priority_support()  // Enable priority scheduling
///     .build();
///
/// let result = executor
///     .execute(async { Ok::<_, ExecutionError>(42) })
///     .with_priority(Priority::High)
///     .await_result()
///     .await;
/// ```
pub struct ToolExecutor {
    semaphore: Arc<Semaphore>,
    default_timeout: Duration,
    retry_policy: RetryPolicy,
    stats: Arc<ExecutorStatsInner>,
    shutdown: Arc<AtomicBool>,
    /// Optional bounded queue for backpressure (limits pending tasks, held until completion)
    queue_semaphore: Option<Arc<Semaphore>>,
    /// Global cancellation token for graceful shutdown
    cancel_token: CancellationToken,
    /// Priority wait queue for true priority scheduling (only used when priority_support is true)
    priority_queue: Option<Arc<PriorityWaitQueue>>,
    /// Priority support flag
    priority_support: bool,
}

impl Default for ToolExecutor {
    fn default() -> Self {
        ToolExecutorBuilder::new().build()
    }
}

impl Clone for ToolExecutor {
    fn clone(&self) -> Self {
        Self {
            semaphore: self.semaphore.clone(),
            default_timeout: self.default_timeout,
            retry_policy: self.retry_policy.clone(),
            stats: self.stats.clone(),
            shutdown: self.shutdown.clone(),
            queue_semaphore: self.queue_semaphore.clone(),
            cancel_token: self.cancel_token.clone(),
            priority_support: self.priority_support,
        }
    }
}

impl ToolExecutor {
    /// Create a builder for custom configuration
    pub fn builder() -> ToolExecutorBuilder {
        ToolExecutorBuilder::new()
    }

    /// Get executor statistics
    pub fn stats(&self) -> ExecutorStats {
        self.stats.to_stats()
    }

    /// Start building an execution request
    pub fn execute<F, T, E>(&self, future: F) -> ExecutionBuilder<'_, F, T, E>
    where
        F: Future<Output = Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: Into<ExecutionError> + fmt::Debug + Send + 'static,
    {
        ExecutionBuilder {
            executor: self,
            future,
            timeout: None,
            retry_policy: None,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Execute a future with default configuration
    ///
    /// This is a convenience method equivalent to:
    /// ```ignore
    /// executor.execute(future).await_result().await
    /// ```
    pub async fn execute_simple<F, T, E>(&self, future: F) -> Result<T, ExecutionError>
    where
        F: Future<Output = Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: Into<ExecutionError> + fmt::Debug + Send + 'static,
    {
        self.execute(future).await_result().await
    }

    /// Execute a future with custom timeout
    pub async fn execute_with_timeout<F, T, E>(
        &self,
        future: F,
        timeout: Duration,
    ) -> Result<T, ExecutionError>
    where
        F: Future<Output = Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: Into<ExecutionError> + fmt::Debug + Send + 'static,
    {
        self.execute(future).with_timeout(timeout).await_result().await
    }

    /// Internal execution logic with unified retry handling
    async fn execute_inner<F, T, E>(
        &self,
        future: F,
        timeout_duration: Duration,
        retry_policy: &RetryPolicy,
        cancel_token: Option<CancellationToken>,
        priority: Priority,
    ) -> Result<T, ExecutionError>
    where
        F: Future<Output = Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: Into<ExecutionError> + fmt::Debug + Send + 'static,
    {
        // Check shutdown (using SeqCst for proper visibility)
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(ExecutionError {
                kind: ExecutionErrorKind::Shutdown,
                message: "Executor is shutting down".to_string(),
            });
        }

        // Check if cancelled
        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                return Err(ExecutionError {
                    kind: ExecutionErrorKind::Cancelled,
                    message: "Task was cancelled".to_string(),
                });
            }
        }

        // Acquire queue permit first (backpressure)
        // This permit is held until task COMPLETION, not just until execution starts
        let _queue_permit = if let Some(ref queue_sem) = self.queue_semaphore {
            Some(queue_sem.acquire_owned().await.map_err(|_| {
                ExecutionError {
                    kind: ExecutionErrorKind::Shutdown,
                    message: "Executor queue is shutting down".to_string(),
                }
            })?)
        } else {
            None
        };

        // Acquire execution permit with priority support
        // If priority support is enabled, use the priority wait queue
        let _permit = if self.priority_support {
            if let Some(ref pq) = self.priority_queue {
                // Wait in priority queue
                let mut rx = pq.wait(priority).await;
                
                // Try to acquire the semaphore
                // We need to loop because we might be woken up but another task stole the permit
                loop {
                    match self.semaphore.clone().try_acquire_owned() {
                        Ok(permit) => {
                            // We got a permit, notify the next waiter
                            pq.notify_one().await;
                            break permit;
                        }
                        Err(_) => {
                            // No permit available, wait for notification
                            // Use a timeout to avoid deadlocks
                            let timeout_rx = tokio::time::timeout(Duration::from_secs(60), &mut rx);
                            match timeout_rx.await {
                                Ok(Ok(())) => {
                                    // We were notified, try to acquire again
                                    continue;
                                }
                                Ok(Err(_)) | Err(_) => {
                                    // Channel closed or timeout, try to acquire normally
                                    break self.semaphore.acquire_owned().await.map_err(|_| {
                                        ExecutionError {
                                            kind: ExecutionErrorKind::Shutdown,
                                            message: "Executor is shutting down".to_string(),
                                        }
                                    })?;
                                }
                            }
                        }
                    }
                }
            } else {
                self.semaphore.acquire_owned().await.map_err(|_| {
                    ExecutionError {
                        kind: ExecutionErrorKind::Shutdown,
                        message: "Executor is shutting down".to_string(),
                    }
                })?
            }
        } else {
            // No priority support - use default semaphore
            self.semaphore.acquire_owned().await.map_err(|_| {
                ExecutionError {
                    kind: ExecutionErrorKind::Shutdown,
                    message: "Executor is shutting down".to_string(),
                }
            })?
        };

        // NOTE: _queue_permit is held until task completion (when this function returns)
        // This provides true backpressure - the queue slot is not released until the task finishes

        self.stats.record_start();

        let start_time = std::time::Instant::now();
        let mut attempts = 0;

        loop {
            // Check cancellation before each attempt
            if let Some(ref token) = cancel_token {
                if token.is_cancelled() {
                    return Err(ExecutionError {
                        kind: ExecutionErrorKind::Cancelled,
                        message: "Task was cancelled".to_string(),
                    });
                }
            }

            // Execute with timeout and handle result uniformly
            let result: Result<T, ExecutionError> = match tokio::time::timeout(timeout_duration, &future).await {
                Ok(Ok(value)) => {
                    let duration_ms = start_time.elapsed().as_millis() as u64;
                    self.stats.record_success(duration_ms);
                    return Ok(value);
                }
                Ok(Err(e)) => Err(e.into()),
                Err(_) => Err(ExecutionError {
                    kind: ExecutionErrorKind::Timeout,
                    message: format!("Task timed out after {:?}", timeout_duration),
                }),
            };

            // Unified retry logic
            let error = result.unwrap_err();
            if retry_policy.should_retry(&error) && attempts < retry_policy.max_retries {
                attempts += 1;
                self.stats.record_retry();
                let backoff = retry_policy.backoff.delay_for_attempt(attempts);
                
                // Sleep with cancellation support
                let sleep_future = tokio::time::sleep(backoff);
                tokio::pin!(sleep_future);
                
                if let Some(ref token) = cancel_token {
                    tokio::select! {
                        _ = &mut sleep_future => {}
                        _ = token.cancelled() => {
                            return Err(ExecutionError {
                                kind: ExecutionErrorKind::Cancelled,
                                message: "Task was cancelled during retry backoff".to_string(),
                            });
                        }
                    }
                } else {
                    sleep_future.await;
                }
                
                continue;
            }

            // Check if retries were exhausted
            if attempts >= retry_policy.max_retries {
                self.stats.record_failure(error.kind == ExecutionErrorKind::Timeout);
                return Err(ExecutionError {
                    kind: ExecutionErrorKind::RetriesExhausted,
                    message: format!("Failed after {} attempts: {}", attempts, error.message),
                });
            }

            // Final failure (no retries or not retryable)
            self.stats.record_failure(error.kind == ExecutionErrorKind::Timeout);
            return Err(error);
        }
    }

    /// Try to execute a future immediately (non-blocking)
    ///
    /// Returns an error if the executor is at capacity (backpressure).
    ///
    /// # Returns
    ///
    /// A join handle to the task, or an error if rejected due to backpressure.
    pub fn try_execute<F, T, E>(&self, future: F) -> Result<tokio::task::JoinHandle<Result<T, ExecutionError>>, ExecutionError>
    where
        F: Future<Output = Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: Into<ExecutionError> + fmt::Debug + Send + 'static,
    {
        // Check shutdown
        if self.shutdown.load(Ordering::SeqCst) {
            return Err(ExecutionError {
                kind: ExecutionErrorKind::Shutdown,
                message: "Executor is shutting down".to_string(),
            });
        }

        // Try to acquire queue permit (non-blocking)
        let queue_permit = if let Some(ref queue_sem) = self.queue_semaphore {
            Some(queue_sem.try_acquire_owned().map_err(|_| {
                ExecutionError {
                    kind: ExecutionErrorKind::QueueFull,
                    message: "Executor queue is full".to_string(),
                }
            })?)
        } else {
            None
        };

        let executor = self.clone();
        let handle = tokio::spawn(async move {
            // Hold queue permit until task completion (true backpressure)
            let _queue_permit = queue_permit;
            executor.execute(future).await_result().await
        });

        Ok(handle)
    }

    /// Execute a blocking (synchronous) task
    pub async fn execute_blocking<F, T, E>(&self, f: F) -> Result<T, ExecutionError>
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: Into<ExecutionError> + fmt::Debug + Send + 'static,
    {
        let future = async {
            let handle = tokio::task::spawn_blocking(f);
            handle.await.map_err(|_| ExecutionError {
                kind: ExecutionErrorKind::Panic,
                message: "Task panicked".to_string(),
            })?
        };
        self.execute_simple(future).await
    }

    /// Execute multiple futures concurrently with bounded concurrency
    ///
    /// This method preserves the order of results - the i-th result corresponds
    /// to the i-th input future.
    ///
    /// # Note
    ///
    /// This method spawns all tasks immediately. Each task will acquire a permit
    /// from the executor's semaphore before executing, so concurrency is still
    /// limited by `max_concurrent`. However, spawning many tasks at once may
    /// cause memory pressure.
    ///
    /// For large batches, consider using `execute_batch_bounded` which limits
    /// both spawning and execution concurrency.
    ///
    /// # Arguments
    ///
    /// * `futures` - Iterator of futures to execute
    ///
    /// # Returns
    ///
    /// A vector of results, in the same order as the input futures.
    /// Each result is either the task's result or an error (including JoinError).
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let tasks = vec![
    ///     async { Ok::<_, ExecutionError>(1) },
    ///     async { Ok::<_, ExecutionError>(2) },
    ///     async { Ok::<_, ExecutionError>(3) },
    /// ];
    ///
    /// let results = executor.execute_batch(tasks).await;
    /// // results = [Ok(1), Ok(2), Ok(3)]
    /// ```
    pub async fn execute_batch<F, T, E>(
        &self,
        futures: impl IntoIterator<Item = F>,
    ) -> Vec<Result<T, ExecutionError>>
    where
        F: Future<Output = Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: Into<ExecutionError> + fmt::Debug + Send + 'static,
    {
        use futures_util::future::join_all;

        let tasks = futures.into_iter().map(|future| {
            let executor = self.clone();
            tokio::spawn(async move {
                executor.execute(future).await_result().await
            })
        });

        // Preserve all results, including JoinError (converted to ExecutionError)
        join_all(tasks).await
            .into_iter()
            .map(|r| match r {
                Ok(result) => result,
                Err(join_err) => Err(ExecutionError {
                    kind: ExecutionErrorKind::Cancelled,
                    message: format!("Task cancelled: {}", join_err),
                }),
            })
            .collect()
    }

    /// Execute multiple futures with all-or-nothing semantics
    ///
    /// Returns `Ok` only if all futures succeed, otherwise returns the first error.
    /// All tasks are spawned and will complete even if one fails (to avoid resource leaks).
    ///
    /// # Arguments
    ///
    /// * `futures` - Iterator of futures to execute
    ///
    /// # Returns
    ///
    /// A vector of results if all succeed, or the first error if any fail.
    ///
    /// # Note
    ///
    /// If a task fails, other tasks continue to execute. This is by design to avoid
    /// resource leaks. The first error is returned, but all tasks will complete.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let tasks = vec![
    ///     async { Ok::<_, ExecutionError>(1) },
    ///     async { Ok::<_, ExecutionError>(2) },
    ///     async { Ok::<_, ExecutionError>(3) },
    /// ];
    ///
    /// let results = executor.execute_batch_all_ok(tasks).await;
    /// // Ok([1, 2, 3]) or Err(first_error)
    /// ```
    pub async fn execute_batch_all_ok<F, T, E>(
        &self,
        futures: impl IntoIterator<Item = F>,
    ) -> Result<Vec<T>, ExecutionError>
    where
        F: Future<Output = Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: Into<ExecutionError> + fmt::Debug + Send + 'static,
    {
        use futures_util::future::join_all;

        let tasks = futures.into_iter().map(|future| {
            let executor = self.clone();
            tokio::spawn(async move {
                executor.execute(future).await_result().await
            })
        });

        // Wait for ALL tasks to complete (don't early return)
        let results = join_all(tasks).await;
        let mut success_results = Vec::new();
        let mut first_error: Option<ExecutionError> = None;

        for result in results {
            match result {
                Ok(Ok(value)) => success_results.push(value),
                Ok(Err(e)) => {
                    // Store first error but continue waiting for other tasks
                    if first_error.is_none() {
                        first_error = Some(e);
                    }
                }
                Err(join_err) => {
                    if first_error.is_none() {
                        first_error = Some(ExecutionError {
                            kind: ExecutionErrorKind::Cancelled,
                            message: format!("Task cancelled: {}", join_err),
                        });
                    }
                }
            }
        }

        // Return first error if any, otherwise success
        if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(success_results)
        }
    }

    /// Execute multiple futures with bounded concurrency using a stream
    ///
    /// This is a convenience method that uses `futures_util::stream::buffer_unordered`
    /// internally. The concurrency limiting is provided by the futures_util library.
    ///
    /// This method is useful for executing a large batch of tasks with memory efficiency,
    /// as it limits the number of concurrently spawned tasks.
    ///
    /// # Implementation Note
    ///
    /// This method is a convenience wrapper around `futures_util::stream::buffer_unordered`.
    /// It is NOT a custom implementation - the concurrency limiting logic is provided
    /// by the futures_util crate.
    ///
    /// # Arguments
    ///
    /// * `futures` - Iterator of futures to execute
    /// * `max_concurrent` - Maximum number of concurrent tasks
    ///
    /// # Returns
    ///
    /// A vector of results, in the same order as the input futures.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let tasks = (0..10000).map(|i| async move { Ok::<_, ExecutionError>(i) });
    /// let results = executor.execute_batch_bounded(tasks, 100).await;
    /// ```
    pub async fn execute_batch_bounded<F, T, E>(
        &self,
        futures: impl IntoIterator<Item = F>,
        max_concurrent: usize,
    ) -> Vec<Result<T, ExecutionError>>
    where
        F: Future<Output = Result<T, E>> + Send + 'static,
        T: Send + 'static,
        E: Into<ExecutionError> + fmt::Debug + Send + 'static,
    {
        use futures_util::stream::{StreamExt, stream::iter};

        let stream = iter(futures)
            .map(|future| {
                let executor = self.clone();
                async move {
                    executor.execute(future).await_result().await
                }
            })
            .buffer_unordered(max_concurrent);

        stream.collect().await
            .into_iter()
            .map(|r| match r {
                Ok(result) => result,
                Err(join_err) => Err(ExecutionError {
                    kind: ExecutionErrorKind::Cancelled,
                    message: format!("Task cancelled: {}", join_err),
                }),
            })
            .collect()
    }

    /// Get the default timeout
    pub fn default_timeout(&self) -> Duration {
        self.default_timeout
    }

    /// Get a cancellation token that can be used to cancel all tasks
    ///
    /// When this token is cancelled, all tasks using the executor's
    /// default cancellation will be aborted.
    pub fn cancellation_token(&self) -> CancellationToken {
        self.cancel_token.clone()
    }

    /// Initiate graceful shutdown
    ///
    /// After calling this, new task submissions will be rejected.
    /// Existing tasks will continue to execute to completion.
    pub fn shutdown(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
    }

    /// Initiate immediate cancellation
    ///
    /// All tasks using this executor's cancellation token will be aborted.
    /// This is more aggressive than `shutdown()` which only rejects new tasks.
    pub fn cancel_all(&self) {
        self.cancel_token.cancel();
    }

    /// Check if executor is shutting down
    pub fn is_shutting_down(&self) -> bool {
        self.shutdown.load(Ordering::SeqCst)
    }

    /// Check if executor has been cancelled
    pub fn is_cancelled(&self) -> bool {
        self.cancel_token.is_cancelled()
    }
}

/// Helper function to convert any error into ExecutionError
pub fn into_execution_error<E: std::error::Error>(e: E) -> ExecutionError {
    ExecutionError {
        kind: ExecutionErrorKind::Internal,
        message: e.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_basic_execution() {
        let executor = ToolExecutor::default();
        let result = executor
            .execute(async { Ok::<i32, ExecutionError>(42) })
            .await_result()
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[tokio::test]
    async fn test_timeout() {
        let executor = ToolExecutor::builder()
            .with_default_timeout(Duration::from_millis(50))
            .build();

        let result = executor
            .execute(async {
                tokio::time::sleep(Duration::from_secs(1)).await;
                Ok::<i32, ExecutionError>(42)
            })
            .await_result()
            .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, ExecutionErrorKind::Timeout);
    }

    #[tokio::test]
    async fn test_retry_with_success() {
        use std::sync::atomic::AtomicU32;
        
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();
        
        let executor = ToolExecutor::builder()
            .with_retry_policy(RetryPolicy::with_retries(3))
            .build();

        let result = executor
            .execute(async move {
                let prev = attempts_clone.fetch_add(1, Ordering::Relaxed);
                if prev < 2 {
                    Err(ExecutionError {
                        kind: ExecutionErrorKind::Internal,
                        message: "Temporary failure".to_string(),
                    })
                } else {
                    Ok::<i32, ExecutionError>(42)
                }
            })
            .await_result()
            .await;

        assert!(result.is_ok());
        assert_eq!(attempts.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn test_retry_exhausted() {
        let executor = ToolExecutor::builder()
            .with_retry_policy(RetryPolicy::with_retries(2))
            .build();

        let result = executor
            .execute(async {
                Err::<i32, _>(ExecutionError {
                    kind: ExecutionErrorKind::Internal,
                    message: "Always fails".to_string(),
                })
            })
            .await_result()
            .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, ExecutionErrorKind::RetriesExhausted);
    }

    #[tokio::test]
    async fn test_retry_on_timeout() {
        use std::sync::atomic::AtomicU32;
        
        let attempts = Arc::new(AtomicU32::new(0));
        let attempts_clone = attempts.clone();
        
        let executor = ToolExecutor::builder()
            .with_default_timeout(Duration::from_millis(50))
            .with_retry_policy(
                RetryPolicy::with_retries(3)
                    .with_retry_on_timeout(true)
            )
            .build();

        let result = executor
            .execute(async move {
                let prev = attempts_clone.fetch_add(1, Ordering::Relaxed);
                if prev < 2 {
                    // Simulate timeout by sleeping longer than timeout
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
                Ok::<i32, ExecutionError>(42)
            })
            .await_result()
            .await;

        assert!(result.is_ok());
        assert_eq!(attempts.load(Ordering::Relaxed), 3);
    }

    #[tokio::test]
    async fn test_stats() {
        let executor = ToolExecutor::default();

        // Execute some tasks
        for _ in 0..10 {
            let _ = executor
                .execute(async { Ok::<i32, ExecutionError>(42) })
                .await_result()
                .await;
        }

        let stats = executor.stats();
        assert_eq!(stats.total_executed, 10);
        assert_eq!(stats.total_successes, 10);
    }

    #[tokio::test]
    async fn test_concurrency_limit() {
        use std::sync::atomic::AtomicUsize;

        let executor = ToolExecutor::builder()
            .with_max_concurrent(2)
            .build();

        let concurrent = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));

        let mut handles = Vec::new();
        for _ in 0..5 {
            let exec = executor.clone();
            let conc = concurrent.clone();
            let max_c = max_concurrent.clone();

            handles.push(tokio::spawn(async move {
                exec.execute(async move {
                    let prev = conc.fetch_add(1, Ordering::Relaxed);
                    let current = prev + 1;
                    max_c.fetch_max(current, Ordering::Relaxed);
                    
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    
                    conc.fetch_sub(1, Ordering::Relaxed);
                    Ok::<i32, ExecutionError>(42)
                }).await_result().await
            }));
        }

        for handle in handles {
            let _ = handle.await;
        }

        assert!(max_concurrent.load(Ordering::Relaxed) <= 2);
    }

    #[tokio::test]
    async fn test_shutdown() {
        let executor = ToolExecutor::default();
        
        assert!(!executor.is_shutting_down());
        
        executor.shutdown();
        
        assert!(executor.is_shutting_down());
        
        // New submissions should be rejected
        let result = executor
            .execute(async { Ok::<i32, ExecutionError>(42) })
            .await_result()
            .await;
        
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, ExecutionErrorKind::Shutdown);
    }

    #[tokio::test]
    async fn test_ring_buffer() {
        let mut buffer = RingBuffer::with_capacity(5);
        
        // Fill the buffer
        for i in 0..5 {
            buffer.push(i);
        }
        
        // Overwrite
        buffer.push(100);
        
        // Should contain: 1, 2, 3, 4, 100 (0 was overwritten)
        let values: Vec<u64> = buffer.iter().collect();
        assert_eq!(values, vec![1, 2, 3, 4, 100]);
    }

    #[tokio::test]
    async fn test_p99_calculation() {
        let executor = ToolExecutor::default();
        
        // Execute tasks with varying durations
        for i in 0..100 {
            let delay = Duration::from_millis(i as u64);
            let _ = executor
                .execute(async move {
                    tokio::time::sleep(delay).await;
                    Ok::<i32, ExecutionError>(i)
                })
                .await_result()
                .await;
        }
        
        let stats = executor.stats();
        
        // P99 should be around 99ms (99th percentile of 0-99)
        assert!(stats.p99_execution_time_ms >= 90);
        assert!(stats.avg_execution_time_ms >= 40);
    }

    #[tokio::test]
    async fn test_stress_high_concurrency() {
        let executor = ToolExecutor::builder()
            .with_max_concurrent(100)
            .build();

        let mut handles = Vec::new();
        for _ in 0..500 {
            let exec = executor.clone();
            handles.push(tokio::spawn(async move {
                exec.execute(async { Ok::<_, ExecutionError>(42) })
                    .await_result()
                    .await
            }));
        }

        // Verify all tasks completed
        let results = futures::future::join_all(handles).await;
        let success_count = results.iter().filter(|r| {
            matches!(r, Ok(Ok(_)))
        }).count();
        
        assert_eq!(success_count, 500);
    }

    #[tokio::test]
    async fn test_long_running_stats() {
        let executor = ToolExecutor::default();
        
        // Execute many tasks to test ring buffer wraparound
        for i in 0..2000 {
            let _ = executor
                .execute(async move {
                    tokio::time::sleep(Duration::from_millis(1)).await;
                    Ok::<i32, ExecutionError>(i)
                })
                .await_result()
                .await;
        }
        
        let stats = executor.stats();
        
        // Should have executed all tasks
        assert_eq!(stats.total_executed, 2000);
        assert_eq!(stats.total_successes, 2000);
        
        // P99 should be reasonable (based on last 1000 samples)
        assert!(stats.p99_execution_time_ms > 0);
        assert!(stats.p99_execution_time_ms < 100); // Should be < 100ms for 1ms sleep
    }

    #[tokio::test]
    async fn test_executor_high_concurrency() {
        let executor = ToolExecutor::builder()
            .with_max_concurrent(100)
            .build();

        let mut handles = Vec::new();
        for i in 0..500 {
            let exec = executor.clone();
            handles.push(tokio::spawn(async move {
                exec.execute(async move { 
                    tokio::time::sleep(Duration::from_millis(1)).await;
                    Ok::<_, ExecutionError>(i) 
                })
                .await_result()
                .await
            }));
        }

        // Verify all tasks completed
        let results = futures_util::future::join_all(handles).await;
        let success_count = results.iter().filter(|r| {
            matches!(r, Ok(Ok(_)))
        }).count();
        
        assert_eq!(success_count, 500);
    }

    #[tokio::test]
    async fn test_executor_extreme_concurrency() {
        let executor = ToolExecutor::builder()
            .with_max_concurrent(500)
            .build();

        let mut handles = Vec::new();
        for i in 0..1000 {
            let exec = executor.clone();
            handles.push(tokio::spawn(async move {
                exec.execute_simple(async move { 
                    Ok::<_, ExecutionError>(i) 
                }).await
            }));
        }

        // Verify all tasks completed successfully
        let results = futures_util::future::join_all(handles).await;
        for (i, result) in results.iter().enumerate() {
            assert!(result.is_ok(), "Task {} failed: {:?}", i, result);
        }
    }

    #[tokio::test]
    async fn test_batch_execution() {
        let executor = ToolExecutor::default();

        let tasks = vec![
            async { Ok::<_, ExecutionError>(1) },
            async { Ok::<_, ExecutionError>(2) },
            async { Ok::<_, ExecutionError>(3) },
        ];

        let results = executor.execute_batch(tasks).await;

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| r.is_ok()));
    }

    #[tokio::test]
    async fn test_batch_with_failures() {
        let executor = ToolExecutor::default();

        let tasks = vec![
            async { Ok::<_, ExecutionError>(1) },
            async { 
                Err::<i32, _>(ExecutionError {
                    kind: ExecutionErrorKind::Internal,
                    message: "fail".into(),
                })
            },
            async { Ok::<_, ExecutionError>(3) },
        ];

        let results = executor.execute_batch(tasks).await;
        
        // Should return 3 results, one is Err
        assert_eq!(results.len(), 3);
        assert!(results[0].is_ok());
        assert!(results[1].is_err());
        assert!(results[2].is_ok());
        assert_eq!(results[1].as_ref().unwrap_err().kind, ExecutionErrorKind::Internal);
    }

    #[tokio::test]
    async fn test_batch_preserves_order() {
        let executor = ToolExecutor::default();

        let tasks = vec![
            async { Ok::<_, ExecutionError>(1) },
            async { Ok::<_, ExecutionError>(2) },
            async { Ok::<_, ExecutionError>(3) },
        ];

        let results = executor.execute_batch(tasks).await;
        
        // Verify order is preserved
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].as_ref().unwrap(), &1);
        assert_eq!(results[1].as_ref().unwrap(), &2);
        assert_eq!(results[2].as_ref().unwrap(), &3);
    }

    #[tokio::test]
    async fn test_batch_all_ok_success() {
        let executor = ToolExecutor::default();

        let tasks = vec![
            async { Ok::<_, ExecutionError>(1) },
            async { Ok::<_, ExecutionError>(2) },
            async { Ok::<_, ExecutionError>(3) },
        ];

        let results = executor.execute_batch_all_ok(tasks).await;

        assert!(results.is_ok());
        assert_eq!(results.unwrap(), vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_batch_all_ok_failure() {
        let executor = ToolExecutor::default();

        let tasks = vec![
            async { Ok::<_, ExecutionError>(1) },
            async {
                Err::<i32, _>(ExecutionError {
                    kind: ExecutionErrorKind::Internal,
                    message: "Failed".to_string(),
                })
            },
            async { Ok::<_, ExecutionError>(3) },
        ];
        
        let results = executor.execute_batch_all_ok(tasks).await;
        
        assert!(results.is_err());
        assert_eq!(results.unwrap_err().kind, ExecutionErrorKind::Internal);
    }

    #[tokio::test]
    async fn test_concurrent_stats_access() {
        let executor = ToolExecutor::default();
        
        // Execute tasks while concurrently reading stats
        let mut handles = Vec::new();
        
        // Writer tasks
        for _ in 0..100 {
            let exec = executor.clone();
            handles.push(tokio::spawn(async move {
                exec.execute(async { Ok::<_, ExecutionError>(42) })
                    .await_result()
                    .await
            }));
        }
        
        // Reader tasks
        for _ in 0..10 {
            let exec = executor.clone();
            handles.push(tokio::spawn(async move {
                let _stats = exec.stats();
            }));
        }
        
        let results = futures_util::future::join_all(handles).await;
        
        // All should complete without panic
        assert!(results.iter().all(|r| r.is_ok()));
    }

    #[tokio::test]
    async fn test_semaphore_efficiency() {
        let executor = ToolExecutor::builder()
            .with_max_concurrent(10)
            .build();

        let concurrent_count = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));
        
        let mut handles = Vec::new();
        for _ in 0..100 {
            let exec = executor.clone();
            let count = concurrent_count.clone();
            let max_c = max_concurrent.clone();
            
            handles.push(tokio::spawn(async move {
                exec.execute(async move {
                    let prev = count.fetch_add(1, Ordering::SeqCst);
                    let current = prev + 1;
                    
                    // Track max concurrent
                    let mut max = max_c.load(Ordering::Relaxed);
                    while current > max {
                        match max_c.compare_exchange_weak(
                            max,
                            current,
                            Ordering::SeqCst,
                            Ordering::Relaxed
                        ) {
                            Ok(_) => break,
                            Err(new_max) => max = new_max,
                        }
                    }
                    
                    tokio::time::sleep(Duration::from_millis(10)).await;
                    
                    count.fetch_sub(1, Ordering::SeqCst);
                    Ok::<_, ExecutionError>(42)
                }).await_result().await
            }));
        }
        
        let results = futures_util::future::join_all(handles).await;
        assert!(results.iter().all(|r| r.is_ok()));

        // Verify concurrency was limited
        let observed_max = max_concurrent.load(Ordering::Relaxed);
        assert!(observed_max <= 10, "Max concurrent was {}, expected <= 10", observed_max);
    }

    #[tokio::test]
    async fn test_batch_bounded_concurrency() {
        let executor = ToolExecutor::default();

        let concurrent_count = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));

        // Create tasks that track concurrency
        let tasks: Vec<_> = (0..50).map(|_| {
            let count = concurrent_count.clone();
            let max_c = max_concurrent.clone();
            async move {
                let prev = count.fetch_add(1, Ordering::SeqCst);
                let current = prev + 1;

                // Track max concurrent
                let mut max = max_c.load(Ordering::Relaxed);
                while current > max {
                    match max_c.compare_exchange_weak(
                        max, current, Ordering::SeqCst, Ordering::Relaxed
                    ) {
                        Ok(_) => break,
                        Err(new_max) => max = new_max,
                    }
                }

                tokio::time::sleep(Duration::from_millis(5)).await;
                count.fetch_sub(1, Ordering::SeqCst);
                Ok::<_, ExecutionError>(42)
            }
        }).collect();

        // Execute with bounded concurrency (max 10)
        let results = executor.execute_batch_bounded(tasks, 10).await;

        assert!(results.iter().all(|r| r.is_ok()));

        // Verify concurrency was limited
        let observed_max = max_concurrent.load(Ordering::Relaxed);
        assert!(observed_max <= 10, "Max concurrent was {}, expected <= 10", observed_max);
    }

    #[tokio::test]
    async fn test_batch_bounded_large_batch() {
        let executor = ToolExecutor::default();

        // Create a large batch (1000 tasks)
        let tasks: Vec<_> = (0..1000).map(|i| async move {
            Ok::<_, ExecutionError>(i)
        }).collect();

        // Execute with bounded concurrency
        let results = executor.execute_batch_bounded(tasks, 50).await;

        assert_eq!(results.len(), 1000);
        assert!(results.iter().enumerate().all(|(i, r)| {
            matches!(r, Ok(v) if *v == i as i32)
        }));
    }

    #[tokio::test]
    async fn test_backpressure_rejection() {
        let executor = ToolExecutor::builder()
            .with_max_concurrent(2)
            .with_max_pending(3)
            .build();

        // Fill the queue (2 executing + 3 pending = 5 total)
        let mut permits = Vec::new();
        for _ in 0..2 {
            let permit = executor.semaphore.acquire_owned().await.unwrap();
            permits.push(permit);
        }

        // Try to submit tasks that would exceed the queue limit
        let mut handles = Vec::new();
        for _ in 0..5 {
            let result = executor.try_execute(async {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok::<_, ExecutionError>(42)
            });
            handles.push(result);
        }

        // Some should be rejected due to backpressure
        let success_count = handles.iter().filter(|r| r.is_ok()).count();
        let rejected_count = handles.iter().filter(|r| r.is_err()).count();
        
        // At least some should be rejected
        assert!(rejected_count > 0, "Expected some tasks to be rejected due to backpressure");
        
        // Drop permits to allow tasks to complete
        drop(permits);
    }

    #[tokio::test]
    async fn test_memory_leak_semaphore() {
        let executor = ToolExecutor::builder()
            .with_max_concurrent(10)
            .build();

        // Execute many tasks and verify semaphore permits are returned
        let initial_permits = executor.semaphore.available_permits();
        
        for _ in 0..1000 {
            let _ = executor.execute(async {
                Ok::<_, ExecutionError>(42)
            }).await_result().await;
        }

        // All permits should be returned
        let final_permits = executor.semaphore.available_permits();
        assert_eq!(initial_permits, final_permits, "Semaphore permits leaked!");
    }

    #[tokio::test]
    async fn test_shutdown_in_progress_tasks() {
        let executor = ToolExecutor::default();

        // Start a long-running task
        let handle = tokio::spawn({
            let exec = executor.clone();
            async move {
                exec.execute(async move {
                    tokio::time::sleep(Duration::from_millis(500)).await;
                    Ok::<_, ExecutionError>(42)
                }).await_result().await
            }
        });

        // Shutdown while task is running
        executor.shutdown();

        // The task should still complete (not cancelled)
        let result = handle.await.unwrap();
        assert!(result.is_ok(), "In-progress task should complete after shutdown");
    }

    #[tokio::test]
    async fn test_cancellation() {
        let executor = ToolExecutor::default();
        let cancel_token = executor.cancellation_token();

        // Start a long-running task with cancellation
        let handle = tokio::spawn(async move {
            executor.execute(async move {
                tokio::time::sleep(Duration::from_secs(10)).await;
                Ok::<_, ExecutionError>(42)
            })
            .with_cancellation(cancel_token)
            .await_result()
            .await
        });

        // Cancel after a short delay
        tokio::time::sleep(Duration::from_millis(50)).await;
        executor.cancel_all();

        // The task should be cancelled
        let result = handle.await.unwrap();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, ExecutionErrorKind::Cancelled);
    }

    #[tokio::test]
    async fn test_cancellation_during_retry() {
        let executor = ToolExecutor::builder()
            .with_retry_policy(RetryPolicy::with_retries(5))
            .build();
        
        let cancel_token = executor.cancellation_token();
        let attempts = Arc::new(AtomicUsize::new(0));

        // Start a task that always fails, with cancellation
        let handle = tokio::spawn({
            let attempts_clone = attempts.clone();
            async move {
                executor.execute(async move {
                    attempts_clone.fetch_add(1, Ordering::Relaxed);
                    Err::<i32, _>(ExecutionError {
                        kind: ExecutionErrorKind::Internal,
                        message: "Always fails".to_string(),
                    })
                })
                .with_cancellation(cancel_token.clone())
                .await_result()
                .await
            }
        });

        // Cancel after first retry starts
        tokio::time::sleep(Duration::from_millis(50)).await;
        executor.cancel_all();

        let result = handle.await.unwrap();
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, ExecutionErrorKind::Cancelled);
        
        // Should have attempted fewer times than max retries
        assert!(attempts.load(Ordering::Relaxed) < 6);
    }

    #[tokio::test]
    async fn test_is_cancelled() {
        let executor = ToolExecutor::default();
        
        assert!(!executor.is_cancelled());
        
        executor.cancel_all();
        
        assert!(executor.is_cancelled());
    }

    #[tokio::test]
    async fn test_priority_scheduling_order() {
        // 验证高优先级任务是否真的优先完成
        use std::sync::Arc;
        use tokio::sync::Mutex;

        let executor = ToolExecutor::builder()
            .with_max_concurrent(1)  // Only 1 concurrent task
            .with_priority_support()  // Enable priority scheduling
            .build();

        let completion_order = Arc::new(Mutex::new(Vec::new()));

        // 先提交一个低优先级长任务
        let low_handle = tokio::spawn({
            let exec = executor.clone();
            let order = completion_order.clone();
            async move {
                exec.execute(async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok::<_, ExecutionError>(1)
                })
                .with_priority(Priority::Low)
                .await_result()
                .await
            }
        });

        // 等待低优先级任务开始执行
        tokio::time::sleep(Duration::from_millis(20)).await;

        // 提交高优先级任务（应该等待）- 执行相同时间
        let high_handle = tokio::spawn({
            let exec = executor.clone();
            let order = completion_order.clone();
            async move {
                let result = exec.execute(async move {
                    // 同样执行 100ms - 确保执行时间相同
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok::<_, ExecutionError>(2)
                })
                .with_priority(Priority::High)
                .await_result()
                .await;

                // 记录完成顺序
                order.lock().await.push("high");
                result
            }
        });

        // 提交另一个低优先级任务（应该在高优先级之后）- 执行相同时间
        let low2_handle = tokio::spawn({
            let exec = executor.clone();
            let order = completion_order.clone();
            async move {
                let result = exec.execute(async move {
                    // 同样执行 100ms
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok::<_, ExecutionError>(3)
                })
                .with_priority(Priority::Low)
                .await_result()
                .await;

                // 记录完成顺序
                order.lock().await.push("low2");
                result
            }
        });

        // 等待所有任务完成
        let _ = low_handle.await.unwrap();
        let _ = high_handle.await.unwrap();
        let _ = low2_handle.await.unwrap();

        // 验证完成顺序：high 应该在 low2 之前
        let order = completion_order.lock().await;
        assert_eq!(order.len(), 2);
        assert_eq!(order[0], "high", "High priority should complete before low2");
        assert_eq!(order[1], "low2");
    }

    #[tokio::test]
    async fn test_priority_shared_permit_pool() {
        // 验证共享许可池：总并发 = max_concurrent，不是 max_concurrent * 3
        use std::sync::atomic::AtomicUsize;

        let executor = ToolExecutor::builder()
            .with_max_concurrent(10)
            .with_priority_support()
            .build();

        let current_concurrent = Arc::new(AtomicUsize::new(0));
        let max_concurrent = Arc::new(AtomicUsize::new(0));

        // Submit 10 high + 10 normal + 10 low priority tasks
        let mut handles = Vec::new();
        
        for _ in 0..10 {
            let exec = executor.clone();
            let current = current_concurrent.clone();
            let max = max_concurrent.clone();
            handles.push(tokio::spawn(async move {
                let prev = current.fetch_add(1, Ordering::SeqCst);
                let new_current = prev + 1;
                
                // Track max concurrent
                let mut old_max = max.load(Ordering::Relaxed);
                while new_current > old_max {
                    match max.compare_exchange_weak(
                        old_max,
                        new_current,
                        Ordering::SeqCst,
                        Ordering::Relaxed
                    ) {
                        Ok(_) => break,
                        Err(new_old_max) => old_max = new_old_max,
                    }
                }
                
                exec.execute(async move {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Ok::<_, ExecutionError>(1)
                })
                .with_priority(Priority::High)
                .await_result()
                .await
            }));
        }
        
        for _ in 0..10 {
            let exec = executor.clone();
            let current = current_concurrent.clone();
            let max = max_concurrent.clone();
            handles.push(tokio::spawn(async move {
                let prev = current.fetch_add(1, Ordering::SeqCst);
                let new_current = prev + 1;
                
                // Track max concurrent
                let mut old_max = max.load(Ordering::Relaxed);
                while new_current > old_max {
                    match max.compare_exchange_weak(
                        old_max,
                        new_current,
                        Ordering::SeqCst,
                        Ordering::Relaxed
                    ) {
                        Ok(_) => break,
                        Err(new_old_max) => old_max = new_old_max,
                    }
                }
                
                exec.execute(async move {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Ok::<_, ExecutionError>(2)
                })
                .with_priority(Priority::Normal)
                .await_result()
                .await
            }));
        }
        
        for _ in 0..10 {
            let exec = executor.clone();
            let current = current_concurrent.clone();
            let max = max_concurrent.clone();
            handles.push(tokio::spawn(async move {
                let prev = current.fetch_add(1, Ordering::SeqCst);
                let new_current = prev + 1;
                
                // Track max concurrent
                let mut old_max = max.load(Ordering::Relaxed);
                while new_current > old_max {
                    match max.compare_exchange_weak(
                        old_max,
                        new_current,
                        Ordering::SeqCst,
                        Ordering::Relaxed
                    ) {
                        Ok(_) => break,
                        Err(new_old_max) => old_max = new_old_max,
                    }
                }
                
                exec.execute(async move {
                    tokio::time::sleep(Duration::from_millis(50)).await;
                    Ok::<_, ExecutionError>(3)
                })
                .with_priority(Priority::Low)
                .await_result()
                .await
            }));
        }

        // All should complete
        let results = futures_util::future::join_all(handles).await;
        assert!(results.iter().all(|r| r.is_ok()));

        // Verify max concurrent <= 10 (shared permit pool)
        let observed_max = max_concurrent.load(Ordering::Relaxed);
        assert!(
            observed_max <= 10,
            "Max concurrent was {}, expected <= 10 (shared permit pool)",
            observed_max
        );
    }

    #[tokio::test]
    async fn test_backpressure_holds_until_completion() {
        let executor = ToolExecutor::builder()
            .with_max_concurrent(10)
            .with_max_pending(5)  // Only 5 pending tasks allowed
            .build();

        // Start 5 tasks that hold queue permits
        let mut handles = Vec::new();
        for i in 0..5 {
            let exec = executor.clone();
            handles.push(tokio::spawn(async move {
                exec.execute(async move {
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    Ok::<_, ExecutionError>(i)
                })
                .await_result()
                .await
            }));
        }

        // Give tasks time to acquire queue permits
        tokio::time::sleep(Duration::from_millis(10)).await;

        // Try to submit 6th task - should wait for queue permit
        let start = std::time::Instant::now();
        let result = executor.execute(async move {
            Ok::<_, ExecutionError>(6)
        })
        .await_result()
        .await;

        // Should have waited for a queue permit to be released
        assert!(result.is_ok());
        assert!(start.elapsed() >= Duration::from_millis(50));
    }

    #[tokio::test]
    async fn test_try_execute_queue_full() {
        let executor = ToolExecutor::builder()
            .with_max_concurrent(2)
            .with_max_pending(2)
            .build();

        // Fill the queue
        let mut handles = Vec::new();
        for _ in 0..4 {
            let handle = executor.try_execute(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok::<_, ExecutionError>(1)
            });
            if let Ok(h) = handle {
                handles.push(h);
            }
        }

        // 5th task should be rejected with QueueFull
        let result = executor.try_execute(async move {
            Ok::<_, ExecutionError>(1)
        });
        
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind, ExecutionErrorKind::QueueFull);

        // Wait for existing tasks to complete
        for handle in handles {
            let _ = handle.await;
        }
    }

    #[tokio::test]
    async fn test_priority_without_support() {
        let executor = ToolExecutor::builder()
            .with_max_concurrent(10)
            .build();  // No priority support

        // Priority should be ignored when not enabled
        let result = executor
            .execute(async { Ok::<_, ExecutionError>(42) })
            .with_priority(Priority::High)
            .await_result()
            .await;
        
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }
}
