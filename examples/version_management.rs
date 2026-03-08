//! 版本管理示例：展示如何使用 version、deprecated 等属性

use tokitai::tool;

#[tool]
pub struct VersionedTools;

#[tool]
impl VersionedTools {
    /// 获取用户信息（旧版本，已弃用）
    #[tool(
        version = "0.1.0",
        deprecated = true,
        deprecated_since = "0.3.0",
        remove_in = "1.0.0",
        replaced_by = "get_user_info_v2"
    )]
    pub fn get_user_info(&self, user_id: String) -> Result<String, tokitai::ToolError> {
        Ok(format!("用户信息：{}", user_id))
    }

    /// 获取用户信息（新版本）
    #[tool(version = "0.3.0")]
    pub fn get_user_info_v2(&self, user_id: String, include_details: bool) -> Result<String, tokitai::ToolError> {
        Ok(format!("用户信息 v2：{} (详情：{})", user_id, include_details))
    }

    /// 计算加法（原始版本）
    #[tool(
        version = "0.1.0",
        deprecated = true,
        deprecated_since = "0.2.0",
        remove_in = "0.5.0"
    )]
    pub fn add(&self, a: i32, b: i32) -> Result<i32, tokitai::ToolError> {
        Ok(a + b)
    }

    /// 计算加法（新版本，支持多个数）
    #[tool(version = "0.2.0")]
    pub fn add_multi(&self, numbers: Vec<i32>) -> Result<i32, tokitai::ToolError> {
        Ok(numbers.iter().sum())
    }
}

fn main() {
    println!("=== 版本管理示例 ===\n");

    for def in VersionedTools::TOOL_DEFINITIONS {
        println!("工具：{}", def.name);
        println!("  描述：{}", def.description);
        println!("  版本：{:?}", def.version);
        println!("  废弃于：{:?}", def.deprecated_since);
        println!("  移除于：{:?}", def.remove_in);
        println!("  替代者：{:?}", def.replaced_by);
        println!();
    }

    // 演示工具调用
    println!("=== 工具调用演示 ===\n");

    let tools = VersionedTools;

    // 调用新方法
    match tools.call_tool("get_user_info_v2", &tokitai::json!({
        "user_id": "user123",
        "include_details": true
    })) {
        Ok(result) => println!("get_user_info_v2 结果：{}", result),
        Err(e) => println!("错误：{:?}", e),
    }

    // 调用 add_multi
    match tools.call_tool("add_multi", &tokitai::json!({
        "numbers": [1, 2, 3, 4, 5]
    })) {
        Ok(result) => println!("add_multi 结果：{}", result),
        Err(e) => println!("错误：{:?}", e),
    }
}
