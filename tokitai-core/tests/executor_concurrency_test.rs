//! ToolExecutor 并发控制测试
//!
//! 运行测试：cargo test -p tokitai-core --test executor_concurrency_test --features async

#![cfg(feature = "async")]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokitai_core::executor::{ExecutionError, Priority, ToolExecutor};

/// 测试 1: 验证优先级调度顺序
///
/// 注意：这是一个"软"优先级测试，验证高优先级任务在竞争时是否有优势。
/// 由于使用共享许可池，不保证严格的优先级顺序。
#[tokio::test]
async fn test_priority_ordering() {
    use tokio::sync::{Barrier, Mutex};

    let executor = ToolExecutor::builder()
        .with_max_concurrent(1)  // 只有 1 个并发，强制竞争
        .with_priority_support()
        .build();

    let completion_order = Arc::new(Mutex::new(Vec::new()));
    let barrier = Arc::new(Barrier::new(2));  // 用于确保第一个任务真正开始

    // 先提交一个低优先级任务，占用执行许可
    let low_handle = tokio::spawn({
        let exec = executor.clone();
        let order = completion_order.clone();
        let barrier = barrier.clone();
        async move {
            let result = exec.execute(async move {
                // 等待测试开始后才执行
                barrier.wait().await;
                // 执行 100ms
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok::<_, ExecutionError>(1)
            })
            .with_priority(Priority::Low)
            .await_result()
            .await;
            
            // 低优先级任务完成后记录
            order.lock().await.push("low_done");
            result
        }
    });

    // 等待屏障释放，确保第一个任务真正开始执行
    barrier.wait().await;
    // 额外等待一点时间确保任务获取了许可
    tokio::time::sleep(Duration::from_millis(10)).await;

    // 提交高优先级任务（需要等待）
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
            
            // 高优先级任务完成后记录
            order.lock().await.push("high_done");
            result
        }
    });

    // 提交另一个低优先级任务（应该在高优先级之后）
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
            
            // 低优先级任务完成后记录
            order.lock().await.push("low2_done");
            result
        }
    });

    // 等待所有任务完成
    let _ = low_handle.await.unwrap();
    let _ = high_handle.await.unwrap();
    let _ = low2_handle.await.unwrap();

    // 验证完成顺序
    // 由于 max_concurrent=1，任务必须串行执行
    // 第一个低优先级任务先执行（已占用许可）
    // 高优先级和第二个低优先级竞争下一个许可
    // 如果优先级有效，高优先级应该先于第二个低优先级完成
    let order = completion_order.lock().await;
    
    // 第一个完成的应该是第一个低优先级任务
    assert_eq!(order[0], "low_done", "第一个低优先级任务应该先完成（它先拿到许可）");
    
    // 关键验证：高优先级应该在第二个低优先级之前完成
    // 这验证了优先级调度的效果（高优先级插队成功）
    let high_idx = order.iter().position(|x| x == &"high_done");
    let low2_idx = order.iter().position(|x| x == &"low2_done");
    
    assert!(
        high_idx < low2_idx,
        "高优先级应该在第二个低优先级之前完成（优先级调度）"
    );
}

/// 测试 2: 验证共享许可池（总并发 = max_concurrent）
#[tokio::test]
async fn test_shared_permit_pool() {
    let executor = ToolExecutor::builder()
        .with_max_concurrent(10)
        .with_priority_support()
        .build();

    let current_concurrent = Arc::new(AtomicUsize::new(0));
    let max_concurrent = Arc::new(AtomicUsize::new(0));

    let mut handles = Vec::new();
    
    // 提交 30 个任务（10 高 + 10 普通 + 10 低）
    for priority in &[Priority::High, Priority::Normal, Priority::Low] {
        for _ in 0..10 {
            let exec = executor.clone();
            let current = current_concurrent.clone();
            let max = max_concurrent.clone();
            let prio = *priority;
            
            handles.push(tokio::spawn(async move {
                let prev = current.fetch_add(1, Ordering::SeqCst);
                let new_current = prev + 1;
                
                // 更新最大并发数
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
                .with_priority(prio)
                .await_result()
                .await
            }));
        }
    }

    let results = futures_util::future::join_all(handles).await;
    assert!(results.iter().all(|r| r.is_ok()));

    // 验证最大并发不超过 10（共享许可池）
    let observed_max = max_concurrent.load(Ordering::Relaxed);
    assert!(
        observed_max <= 10,
        "最大并发为 {}，期望 <= 10（共享许可池）",
        observed_max
    );
}

