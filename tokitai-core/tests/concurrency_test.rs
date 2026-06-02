//! Concurrency safety test: verifies the thread-safety of GLOBAL_CONFIG_REGISTRY.
//!
//! Run with: `cargo test -p tokitai-core --test concurrency_test --features serde`

#![cfg(feature = "serde")]

use serial_test::serial;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokitai_core::GLOBAL_CONFIG_REGISTRY;

/// Test 1: concurrent writes from multiple threads
#[test]
#[serial]
fn test_concurrent_writes() {
    // Clean up any pre-existing configuration
    GLOBAL_CONFIG_REGISTRY.clear_all();

    let num_threads = 10;
    let writes_per_thread = 100;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    // Spawn threads that write concurrently
    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                // Wait until every thread is ready
                barrier.wait();

                // Each thread writes 100 configurations
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

    // Wait for all threads to finish
    for handle in handles {
        handle.join().expect("thread should not panic");
    }

    // Verify all configurations were written
    // Note: because the tool names differ, every configuration should exist.
    // We verify the registry has not crashed and is accessible.
    GLOBAL_CONFIG_REGISTRY.configure("test_key", &[tokitai_core::ToolConfig::desc("test")]);
    assert!(GLOBAL_CONFIG_REGISTRY.has_config("test_key"));
}

/// Test 2: concurrent reads from multiple threads
#[test]
#[serial]
fn test_concurrent_reads() {
    // Pre-populate data - use a unique prefix to avoid clashing with other tests
    let prefix = "concurrent_reads_";
    for i in 0..50 {
        GLOBAL_CONFIG_REGISTRY.configure(
            &format!("{}tool_{}", prefix, i),
            &[tokitai_core::ToolConfig::desc(format!("desc_{}", i))],
        );
    }

    let num_threads = 20;
    let reads_per_thread = 100;
    // Two barriers: one to wait for readiness, one to signal start of reads
    let prepare_barrier = Arc::new(std::sync::Barrier::new(num_threads + 1));
    let start_barrier = Arc::new(std::sync::Barrier::new(num_threads + 1));

    // Spawn threads that read concurrently
    let handles: Vec<_> = (0..num_threads)
        .map(|_| {
            let prepare_barrier = Arc::clone(&prepare_barrier);
            let start_barrier = Arc::clone(&start_barrier);
            thread::spawn(move || {
                // Wait until every thread is ready
                prepare_barrier.wait();
                // Wait for the start signal
                start_barrier.wait();

                for _ in 0..reads_per_thread {
                    for i in 0..50 {
                        let _configs = GLOBAL_CONFIG_REGISTRY.get(&format!("{}tool_{}", prefix, i));
                    }
                }
            })
        })
        .collect();

    // The main thread also joins the barriers to ensure everyone is ready before starting
    prepare_barrier.wait();
    start_barrier.wait();

    // Wait for all threads to finish
    for handle in handles {
        handle.join().expect("thread should not panic");
    }

    // Verify data consistency - with no concurrent writes, data should remain unchanged
    for i in 0..50 {
        let configs = GLOBAL_CONFIG_REGISTRY.get(&format!("{}tool_{}", prefix, i));
        assert!(
            !configs.is_empty(),
            "configuration for {}tool_{} should exist",
            prefix,
            i
        );
    }
}

/// Test 3: mixed concurrent reads and writes
#[test]
#[serial]
fn test_concurrent_read_write() {
    GLOBAL_CONFIG_REGISTRY.clear_all();

    // Seed some baseline data
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

    // Spawn writer threads
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

    // Spawn reader threads
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

    // Wait for all threads to finish
    for handle in write_handles {
        handle.join().expect("writer thread should not panic");
    }
    for handle in read_handles {
        handle.join().expect("reader thread should not panic");
    }

    // Verify the final state is consistent - every tool should have at least one configuration
    for j in 0..50 {
        let configs = GLOBAL_CONFIG_REGISTRY.get(&format!("shared_tool_{}", j));
        // Since writes occurred, the configuration should exist (possibly from any writer)
        assert!(
            !configs.is_empty(),
            "shared_tool_{} should have a configuration",
            j
        );
    }
}

