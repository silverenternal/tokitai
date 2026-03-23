# 异步执行器指南

本指南介绍如何使用 Tokitai 的异步执行器来简化 MCP 服务器的异步处理。

## 概述

Tokitai v0.4.0+ 引入了一个轻量级的异步执行器包装器，提供：

- **并发控制** - 限制同时执行的工具数量，防止服务器过载
- **超时支持** - 自动终止执行时间过长的工具
- **可选重试** - 配置重试策略（支持指数退避、超时重试）
- **统计监控** - 跟踪执行次数、成功率、P99 延迟
- **背压支持** - 可选的任务队列限制，防止内存爆炸

### 设计说明

此执行器是 tokio 运行时的**薄包装**，专注于 MCP 服务器的常见需求。

**这是什么**：
- tokio::sync::Semaphore 的包装器
- 统一的配置接口
- 便捷的链式 API
- 高效的统计收集（环形缓冲区，O(1) 操作）
- 可选的背压机制

**这不是什么**：
- 独立的线程池（使用 tokio 内置的调度器）
- 真正的优先级调度
- 自定义任务调度算法
- 完整的异步执行器（如 async-executor）

对于更高级的调度需求，建议直接使用 tokio 或专用 crate：
- `rayon` - CPU 密集型并行计算
- `async-executor` - 自定义调度
- `tokio` - 直接使用底层原语

## 快速开始

### 启用异步功能

在 `Cargo.toml` 中：

```toml
[dependencies]
tokitai = { version = "0.4.0", features = ["async"] }
tokitai-mcp-server = { version = "0.4.0", features = ["async"] }
```

### 配置 MCP 服务器

```rust
use tokitai_mcp_server::{McpServerBuilder, McpServerConfig};

#[tokio::main]
async fn main() {
    // 创建自定义配置
    let config = McpServerConfig::default()
        .with_port(8080)
        .with_async_params(
            100,  // 最大并发：100 个工具调用
            30,   // 超时：30 秒
            3,    // 最大重试次数：3 次
        );

    let server = McpServerBuilder::with_tool(MyTools::default())
        .with_config(config)
        .build();

    server.run().await.unwrap();
}
```

## 执行器配置参数

| 参数 | 默认值 | 说明 |
|------|--------|------|
| `max_concurrent` | 100 | 最大并发工具调用 |
| `timeout_secs` | 30 | 默认超时 (秒) |
| `max_retries` | 0 | 最大重试次数 |

### 最大并发 (`max_concurrent`)

- **默认值**: 100
- **用途**: 限制同时执行的工具数量
- **建议**: 根据服务器资源和预期负载调整

### 超时时间 (`timeout_secs`)

- **默认值**: 30 秒
- **用途**: 工具执行的最长时间
- **建议**: 根据工具特性设置

### 最大重试次数 (`max_retries`)

- **默认值**: 0（不重试）
- **用途**: 失败任务的自动重试次数
- **退避策略**: 指数退避（10ms, 20ms, 40ms, ...，最大 1 秒）
- **建议**: 对于可能暂时失败的操作（如网络请求），设置为 2-3

### 超时重试 (`retry_on_timeout`)

- **默认值**: false（超时不重试）
- **用途**: 是否在超时后重试
- **建议**: 对于可能因暂时网络问题导致的超时，启用此选项

```rust
let retry_policy = RetryPolicy::with_retries(3)
    .with_retry_on_timeout(true);  // 超时也重试
```

### 最大等待任务数 (`max_pending`)

- **默认值**: None（无限制）
- **用途**: 限制等待执行的任务数量（背压机制）
- **行为**: 超过限制时，新任务会被拒绝
- **建议**: 对于高负载场景，设置为 `max_concurrent * 10` 左右

```rust
let executor = ToolExecutor::builder()
    .with_max_concurrent(100)
    .with_max_pending(500)  // 最多 500 个等待任务
    .build();

// 或者使用 try_execute 进行非阻塞提交
match executor.try_execute(my_task()) {
    Ok(handle) => { /* 任务已提交 */ }
    Err(e) => { /* 队列已满，拒绝提交 */ }
}
```

### 优先级调度 (`with_priority_support`)

- **默认值**: 禁用
- **用途**: 启用基于优先级的调度
- **行为**: 高优先级任务在竞争时会优先获取执行许可
- **注意**: 这是"软"优先级系统，不保证严格的优先级顺序