/// 测试 3: 验证背压持有到任务完成
#[tokio::test]
async fn test_backpressure_holds_to_completion() {
    let executor = ToolExecutor::builder()
        .with_max_concurrent(10)
        .with_max_pending(5)
        .build();

    let mut handles = Vec::new();
    
    // 启动 5 个任务占用队列许可
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

    tokio::time::sleep(Duration::from_millis(10)).await;

    // 提交第 6 个任务 - 应该等待队列许可
    let start = std::time::Instant::now();
    let result = executor.execute(async move {
        Ok::<_, ExecutionError>(6)
    })
    .await_result()
    .await;

    assert!(result.is_ok());
    // 应该等待了至少一个任务完成
    assert!(start.elapsed() >= Duration::from_millis(50));
}

/// 测试 4: 验证 try_execute 队列满时返回 QueueFull
#[tokio::test]
async fn test_try_execute_queue_full() {
    use tokitai_core::executor::ExecutionErrorKind;

    let executor = ToolExecutor::builder()
        .with_max_concurrent(2)
        .with_max_pending(2)
        .build();

    let mut handles = Vec::new();
    
    // 填满队列
    for _ in 0..4 {
        if let Ok(h) = executor.try_execute(async move {
            tokio::time::sleep(Duration::from_millis(100)).await;
            Ok::<_, ExecutionError>(1)
        }) {
            handles.push(h);
        }
    }

    // 第 5 个任务应该被拒绝
    let result = executor.try_execute(async move {
        Ok::<_, ExecutionError>(1)
    });
    
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind, ExecutionErrorKind::QueueFull);

    // 等待现有任务完成
    for handle in handles {
        let _ = handle.await;
    }
}

/// 测试 5: 验证取消机制
#[tokio::test]
async fn test_cancellation_propagation() {
    let executor = ToolExecutor::default();
    let cancel_token = executor.cancellation_token();

    let handle = tokio::spawn(async move {
        executor.execute(async move {
            tokio::time::sleep(Duration::from_secs(10)).await;
            Ok::<_, ExecutionError>(42)
        })
        .with_cancellation(cancel_token)
        .await_result()
        .await
    });

    tokio::time::sleep(Duration::from_millis(50)).await;
    executor.cancel_all();

    let result = handle.await.unwrap();
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind, tokitai_core::executor::ExecutionErrorKind::Cancelled);
}

/// 测试 6: 高并发压力测试
#[tokio::test]
async fn test_high_concurrency_stress() {
    let executor = ToolExecutor::builder()
        .with_max_concurrent(50)
        .with_max_pending(100)
        .build();

    let mut handles = Vec::new();
    
    // 提交 500 个任务
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

    let results = futures_util::future::join_all(handles).await;
    let success_count = results.iter().filter(|r| r.is_ok()).count();
    
    assert_eq!(success_count, 500, "所有任务应该完成");
}

/// 测试 7: 验证信号量无泄漏（并发执行）
#[tokio::test]
async fn test_semaphore_no_leak() {
    let executor = ToolExecutor::builder()
        .with_max_concurrent(10)
        .build();

    let initial_permits = executor.semaphore.available_permits();

    // 并发执行 1000 个任务（不是串行！）
    let mut handles = Vec::new();
    for _ in 0..1000 {
        let exec = executor.clone();
        handles.push(tokio::spawn(async move {
            exec.execute(async {
                // 模拟一些工作
                tokio::time::sleep(Duration::from_micros(100)).await;
                Ok::<_, ExecutionError>(42)
            })
            .await_result()
            .await
        }));
    }

    // 等待所有任务完成
    let results = futures_util::future::join_all(handles).await;
    assert!(results.iter().all(|r| r.is_ok()), "所有任务应该成功完成");

    // 验证所有许可已归还
    let final_permits = executor.semaphore.available_permits();
    assert_eq!(
        initial_permits, final_permits,
        "信号量许可泄漏！初始：{}, 最终：{}",
        initial_permits, final_permits
    );
}

