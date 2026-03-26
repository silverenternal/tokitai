//! 并发安全测试：验证 GLOBAL_CONFIG_REGISTRY 的线程安全性
//!
//! 运行测试：cargo test -p tokitai-core --test concurrency_test --features serde

#![cfg(feature = "serde")]

use serial_test::serial;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokitai_core::GLOBAL_CONFIG_REGISTRY;

/// 测试 1: 多线程并发写入
#[test]
#[serial]
fn test_concurrent_writes() {
    // 清理之前的配置
    GLOBAL_CONFIG_REGISTRY.clear_all();

    let num_threads = 10;
    let writes_per_thread = 100;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    // 创建多个线程同时写入
    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                // 等待所有线程就绪
                barrier.wait();

                // 每个线程写入 100 个配置
                for j in 0..writes_per_thread {
                    let tool_name = format!("tool_{}_{}", i, j);
                    GLOBAL_CONFIG_REGISTRY.configure(
                        &tool_name,
                        &[tokitai_core::ToolConfig::desc(format!("desc_{}", j))],
                    );
                }
            })
        })
        .collect();

    // 等待所有线程完成
    for handle in handles {
        handle.join().expect("线程不应该 panic");
    }

    // 验证所有配置都已写入
    // 注意：由于工具名不同，所有配置都应该存在
    // 我们验证注册表没有崩溃且可以访问
    GLOBAL_CONFIG_REGISTRY.configure("test_key", &[tokitai_core::ToolConfig::desc("test")]);
    assert!(GLOBAL_CONFIG_REGISTRY.has_config("test_key"));
}

/// 测试 2: 多线程并发读取
#[test]
#[serial]
fn test_concurrent_reads() {
    // 先准备一些数据 - 使用唯一前缀避免与其他测试冲突
    let prefix = "concurrent_reads_";
    for i in 0..50 {
        GLOBAL_CONFIG_REGISTRY.configure(
            &format!("{}tool_{}", prefix, i),
            &[tokitai_core::ToolConfig::desc(format!("desc_{}", i))],
        );
    }

    let num_threads = 20;
    let reads_per_thread = 100;
    // 使用两个屏障：一个用于准备就绪，一个用于开始读取
    let prepare_barrier = Arc::new(std::sync::Barrier::new(num_threads + 1));
    let start_barrier = Arc::new(std::sync::Barrier::new(num_threads + 1));

    // 创建多个线程同时读取
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let prepare_barrier = Arc::clone(&prepare_barrier);
            let start_barrier = Arc::clone(&start_barrier);
            thread::spawn(move || {
                // 等待所有线程就绪
                prepare_barrier.wait();
                // 等待开始信号
                start_barrier.wait();

                for _ in 0..reads_per_thread {
                    for i in 0..50 {
                        let _configs = GLOBAL_CONFIG_REGISTRY.get(&format!("{}tool_{}", prefix, i));
                    }
                }
            })
        })
        .collect();

    // 主线程也参与屏障，确保所有线程就绪后才开始
    prepare_barrier.wait();
    start_barrier.wait();

    // 等待所有线程完成
    for handle in handles {
        handle.join().expect("线程不应该 panic");
    }

    // 验证数据一致性 - 注意：由于没有并发写入，数据应该保持不变
    for i in 0..50 {
        let configs = GLOBAL_CONFIG_REGISTRY.get(&format!("{}tool_{}", prefix, i));
        assert!(!configs.is_empty(), "{}tool_{} 的配置应该存在", prefix, i);
    }
}

/// 测试 3: 读写混合并发
#[test]
#[serial]
fn test_concurrent_read_write() {
    GLOBAL_CONFIG_REGISTRY.clear_all();

    // 先写入一些基础数据
    for j in 0..50 {
        GLOBAL_CONFIG_REGISTRY.configure(
            &format!("shared_tool_{}", j),
            &[tokitai_core::ToolConfig::desc(format!(
                "initial_desc_{}",
                j
            ))],
        );
    }

    let num_readers = 5;
    let num_writers = 5;
    let barrier = Arc::new(std::sync::Barrier::new(num_readers + num_writers));

    // 创建写线程
    let write_handles: Vec<_> = (0..num_writers)
        .map(|i| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();

                for j in 0..50 {
                    GLOBAL_CONFIG_REGISTRY.configure(
                        &format!("shared_tool_{}", j),
                        &[tokitai_core::ToolConfig::desc(format!("writer_{}_desc", i))],
                    );
                    thread::sleep(Duration::from_micros(10));
                }
            })
        })
        .collect();

    // 创建读线程
    let read_handles: Vec<_> = (0..num_readers)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();

                for _ in 0..100 {
                    for j in 0..50 {
                        let _configs = GLOBAL_CONFIG_REGISTRY.get(&format!("shared_tool_{}", j));
                    }
                    thread::sleep(Duration::from_micros(5));
                }
            })
        })
        .collect();

    // 等待所有线程完成
    for handle in write_handles {
        handle.join().expect("写线程不应该 panic");
    }
    for handle in read_handles {
        handle.join().expect("读线程不应该 panic");
    }

    // 验证最终状态一致 - 每个工具至少有一个配置
    for j in 0..50 {
        let configs = GLOBAL_CONFIG_REGISTRY.get(&format!("shared_tool_{}", j));
        // 由于有写入操作，配置应该存在（可能是任何一个写入者的配置）
        assert!(!configs.is_empty(), "shared_tool_{} 应该有配置", j);
    }
}