```rust
let executor = ToolExecutor::builder()
    .with_max_concurrent(100)
    .with_priority_support()  // 启用优先级调度
    .build();

// 高优先级任务
executor.execute(urgent_task())
    .with_priority(Priority::High)
    .await_result()
    .await?;

// 普通优先级任务（默认）
executor.execute(normal_task())
    .await_result()
    .await?;

// 低优先级任务
executor.execute(background_task())
    .with_priority(Priority::Low)
    .await_result()
    .await?;
```

### 任务取消

```rust
let executor = ToolExecutor::default();
let cancel_token = executor.cancellation_token();

// 提交可取消的任务
let handle = tokio::spawn({
    let exec = executor.clone();
    async move {
        exec.execute(my_task())
            .with_cancellation(cancel_token)
            .await_result()
            .await
    }
});

// 取消所有任务
executor.cancel_all();

// 检查取消状态
if executor.is_cancelled() {
    // 执行器已取消
}
```

## 使用执行器 API

### 基本执行

```rust
use tokitai_core::executor::ToolExecutor;

let executor = ToolExecutor::default();

let result = executor
    .execute(async {
        // 你的异步任务
        Ok::<_, ExecutionError>(42)
    })
    .await_result()
    .await;
```

### 带超时执行

```rust
use std::time::Duration;

let result = executor
    .execute(async {
        // 可能耗时的任务
        Ok::<_, ExecutionError>(compute_something())
    })
    .with_timeout(Duration::from_secs(5))
    .await_result()
    .await;
```

### 带重试执行

```rust
use tokitai_core::executor::{RetryPolicy, BackoffStrategy};

let retry_policy = RetryPolicy::with_retries(3)
    .with_backoff(BackoffStrategy::exponential());

let result = executor
    .execute(async {
        // 可能暂时失败的任务
        fetch_from_network().await
    })
    .with_retry(retry_policy)
    .await_result()
    .await;
```

### 链式调用

```rust
let result = executor
    .execute(my_async_task())
    .with_timeout(Duration::from_secs(10))
    .with_retry(RetryPolicy::with_retries(2))
    .await_result()
    .await;
```

### 便捷方法

```rust
// 简单执行（使用默认配置）
let result = executor.execute_simple(my_task()).await?;

// 带超时执行
let result = executor.execute_with_timeout(my_task(), Duration::from_secs(5)).await?;

// 阻塞任务
let result = executor.execute_blocking(|| {
    // 同步代码
    Ok::<_, MyError>(compute())
}).await?;

// 批处理执行
let tasks = vec![
    async { Ok::<_, ExecutionError>(1) },
    async { Ok::<_, ExecutionError>(2) },
    async { Ok::<_, ExecutionError>(3) },
];

// 批处理（保留所有结果，包括错误）
let results = executor.execute_batch(tasks).await;
// results = [Ok(1), Ok(2), Ok(3)] 或包含错误

// 批处理（全或无）
let results = executor.execute_batch_all_ok(tasks).await?;
// Ok([1, 2, 3]) 或 Err(first_error)

// 大批量批处理（带并发限制）
let tasks = (0..10000).map(|i| async move { Ok::<_, ExecutionError>(i) });
let results = executor.execute_batch_bounded(tasks, 100).await;
// 最多同时执行 100 个任务
```

## 统计监控

执行器提供详细的运行时统计：

```rust
let stats = executor.stats();

println!("运行中任务：{}", stats.running_tasks);
println!("总执行数：{}", stats.total_executed);
println!("成功数：{}", stats.total_successes);
println!("失败数：{}", stats.total_failures);
println!("超时数：{}", stats.total_timeouts);
println!("重试数：{}", stats.total_retries);
println!("平均执行时间：{}ms", stats.avg_execution_time_ms);
println!("P99 执行时间：{}ms", stats.p99_execution_time_ms);
println!("最大并发：{}", stats.max_concurrent);
```

### 统计说明

- **P99 延迟**: 基于最近 1000 次执行样本计算
- **平均延迟**: 基于最近 1000 次执行样本计算
- **环形缓冲区**: 使用 O(1) 操作，高效记录统计

## 错误处理

