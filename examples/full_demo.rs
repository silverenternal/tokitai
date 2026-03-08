//! 完整功能演示
//! cargo run --example full_demo
//!
//! 演示所有参数属性功能：
//! - @param doc comment 语法
//! - `- `name`: description` 格式支持
//! - #[tool(deprecated)] 标记
//! - #[tool(visible = false)] 隐藏工具
//! - #[tool(tags = [...])] 标签支持
//! - #[tool(return_description = "...")] 返回值描述
//! - #[tool(example_input = "...")] 示例输入
//! - #[tool(replaced_by = "...")] 替代方法

use tokitai::tool;

/// 完整功能演示工具集
pub struct FullDemo;

#[tool]
impl FullDemo {
    /// 使用 @param 语法描述参数
    ///
    /// @param user_id 用户唯一标识
    /// @param name 显示名称
    pub fn method_with_param_docs(
        &self,
        user_id: i32,
        name: String,
    ) -> String {
        format!("User: {} ({})", user_id, name)
    }

    /// 使用 `- `name`: description` 格式描述参数
    ///
    /// - `email`: 用户邮箱地址
    /// - `age`: 用户年龄
    pub fn method_with_dash_param_format(
        &self,
        email: String,
        age: i32,
    ) -> String {
        format!("User {} is {} years old", email, age)
    }

    /// 混合使用 @param 和 #[tool(...)] 方法属性
    ///
    /// @param email 邮箱地址
    #[tool(name = "update_email", desc = "更新用户邮箱")]
    pub fn update_email(
        &self,
        user_id: i32,
        email: String,
    ) -> String {
        format!("User {} email updated to {}", user_id, email)
    }

    /// 使用 Option 类型
    ///
    /// @param required_param 必填参数
    /// @param optional_param 可选参数
    pub fn method_with_option(
        &self,
        required_param: String,
        optional_param: Option<String>,
    ) -> String {
        format!(
            "Required: {}, Optional: {:?}",
            required_param, optional_param
        )
    }

    /// 已废弃的方法
    #[tool(deprecated, replaced_by = "new_method")]
    pub fn old_method(&self) -> String {
        "这是一个已废弃的方法".to_string()
    }

    /// 新替代方法
    #[tool(tags = ["user", "core"], return_description = "操作结果消息", context = "async")]
    pub fn new_method(&self) -> String {
        "这是新的替代方法".to_string()
    }

    /// 内部辅助工具（不暴露给 AI）
    #[tool(visible = false)]
    pub fn internal_helper(&self) -> String {
        "这是一个内部工具，不会出现在 TOOL_DEFINITIONS 中".to_string()
    }

    /// 公开工具
    #[tool(tags = ["demo"], example_input = "{\"name\": \"张三\", \"age\": 30}")]
    pub fn public_tool(&self, _name: String, _age: i32) -> String {
        "这是公开工具".to_string()
    }

    /// 复杂类型参数示例
    ///
    /// @param user_ids 用户 ID 列表
    /// @param position 坐标元组
    pub fn method_with_complex_types(
        &self,
        user_ids: Vec<i32>,
        position: (i32, i32),
    ) -> String {
        format!(
            "Users: {:?}, Position: {:?}",
            user_ids, position
        )
    }
}

fn main() {
    println!("=== Full Demo 功能演示 ===\n");

    let demo = FullDemo;

    // 打印所有工具定义
    println!("📋 工具定义 (TOOL_DEFINITIONS):\n");
    for def in FullDemo::TOOL_DEFINITIONS {
        println!("  工具名称：{}", def.name);
        println!("  描述：{}", def.description);
        println!("  输入 Schema: {}", def.input_schema);
        println!();
    }

    // 演示调用工具
    println!("\n🔧 调用工具演示:\n");

    // 同步调用示例
    match demo.call_tool("method_with_param_docs", &tokitai::json!({
        "user_id": 42,
        "name": "测试用户"
    })) {
        Ok(result) => println!("  method_with_param_docs: {}", result),
        Err(e) => println!("  错误：{}", e),
    }

    match demo.call_tool("new_method", &tokitai::json!({})) {
        Ok(result) => println!("  new_method: {}", result),
        Err(e) => println!("  错误：{}", e),
    }

    match demo.call_tool("public_tool", &tokitai::json!({
        "name": "示例用户",
        "age": 25
    })) {
        Ok(result) => println!("  public_tool: {}", result),
        Err(e) => println!("  错误：{}", e),
    }

    // 尝试调用内部工具（应该失败，因为 visible = false）
    match demo.call_tool("internal_helper", &tokitai::json!({})) {
        Ok(result) => println!("  internal_helper: {}", result),
        Err(_) => println!("  internal_helper: 未找到（预期行为，因为 visible = false）"),
    }

    // 尝试调用已废弃的方法（仍然可以调用）
    match demo.call_tool("old_method", &tokitai::json!({})) {
        Ok(result) => println!("  old_method (deprecated): {}", result),
        Err(_) => println!("  错误：调用失败"),
    }

    println!("\n✅ 演示完成！");
}
