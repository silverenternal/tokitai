//! Performance benchmarks for the async executor
//!
//! Run with: `cargo bench --bench executor_bench`

#![feature(test)]
extern crate test;

#[cfg(test)]
mod benches {
    use test::{black_box, Bencher};
    use tokitai_core::executor::{ExecutionError, ToolExecutor, RetryPolicy, BackoffStrategy};
    use std::time::Duration;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;

    /// Benchmark: Basic execution with new API
    #[bench]
    fn bench_basic_execution(b: &mut Bencher) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let executor = ToolExecutor::default();

        b.iter(|| {
            rt.block_on(async {
                let result: Result<i32, ExecutionError> = executor
                    .execute(async {
                        black_box(1 + 1);
                        Ok(black_box(42))
                    })
                    .await_result()
                    .await;
                result.unwrap()
            })
        });
    }

    /// Benchmark: Execution with timeout
    #[bench]
    fn bench_execution_with_timeout(b: &mut Bencher) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let executor = ToolExecutor::default();

        b.iter(|| {
            rt.block_on(async {
                let result: Result<i32, ExecutionError> = executor
                    .execute(async {
                        Ok(black_box(42))
                    })
                    .with_timeout(Duration::from_secs(5))
                    .await_result()
                    .await;
                result.unwrap()
            })
        });
    }

    /// Benchmark: Execution with retry (success on first try)
    #[bench]
    fn bench_execution_with_retry_success(b: &mut Bencher) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let executor = ToolExecutor::builder()
            .with_retry_policy(RetryPolicy::with_retries(3))
            .build();

        b.iter(|| {
            rt.block_on(async {
                let result: Result<i32, ExecutionError> = executor
                    .execute(async {
                        Ok(black_box(42))
                    })
                    .await_result()
                    .await;
                result.unwrap()
            })
        });
    }

    /// Benchmark: Execution with retry (success on third try)
    /// Each iteration uses independent counter to avoid race conditions
    #[bench]
    fn bench_execution_with_retry_recovery(b: &mut Bencher) {
        let rt = tokio::runtime::Runtime::new().unwrap();

        b.iter(|| {
            // Each iteration has its own counter - no shared state
            let attempts = Arc::new(AtomicUsize::new(0));
            let attempts_clone = attempts.clone();
            
            let executor = ToolExecutor::builder()
                .with_retry_policy(RetryPolicy::with_retries(3))
                .build();

            rt.block_on(async {
                let result: Result<i32, ExecutionError> = executor
                    .execute(async move {
                        let prev = attempts_clone.fetch_add(1, Ordering::Relaxed);
                        if prev < 2 {
                            Err(ExecutionError {
                                kind: tokitai_core::executor::ExecutionErrorKind::Internal,
                                message: "Temporary failure".to_string(),
                            })
                        } else {
                            Ok(black_box(42))
                        }
                    })
                    .await_result()
                    .await;
                result.unwrap()
            })
        });
    }

    /// Benchmark: Concurrent execution (10 concurrent)
    #[bench]
    fn bench_concurrent_execution(b: &mut Bencher) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let executor = ToolExecutor::default();

        b.iter(|| {
            rt.block_on(async {
                let mut handles = Vec::new();
                for i in 0..10 {
                    let exec = executor.clone();
                    handles.push(tokio::spawn(async move {
                        exec.execute(async move {
                            black_box(i * i);
                            Ok::<_, ExecutionError>(black_box(i))
                        }).await_result().await
                    }));
                }

                let mut sum = 0;
                for handle in handles {
                    if let Ok(Ok(val)) = handle.await {
                        sum += val;
                    }
                }
                sum
            })
        });
    }

    /// Benchmark: Statistics collection overhead
    #[bench]
    fn bench_stats_collection(b: &mut Bencher) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let executor = ToolExecutor::default();

        // Execute some tasks first
        rt.block_on(async {
            for _ in 0..100 {
                let _ = executor.execute(async {
                    Ok::<_, ExecutionError>(42)
                }).await_result().await;
            }
        });

        b.iter(|| {
            let stats = executor.stats();
            black_box(stats.total_executed)
        });
    }

    /// Benchmark: Executor builder
    #[bench]
    fn bench_executor_builder(b: &mut Bencher) {
        b.iter(|| {
            let _executor = ToolExecutor::builder()
                .with_max_concurrent(black_box(100))
                .with_default_timeout(black_box(Duration::from_secs(30)))
                .with_retry_policy(RetryPolicy::with_retries(black_box(3)))
                .build();
        });
    }

    /// Benchmark: Ring buffer internal operations
    #[bench]
    fn bench_ring_buffer_internal(b: &mut Bencher) {
        // This benchmark tests the internal statistics collection efficiency
        let rt = tokio::runtime::Runtime::new().unwrap();
        let executor = ToolExecutor::default();

        b.iter(|| {
            rt.block_on(async {
                // Execute many tasks to stress the ring buffer
                let mut handles = Vec::new();
                for _ in 0..10 {
                    let exec = executor.clone();
                    handles.push(tokio::spawn(async move {
                        exec.execute(async {
                            Ok::<_, ExecutionError>(42)
                        }).await_result().await
                    }));
                }
                for handle in handles {
                    let _ = handle.await;
                }
            });
            black_box(())
        });
    }

    /// Benchmark: Semaphore acquisition
    #[bench]
    fn bench_semaphore_acquisition(b: &mut Bencher) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let executor = ToolExecutor::builder()
            .with_max_concurrent(100)
            .build();

        b.iter(|| {
            rt.block_on(async {
                let _permit = executor.semaphore.clone().acquire_owned().await.unwrap();
                black_box(())
            })
        });
    }

    /// Benchmark: Shutdown check
    #[bench]
    fn bench_shutdown_check(b: &mut Bencher) {
        let executor = ToolExecutor::default();

        b.iter(|| {
            black_box(executor.is_shutting_down())
        });
    }

    /// Benchmark: High concurrency stress test
    #[bench]
    fn bench_stress_high_concurrency(b: &mut Bencher) {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let executor = ToolExecutor::builder()
            .with_max_concurrent(100)
            .build();

        b.iter(|| {
            rt.block_on(async {
                let mut handles = Vec::new();
                for _ in 0..100 {
                    let exec = executor.clone();
                    handles.push(tokio::spawn(async move {
                        exec.execute(async {
                            Ok::<_, ExecutionError>(42)
                        }).await_result().await
                    }));
                }
                
                // Wait for all to complete
                for handle in handles {
                    let _ = handle.await;
                }
            })
        });
    }
}