```rust
use tokitai_core::executor::{ExecutionError, ExecutionErrorKind};

match executor.execute(my_task()).await_result().await {
    Ok(result) => {
        // 成功
    }
    Err(e) => match e.kind {
        ExecutionErrorKind::Timeout => {
            // 超时处理
        }
        ExecutionErrorKind::Panic => {
            // 任务崩溃处理
        }
        ExecutionErrorKind::Shutdown => {
            // 执行器关闭处理
        }
        ExecutionErrorKind::RetriesExhausted => {
            // 重试耗尽处理
        }
        ExecutionErrorKind::Internal => {
            // 应用特定错误处理
        }
        _ => {}
    }
}
```

### 错误转换辅助函数

```rust
use tokitai_core::executor::into_execution_error;

// 将任何 Error 转换为 ExecutionError
let exec_error = into_execution_error(my_custom_error);
```

## 最佳实践

### 1. 根据负载调整参数

```rust
// 高并发场景
let config = McpServerConfig::default()
    .with_async_params(500, 60, 3);

// 低延迟要求
let config = McpServerConfig::default()
    .with_async_params(100, 5, 0);

// 资源受限环境
let config = McpServerConfig::default()
    .with_async_params(50, 10, 1);
```

### 2. 监控和告警

```rust
let stats = executor.stats();

// 高超时率告警
if stats.total_timeouts > stats.total_executed / 10 {
    tracing::warn!("High timeout rate: {}%", 
        stats.total_timeouts * 100 / stats.total_executed);
}

// 高失败率告警
if stats.total_failures > stats.total_executed / 20 {
    tracing::warn!("High failure rate: {}%", 
        stats.total_failures * 100 / stats.total_executed);
}

// P99 延迟告警
if stats.p99_execution_time_ms > 5000 {
    tracing::warn!("High P99 latency: {}ms", stats.p99_execution_time_ms);
}
```

### 3. 优雅关闭

```rust
// 停止接受新任务
executor.shutdown();

// 等待现有任务完成
tokio::time::sleep(Duration::from_secs(5)).await;

// 检查关闭状态
if executor.is_shutting_down() {
    tracing::info!("Executor shutting down");
}
```

## 运行示例

```bash
# 异步执行器演示
cargo run --example async_executor_demo -p tokitai-mcp-server

# MCP 服务器示例
cargo run --example mcp_builder_demo -p tokitai-mcp-server
```

## 性能特征

性能取决于任务类型和系统负载。在典型场景下（8 核 CPU，16GB RAM）：

| 操作 | 典型时间 |
|------|----------|
| 基础执行 | 亚毫秒级 |
| 并发 100 任务 | P99 < 100ms |
| 重试（3 次） | +70ms 退避 |
| 统计收集 | < 10μs |

**注意**: 实际性能取决于任务类型和系统负载。建议运行 benchmark 获取准确数据：

```bash
cargo bench --bench executor_bench -p tokitai-core
```

## 常见问题

### Q: 为什么没有独立的 CPU/IO 线程池？

A: 此执行器是 tokio 的薄包装。tokio 已经有成熟的调度器和线程池管理，
重新实现只会增加复杂性和潜在 bug。如果你需要自定义线程池，可以直接
使用 `tokio::task::spawn_blocking` 或考虑 `rayon` crate。

### Q: 如何自定义退避策略？

A: 使用 `BackoffStrategy` 枚举：

```rust
// 固定退避
let backoff = BackoffStrategy::fixed(Duration::from_millis(100));

// 自定义指数退避
let backoff = BackoffStrategy::Exponential {
    initial_delay: Duration::from_millis(50),
    max_delay: Some(Duration::from_secs(5)),
};

let policy = RetryPolicy::with_retries(5)
    .with_backoff(backoff);
```

### Q: 统计会影响性能吗？

A: 影响极小。统计收集使用无锁原子操作和高效的环形缓冲区（O(1) 推送），
开销通常 < 1%。

### Q: 超时后会重试吗？

A: 默认不重试。如果需要超时后重试，使用 `retry_on_timeout` 选项：

```rust
let retry_policy = RetryPolicy::with_retries(3)
    .with_retry_on_timeout(true);
```

### Q: 如何区分"首次失败"和"重试耗尽"？

A: 检查错误类型：

```rust
match error.kind {
    ExecutionErrorKind::Internal => { /* 首次失败 */ }
    ExecutionErrorKind::RetriesExhausted => { /* 重试耗尽 */ }
    _ => {}
}
```

## 更多信息

- [API 文档](https://docs.rs/tokitai-core/latest/tokitai_core/executor/)
- [架构说明](./ARCHITECTURE.md)