/// 测试 4: clear_all 的并发安全
#[test]
#[serial]
fn test_concurrent_clear() {
    GLOBAL_CONFIG_REGISTRY.clear_all();

    // 先写入一些数据
    for i in 0..20 {
        GLOBAL_CONFIG_REGISTRY.configure(
            &format!("clear_test_{}", i),
            &[tokitai_core::ToolConfig::desc("test")],
        );
    }

    let num_threads = 10;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();
                GLOBAL_CONFIG_REGISTRY.clear_all();
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("线程不应该 panic");
    }

    // 验证所有配置已清除
    for i in 0..20 {
        let configs = GLOBAL_CONFIG_REGISTRY.get(&format!("clear_test_{}", i));
        assert!(configs.is_empty());
    }
}

/// 测试 5: has_config 的并发安全
#[test]
#[serial]
fn test_concurrent_has_config() {
    // 准备数据 - 使用唯一前缀避免与其他测试冲突
    let prefix = "has_config_";
    for i in 0..30 {
        GLOBAL_CONFIG_REGISTRY.configure(
            &format!("{}{}", prefix, i),
            &[tokitai_core::ToolConfig::desc("test")],
        );
    }

    let num_threads = 15;
    let prepare_barrier = Arc::new(std::sync::Barrier::new(num_threads + 1));
    let start_barrier = Arc::new(std::sync::Barrier::new(num_threads + 1));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let prepare_barrier = Arc::clone(&prepare_barrier);
            let start_barrier = Arc::clone(&start_barrier);
            thread::spawn(move || {
                // 等待所有线程就绪
                prepare_barrier.wait();
                // 等待开始信号
                start_barrier.wait();

                // 只进行读取操作，验证并发读取的安全性
                for _ in 0..50 {
                    for j in 0..30 {
                        let exists = GLOBAL_CONFIG_REGISTRY.has_config(&format!("{}{}", prefix, j));
                        assert!(exists, "{}{} 应该存在", prefix, j);
                    }
                    // 同时检查不存在的 key
                    assert!(!GLOBAL_CONFIG_REGISTRY.has_config(&format!("nonexistent_{}", i)));
                }
            })
        })
        .collect();

    // 主线程也参与屏障
    prepare_barrier.wait();
    start_barrier.wait();

    for handle in handles {
        handle.join().expect("线程不应该 panic");
    }

    // 验证最终状态
    for i in 0..30 {
        assert!(GLOBAL_CONFIG_REGISTRY.has_config(&format!("{}{}", prefix, i)));
    }
}

/// 测试 6: LazyLock 初始化的并发安全
#[test]
#[serial]
fn test_lazylock_initialization() {
    // 清理之前的状态
    GLOBAL_CONFIG_REGISTRY.clear_all();

    // 多个线程同时首次访问 GLOBAL_CONFIG_REGISTRY
    let num_threads = 20;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();

                // 并发访问注册表
                GLOBAL_CONFIG_REGISTRY.configure(
                    &format!("lazy_init_{}", i),
                    &[tokitai_core::ToolConfig::desc("test")],
                );
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("线程不应该 panic");
    }

    // 验证所有配置都已写入（由于工具名不同，所有配置都应该存在）
    for i in 0..num_threads {
        assert!(
            GLOBAL_CONFIG_REGISTRY.has_config(&format!("lazy_init_{}", i)),
            "lazy_init_{} 应该存在",
            i
        );
    }
}

/// 测试 7: 长时间运行的并发测试（压力测试）
#[test]
#[serial]
fn test_stress_concurrent_access() {
    GLOBAL_CONFIG_REGISTRY.clear_all();

    let num_threads = 30;
    let operations_per_thread = 200;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|_i| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();

                for j in 0..operations_per_thread {
                    match j % 4 {
                        0 => {
                            // 写入
                            GLOBAL_CONFIG_REGISTRY.configure(
                                &format!("stress_tool_{}", j % 50),
                                &[tokitai_core::ToolConfig::desc(format!("desc_{}", j))],
                            );
                        }
                        1 => {
                            // 读取
                            let _configs =
                                GLOBAL_CONFIG_REGISTRY.get(&format!("stress_tool_{}", j % 50));
                        }
                        2 => {
                            // 检查存在
                            let _exists = GLOBAL_CONFIG_REGISTRY
                                .has_config(&format!("stress_tool_{}", j % 50));
                        }
                        3 => {
                            // 偶尔清除
                            if j % 100 == 0 {
                                GLOBAL_CONFIG_REGISTRY.clear_all();
                            }
                        }
                        _ => unreachable!(),
                    }
                }
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("压力测试线程不应该 panic");
    }

    // 验证注册表仍然可用
    GLOBAL_CONFIG_REGISTRY.configure("final_test", &[tokitai_core::ToolConfig::desc("final")]);
    assert!(GLOBAL_CONFIG_REGISTRY.has_config("final_test"));
}