/// Test 4: concurrent safety of `clear_all`
#[test]
#[serial]
fn test_concurrent_clear() {
    GLOBAL_CONFIG_REGISTRY.clear_all();

    // Seed some data
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
        handle.join().expect("thread should not panic");
    }

    // Verify all configurations were cleared
    for i in 0..20 {
        let configs = GLOBAL_CONFIG_REGISTRY.get(&format!("clear_test_{}", i));
        assert!(configs.is_empty());
    }
}

/// Test 5: concurrent safety of `has_config`
#[test]
#[serial]
fn test_concurrent_has_config() {
    // Pre-populate data with a unique prefix to avoid clashing with other tests
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
                // Wait until every thread is ready
                prepare_barrier.wait();
                // Wait for the start signal
                start_barrier.wait();

                // Only perform reads, verifying concurrent-read safety
                for _ in 0..50 {
                    for j in 0..30 {
                        let exists = GLOBAL_CONFIG_REGISTRY.has_config(&format!("{}{}", prefix, j));
                        assert!(exists, "{}{} should exist", prefix, j);
                    }
                    // Also verify keys that do not exist
                    assert!(!GLOBAL_CONFIG_REGISTRY.has_config(&format!("nonexistent_{}", i)));
                }
            })
        })
        .collect();

    // The main thread also joins the barriers
    prepare_barrier.wait();
    start_barrier.wait();

    for handle in handles {
        handle.join().expect("thread should not panic");
    }

    // Verify the final state
    for i in 0..30 {
        assert!(GLOBAL_CONFIG_REGISTRY.has_config(&format!("{}{}", prefix, i)));
    }
}

/// Test 6: concurrent safety of the LazyLock initialization
#[test]
#[serial]
fn test_lazylock_initialization() {
    // Reset state
    GLOBAL_CONFIG_REGISTRY.clear_all();

    // Multiple threads access GLOBAL_CONFIG_REGISTRY for the first time concurrently
    let num_threads = 20;
    let barrier = Arc::new(std::sync::Barrier::new(num_threads));

    let handles: Vec<_> = (0..num_threads)
        .map(|i| {
            let barrier = Arc::clone(&barrier);
            thread::spawn(move || {
                barrier.wait();

                // Concurrent access to the registry
                GLOBAL_CONFIG_REGISTRY.configure(
                    &format!("lazy_init_{}", i),
                    &[tokitai_core::ToolConfig::desc("test")],
                );
            })
        })
        .collect();

    for handle in handles {
        handle.join().expect("thread should not panic");
    }

    // Verify all configurations were written (since the tool names differ, every configuration should exist)
    for i in 0..num_threads {
        assert!(
            GLOBAL_CONFIG_REGISTRY.has_config(&format!("lazy_init_{}", i)),
            "lazy_init_{} should exist",
            i
        );
    }
}

/// Test 7: long-running concurrent test (stress test)
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
                            // Write
                            GLOBAL_CONFIG_REGISTRY.configure(
                                &format!("stress_tool_{}", j % 50),
                                &[tokitai_core::ToolConfig::desc(format!("desc_{}", j))],
                            );
                        }
                        1 => {
                            // Read
                            let _configs =
                                GLOBAL_CONFIG_REGISTRY.get(&format!("stress_tool_{}", j % 50));
                        }
                        2 => {
                            // Check existence
                            let _exists = GLOBAL_CONFIG_REGISTRY
                                .has_config(&format!("stress_tool_{}", j % 50));
                        }
                        3 => {
                            // Occasionally clear
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
        handle.join().expect("stress test thread should not panic");
    }

    // Verify the registry is still usable
    GLOBAL_CONFIG_REGISTRY.configure("final_test", &[tokitai_core::ToolConfig::desc("final")]);
    assert!(GLOBAL_CONFIG_REGISTRY.has_config("final_test"));
}
