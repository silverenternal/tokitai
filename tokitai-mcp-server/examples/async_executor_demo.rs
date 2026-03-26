//! Tokitai 异步执行器示例
//!
//! 此示例展示如何使用 tokitai 的异步执行器来优化 MCP 服务器的性能：
//! - 并发控制：限制同时执行的工具数量
//! - 超时机制：防止慢工具阻塞服务器
//! - 可选重试：配置重试策略处理暂时失败
//! - 统计监控：跟踪执行时间和成功率
//!
//! 运行示例：
//! ```bash
//! cargo run --example async_executor_demo -p tokitai-mcp-server
//! ```

use tokitai::tool;
use tokitai_core::executor::{ExecutionError, ToolExecutor, RetryPolicy, BackoffStrategy};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

// ============================================================================
// 定义示例工具
// ============================================================================

#[tool]
pub struct AsyncTools {
    // 使用 Arc 存储状态，而不是全局静态变量
    flaky_attempts: Arc<AtomicU32>,
}

impl Default for AsyncTools {
    fn default() -> Self {
        Self {
            flaky_attempts: Arc::new(AtomicU32::new(0)),
        }
    }
}

#[tool]
impl AsyncTools {
    /// 快速任务 - 模拟轻量级操作
    pub fn quick_task(&self, message: String) -> String {
        format!("快速响应：{}", message)
    }

    /// 可能超时的任务
    pub fn slow_task(&self, duration_secs: u64) -> String {
        std::thread::sleep(Duration::from_secs(duration_secs));
        format!("慢任务完成，耗时 {} 秒", duration_secs)
    }

    /// 可能失败的任务（用于演示重试）
    /// 使用实例状态而不是全局静态变量
    pub fn flaky_task(&self, attempt_count: u32) -> Result<String, String> {
        let current = self.flaky_attempts.fetch_add(1, Ordering::Relaxed);
        
        if current < attempt_count {
            Err(format!("暂时失败，第 {} 次尝试", current + 1))
        } else {
            // 重置计数器，以便下次演示
            self.flaky_attempts.store(0, Ordering::Relaxed);
            Ok(format!("成功！尝试了 {} 次", current + 1))
        }
    }
    
    /// 重置重试计数器
    pub fn reset_flaky_counter(&self) {
        self.flaky_attempts.store(0, Ordering::Relaxed);
    }
}

// ============================================================================
// 异步执行器示例
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Tokitai 异步执行器演示 ===\n");

    // 1. 创建自定义执行器
    let executor = ToolExecutor::builder()
        .with_max_concurrent(10)         // 最大并发任务数
        .with_default_timeout(Duration::from_secs(5)) // 默认超时
        .with_retry_policy(RetryPolicy::with_retries(3)) // 最多重试 3 次
        .build();

    println!("执行器配置:");
    let stats = executor.stats();
    println!("  - 最大并发：{}", stats.max_concurrent);
    println!("  - 默认超时：{:?}", executor.default_timeout());
    println!();

    let tools = AsyncTools::default();

    // 2. 基本执行
    println!("1. 基本执行...");
    let result = executor
        .execute(async {
            Ok::<_, ExecutionError>(tools.quick_task("Hello!".to_string()))
        })
        .await_result()
        .await?;
    println!("   结果：{}\n", result);

    // 3. 带超时执行
    println!("2. 带超时执行 (任务需要 3 秒，超时设置为 1 秒)...");
    match executor
        .execute(async {
            Ok::<_, ExecutionError>(tools.slow_task(3))
        })
        .with_timeout(Duration::from_secs(1))
        .await_result()
        .await
    {
        Ok(result) => println!("   结果：{}", result),
        Err(e) => println!("   预期错误：{:?} - {}\n", e.kind, e.message),
    }

    // 4. 演示重试机制
    println!("3. 演示重试机制 (前 2 次失败，第 3 次成功)...");
    tools.reset_flaky_counter(); // 重置计数器
    
    let result = executor
        .execute(async {
            Ok::<_, ExecutionError>(tools.flaky_task(2).map_err(|e| ExecutionError {
                kind: tokitai_core::executor::ExecutionErrorKind::Internal,
                message: e,
            })?)
        })
        .await_result()
        .await?;
    println!("   结果：{}\n", result);

    // 5. 演示并发控制
    println!("4. 演示并发控制 (同时提交 20 个任务，最大并发 10)...");
    let start = std::time::Instant::now();

    let mut handles = Vec::new();
    for i in 0..20 {
        let exec = executor.clone();
        let handle = tokio::spawn(async move {
            exec.execute(async move {
                // 模拟一些工作
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok::<_, ExecutionError>(i)
            })
            .await_result()
            .await
        });
        handles.push(handle);
    }

    // 等待所有任务完成
    let mut results = Vec::new();
    for handle in handles {
        if let Ok(Ok(val)) = handle.await {
            results.push(val);
        }
    }

    let elapsed = start.elapsed();
    println!("   完成 {} 个任务，耗时：{:?}", results.len(), elapsed);
    println!("   (如果串行执行需要 2 秒，实际耗时证明并发有效)\n");

    // 6. 显示执行器统计
    println!("5. 执行器统计:");
    let final_stats = executor.stats();
    println!("   - 总执行任务数：{}", final_stats.total_executed);
    println!("   - 成功数：{}", final_stats.total_successes);
    println!("   - 失败数：{}", final_stats.total_failures);
    println!("   - 超时数：{}", final_stats.total_timeouts);
    println!("   - 重试数：{}", final_stats.total_retries);
    println!("   - 平均执行时间：{}ms", final_stats.avg_execution_time_ms);
    println!("   - P99 执行时间：{}ms", final_stats.p99_execution_time_ms);

    println!("\n=== 演示完成 ===");
    println!("\n提示：运行 MCP 服务器示例来查看完整的异步服务器：");
    println!("  cargo run --example mcp_builder_demo -p tokitai-mcp-server");

    Ok(())
}
