//! 性能基准测试 - 测试工具宏展开性能

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use tokitai::tool;
use tokitai::ToolProvider;

/// 简单工具 - 用于基准测试
#[tool]
pub struct BenchTools;

#[tool]
impl BenchTools {
    /// 简单方法
    #[tool]
    pub fn simple_method(&self, name: String) -> Result<String, tokitai::ToolError> {
        Ok(format!("Hello, {}", name))
    }

    /// 多参数方法
    #[tool]
    pub fn multi_param_method(
        &self,
        name: String,
        age: i32,
        email: Option<String>,
    ) -> Result<String, tokitai::ToolError> {
        Ok(format!("{} is {} years old, email: {:?}", name, age, email))
    }

    /// 带验证的方法
    #[tool(min_length_name = 3, max_length_name = 50, min_age = 0, max_age = 150)]
    pub fn validated_method(&self, name: String, age: i32) -> Result<String, tokitai::ToolError> {
        Ok(format!("Validated: {} is {}", name, age))
    }
}

/// 基准测试：访问工具定义
fn bench_tool_definitions(c: &mut Criterion) {
    c.bench_function("tool_definitions_access", |b| {
        b.iter(|| {
            let tools = black_box(BenchTools::tool_definitions());
            black_box(tools.len());
        })
    });
}

/// 基准测试：工具查找
fn bench_tool_lookup(c: &mut Criterion) {
    c.bench_function("tool_lookup", |b| {
        b.iter(|| {
            let tools = black_box(BenchTools::tool_definitions());
            let _tool = tools.iter().find(|t| t.name == "simple_method");
        })
    });
}

/// 基准测试：Schema 格式化
fn bench_schema_pretty(c: &mut Criterion) {
    c.bench_function("schema_pretty_print", |b| {
        b.iter(|| {
            let tools = black_box(BenchTools::tool_definitions());
            let tool = tools.iter().find(|t| t.name == "simple_method").unwrap();
            let _pretty = black_box(tool.input_schema_pretty().unwrap());
        })
    });
}

/// 基准测试：工具调用
fn bench_tool_call(c: &mut Criterion) {
    c.bench_function("tool_call_simple", |b| {
        b.iter(|| {
            let tools = black_box(BenchTools);
            let _result = tools.call_tool("simple_method", &serde_json::json!({"name": "test"}));
        })
    });
}

/// 基准测试：多参数工具调用
fn bench_tool_call_multi(c: &mut Criterion) {
    c.bench_function("tool_call_multi_param", |b| {
        b.iter(|| {
            let tools = black_box(BenchTools);
            let _result = tools.call_tool(
                "multi_param_method",
                &serde_json::json!({
                    "name": "test",
                    "age": 25,
                    "email": "test@example.com"
                }),
            );
        })
    });
}

/// 基准测试：验证工具调用
fn bench_tool_call_validated(c: &mut Criterion) {
    c.bench_function("tool_call_validated", |b| {
        b.iter(|| {
            let tools = black_box(BenchTools);
            let _result = tools.call_tool(
                "validated_method",
                &serde_json::json!({
                    "name": "test_user",
                    "age": 30
                }),
            );
        })
    });
}

criterion_group!(
    benches,
    bench_tool_definitions,
    bench_tool_lookup,
    bench_schema_pretty,
    bench_tool_call,
    bench_tool_call_multi,
    bench_tool_call_validated,
);

criterion_main!(benches);
