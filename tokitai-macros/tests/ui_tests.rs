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
    // 这个测试验证异步方法目前不支持（编译失败）
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/ui/09_async_method.rs");
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
