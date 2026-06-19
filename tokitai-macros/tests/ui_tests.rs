//! trybuild 测试 - 验证宏展开正确性
//!
//! 运行测试：cargo test -p tokitai-macros --test ui_tests

#[test]
fn test_basic_tool() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/01_basic_tool.rs");
}

#[test]
fn test_tool_with_option() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/02_option_param.rs");
}

#[test]
fn test_tool_with_result() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/03_result_return.rs");
}

#[test]
fn test_custom_tool_attrs() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/04_custom_attrs.rs");
}

#[test]
fn test_tool_skip() {
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/05_skip_method.rs");
}

#[test]
fn test_invalid_tool_name() {
    // 这个测试验证宏对无效工具名称的处理（编译通过但运行时无效）
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/06_invalid_tool_name.rs");
}

#[test]
fn test_missing_self_param() {
    // 这个测试验证宏对缺失 self 参数的编译时错误
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/07_missing_self_param.rs");
}

#[test]
fn test_generic_method() {
    // 这个测试验证宏对泛型方法的编译时错误
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/08_generic_method.rs");
}

#[test]
fn test_async_method() {
    // 异步方法现在被支持（编译通过；修复了 #1）
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/09_async_method.rs");
}

#[test]
fn test_config_nonexistent_method() {
    // 这个测试验证配置宏对不存在方法的处理（编译通过）
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/10_config_nonexistent_method.rs");
}

#[test]
fn test_invalid_validation() {
    // 这个测试验证无效验证表达式的编译时错误
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/11_invalid_validation.rs");
}

#[test]
fn test_param_validation_attrs() {
    // 这个测试验证参数验证属性的编译时错误
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/12_param_validation_attrs.rs");
}

#[test]
fn test_complex_return_types() {
    // 这个测试验证复杂返回类型支持
    let t = trybuild::TestCases::new();
    t.pass("tests/ui/13_complex_return_types.rs");
}

#[test]
fn test_invalid_return_type() {
    // T-001: an unsupported return type (bare function pointer)
    // must fail at the user-written method name, not at the
    // `#[tool]` attribute.
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/14_invalid_return_type.rs");
}

#[test]
fn test_unknown_dialect() {
    // T-012: `#[tool(dialect = "garbage")]` is rejected
    // with `E0030` because the dialect name is not in the
    // closed set of known dialects (`mcp`, `openai-strict`,
    // `anthropic`).
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/15_unknown_dialect.rs");
}

#[test]
fn test_openai_strict_tuple() {
    // T-012: `dialect = "openai-strict"` rejects Rust tuple
    // parameters (which serialize as JSON Schema 2020-12
    // positional tuples, not supported by OpenAI).
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/16_openai_tuple_param.rs");
}

#[test]
fn test_openai_strict_any_param() {
    // T-012: `dialect = "openai-strict"` rejects
    // `Option<serde_json::Value>` because the rendered schema
    // has no explicit `type` on the inner property.
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/17_openai_any_param.rs");
}

#[test]
fn test_anthropic_extra_props() {
    // T-012: `dialect = "anthropic"` rejects a method whose
    // nested object schema has no explicit
    // `additionalProperties: false` declaration.
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/18_anthropic_extra_props.rs");
}

#[test]
fn test_mcp_missing_type() {
    // T-012: `dialect = "mcp"` rejects a `serde_json::Value`
    // parameter because the MCP-2025-06-18 dialect requires
    // every property to declare an explicit JSON Schema type.
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/19_mcp_missing_type.rs");
}

// ---------------------------------------------------------------------------
// T-016: baked-few-shot-example negative cases. Both tests assert
// that the example's types do NOT match the real method signature,
// and rustc reports a type error pointed at the `call!` literal.
// ---------------------------------------------------------------------------
#[test]
fn test_example_baking_wrong_arg() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/example_baking_wrong_arg.rs");
}

#[test]
fn test_example_baking_wrong_result() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/example_baking_wrong_result.rs");
}