/// 测试 8: 验证批处理并发限制
#[tokio::test]
async fn test_batch_bounded_concurrency() {
    let executor = ToolExecutor::default();

    let current_concurrent = Arc::new(AtomicUsize::new(0));
    let max_concurrent = Arc::new(AtomicUsize::new(0));

    let tasks: Vec<_> = (0..50).map(|_| {
        let current = current_concurrent.clone();
        let max = max_concurrent.clone();
        async move {
            let prev = current.fetch_add(1, Ordering::SeqCst);
            let new_current = prev + 1;
            
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
            
            tokio::time::sleep(Duration::from_millis(10)).await;
            current.fetch_sub(1, Ordering::SeqCst);
            Ok::<_, ExecutionError>(1)
        }
    }).collect();

    // 使用批处理，限制并发为 10
    let results = executor.execute_batch_bounded(tasks, 10).await;
    
    assert!(results.iter().all(|r| r.is_ok()));
    
    // 验证最大并发不超过 10
    let observed_max = max_concurrent.load(Ordering::Relaxed);
    assert!(
        observed_max <= 10,
        "批处理最大并发为 {}，期望 <= 10",
        observed_max
    );
}

/// 测试 9: 验证真正的优先级调度（使用优先级等待队列）
#[tokio::test]
async fn test_true_priority_scheduling() {
    use tokio::sync::Barrier;

    let executor = ToolExecutor::builder()
        .with_max_concurrent(1)  // 只有 1 个并发
        .with_priority_support()  // 启用真正的优先级调度
        .build();

    let completion_order = Arc::new(Mutex::new(Vec::new()));
    let barrier = Arc::new(Barrier::new(4));  // 4 个任务等待

    // 先提交一个普通优先级任务占用许可
    let normal1_handle = tokio::spawn({
        let exec = executor.clone();
        let order = completion_order.clone();
        let barrier = barrier.clone();
        async move {
            let result = exec.execute(async move {
                barrier.wait().await;  // 等待所有任务准备好
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok::<_, ExecutionError>(1)
            })
            .with_priority(Priority::Normal)
            .await_result()
            .await;
            
            order.lock().await.push("normal1_done");
            result
        }
    });

    // 等待第一个任务真正开始
    barrier.wait().await;
    tokio::time::sleep(Duration::from_millis(10)).await;

    // 提交高优先级任务（应该等待）
    let high_handle = tokio::spawn({
        let exec = executor.clone();
        let order = completion_order.clone();
        async move {
            let result = exec.execute(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok::<_, ExecutionError>(2)
            })
            .with_priority(Priority::High)
            .await_result()
            .await;
            
            order.lock().await.push("high_done");
            result
        }
    });

    // 提交低优先级任务（应该在高优先级之后）
    let low_handle = tokio::spawn({
        let exec = executor.clone();
        let order = completion_order.clone();
        async move {
            let result = exec.execute(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok::<_, ExecutionError>(3)
            })
            .with_priority(Priority::Low)
            .await_result()
            .await;
            
            order.lock().await.push("low_done");
            result
        }
    });

    // 提交另一个普通优先级任务
    let normal2_handle = tokio::spawn({
        let exec = executor.clone();
        let order = completion_order.clone();
        async move {
            let result = exec.execute(async move {
                tokio::time::sleep(Duration::from_millis(100)).await;
                Ok::<_, ExecutionError>(4)
            })
            .with_priority(Priority::Normal)
            .await_result()
            .await;
            
            order.lock().await.push("normal2_done");
            result
        }
    });

    // 等待所有任务完成
    let _ = normal1_handle.await.unwrap();
    let _ = high_handle.await.unwrap();
    let _ = low_handle.await.unwrap();
    let _ = normal2_handle.await.unwrap();

    // 验证完成顺序
    let order = completion_order.lock().await;
    
    // 第一个完成的应该是第一个普通优先级任务（它先拿到许可）
    assert_eq!(order[0], "normal1_done");
    
    // 关键验证：高优先级应该在低优先级和普通优先级 2 之前完成
    let high_idx = order.iter().position(|x| x == &"high_done").unwrap();
    let low_idx = order.iter().position(|x| x == &"low_done").unwrap();
    let normal2_idx = order.iter().position(|x| x == &"normal2_done").unwrap();
    
    assert!(
        high_idx < low_idx && high_idx < normal2_idx,
        "高优先级应该在低优先级和普通优先级 2 之前完成（真正的优先级调度）"
    );
}
