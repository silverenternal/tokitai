//! 真实世界性能基准测试
//!
//! 测量用户感知的性能指标：
//! - 端到端工具调用延迟
//! - 工具定义生成时间
//! - 编译时间（通过外部脚本测量）

use criterion::{black_box, criterion_group, criterion_main, BenchmarkId, Criterion};
use serde_json::json;
use tokitai::tool;
use tokitai::ToolProvider;

/// 真实世界工具 - 模拟实际使用场景
#[tool]
pub struct RealWorldTools;

#[tool]
impl RealWorldTools {
    /// 处理用户请求
    #[tool]
    pub fn process_user_request(
        &self,
        user_id: i32,
        action: String,
        payload: Option<String>,
    ) -> Result<String, tokitai::ToolError> {
        Ok(format!("User {} performed {}: {:?}", user_id, action, payload))
    }

    /// 计算统计数据
    pub fn calculate_stats(&self, numbers: Vec<f64>) -> Result<String, tokitai::ToolError> {
        let sum: f64 = numbers.iter().sum();
        let count = numbers.len();
        let avg = if count > 0 { sum / count as f64 } else { 0.0 };
        Ok(format!("Sum: {}, Count: {}, Avg: {}", sum, count, avg))
    }

    /// 验证并处理数据
    #[tool(min_length_data = 1, max_length_data = 1000)]
    pub fn validate_and_process(&self, data: String) -> Result<String, tokitai::ToolError> {
        Ok(format!("Processed {} characters", data.len()))
    }
}

/// 端到端工具调用基准测试
/// 这测量了用户实际调用工具的完整流程时间
fn bench_end_to_end_tool_call(c: &mut Criterion) {
    let tools = RealWorldTools;

    c.bench_function("e2e_simple_tool_call", |b| {
        b.iter(|| {
            let result = tools.call_tool(
                "process_user_request",
                &json!({
                    "user_id": 123,
                    "action": "create",
                    "payload": "test data"
                }),
            );
            assert!(result.is_ok());
            black_box(result.unwrap());
        })
    });
}

/// 多参数端到端调用
fn bench_end_to_end_multi_param(c: &mut Criterion) {
    let tools = RealWorldTools;

    c.bench_function("e2e_multi_param_tool_call", |b| {
        b.iter(|| {
            let result = tools.call_tool(
                "process_user_request",
                &json!({
                    "user_id": 456,
                    "action": "update",
                    "payload": null
                }),
            );
            assert!(result.is_ok());
            black_box(result.unwrap());
        })
    });
}

/// 大数据量工具调用
fn bench_end_to_end_large_payload(c: &mut Criterion) {
    let tools = RealWorldTools;
    let large_data = "x".repeat(1000);

    c.bench_function("e2e_large_payload_tool_call", |b| {
        b.iter(|| {
            let result = tools.call_tool(
                "validate_and_process",
                &json!({
                    "data": large_data
                }),
            );
            assert!(result.is_ok());
            black_box(result.unwrap());
        })
    });
}

/// 数组处理性能
fn bench_end_to_end_array_processing(c: &mut Criterion) {
    let tools = RealWorldTools;
    let numbers: Vec<f64> = (0..100).map(|i| i as f64).collect();

    c.bench_function("e2e_array_processing", |b| {
        b.iter(|| {
            let result = tools.call_tool(
                "calculate_stats",
                &json!({
                    "numbers": numbers
                }),
            );
            assert!(result.is_ok());
            black_box(result.unwrap());
        })
    });
}

/// 工具定义访问（与原有基准保持一致）
fn bench_tool_definitions_access(c: &mut Criterion) {
    c.bench_function("tool_definitions_access", |b| {
        b.iter(|| {
            let tools = black_box(RealWorldTools::tool_definitions());
            black_box(tools.len());
        })
    });
}

/// 工具查找性能
fn bench_tool_lookup(c: &mut Criterion) {
    c.bench_function("tool_lookup", |b| {
        b.iter(|| {
            let tools = black_box(RealWorldTools::tool_definitions());
            let _tool = tools.iter().find(|t| t.name == "process_user_request");
        })
    });
}

/// Schema 格式化性能
fn bench_schema_pretty(c: &mut Criterion) {
    c.bench_function("schema_pretty_print", |b| {
        b.iter(|| {
            let tools = black_box(RealWorldTools::tool_definitions());
            let tool = tools
                .iter()
                .find(|t| t.name == "process_user_request")
                .unwrap();
            let _pretty = black_box(tool.input_schema_pretty().unwrap());
        })
    });
}

/// 并发工具调用（模拟真实服务器场景）
fn bench_concurrent_tool_calls(c: &mut Criterion) {
    use std::sync::Arc;
    use std::thread;

    let tools = Arc::new(RealWorldTools);
    let mut group = c.benchmark_group("concurrent_calls");

    for concurrency in [1, 4, 16].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(concurrency),
            concurrency,
            |b, &concurrency| {
                b.iter(|| {
                    let mut handles = vec![];
                    for i in 0..concurrency {
                        let tools_clone = Arc::clone(&tools);
                        let handle = thread::spawn(move || {
                            tools_clone.call_tool(
                                "process_user_request",
                                &json!({
                                    "user_id": i,
                                    "action": "concurrent_test",
                                    "payload": null
                                }),
                            )
                        });
                        handles.push(handle);
                    }
                    for handle in handles {
                        let result = handle.join().unwrap();
                        assert!(result.is_ok());
                        black_box(result.unwrap());
                    }
                })
            },
        );
    }
    group.finish();
}

criterion_group!(
    benches,
    bench_tool_definitions_access,
    bench_tool_lookup,
    bench_schema_pretty,
    bench_end_to_end_tool_call,
    bench_end_to_end_multi_param,
    bench_end_to_end_large_payload,
    bench_end_to_end_array_processing,
    bench_concurrent_tool_calls,
);

criterion_main!(benches);
