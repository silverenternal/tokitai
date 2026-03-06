//! trybuild 测试 - 验证宏展开正确性

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
