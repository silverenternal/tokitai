//! 集成测试：alias、cache 和 rate_limit 功能

use serde_json::json;
use tokitai::tool;
use tokitai::ToolProvider;

#[tool]
pub struct IntegrationTools;

#[tool]
impl IntegrationTools {
    /// 创建用户（带别名）
    ///
    /// @param name 用户名
    /// @param email 邮箱
    #[tool(alias = ["create_user_account", "add_user", "register_user"])]
    pub fn create_user(&self, name: String, email: String) -> Result<String, tokitai::ToolError> {
        Ok(format!("创建用户：{} (邮箱：{})", name, email))
    }

    /// 获取缓存数据
    ///
    /// @param key 缓存键
    #[tool(cache = "ttl=3600", rate_limit = "100/min")]
    pub fn get_cached_data(&self, key: String) -> Result<String, tokitai::ToolError> {
        Ok(format!("缓存数据：{}", key))
    }

    /// 组合功能测试
    ///
    /// @param query 查询字符串
    #[tool(
        alias = ["search_items", "find_products"],
        cache = "ttl=300",
        rate_limit = "50/hour"
    )]
    pub fn search(&self, query: String) -> Result<String, tokitai::ToolError> {
        Ok(format!("搜索结果：{}", query))
    }
}

#[test]
fn test_tool_alias_main_name() {
    let tools = IntegrationTools;

    // 测试主名称调用
    let result = tools
        .call_tool(
            "create_user",
            &json!({
                "name": "张三",
                "email": "zhangsan@example.com"
            }),
        )
        .unwrap();

    assert!(result.to_string().contains("创建用户：张三"));
}

#[test]
fn test_tool_alias_first() {
    let tools = IntegrationTools;

    // 测试别名 1 调用
    let result = tools
        .call_tool(
            "create_user_account",
            &json!({
                "name": "李四",
                "email": "lisi@example.com"
            }),
        )
        .unwrap();

    assert!(result.to_string().contains("创建用户：李四"));
}

#[test]
fn test_tool_alias_second() {
    let tools = IntegrationTools;

    // 测试别名 2 调用
    let result = tools
        .call_tool(
            "add_user",
            &json!({
                "name": "王五",
                "email": "wangwu@example.com"
            }),
        )
        .unwrap();

    assert!(result.to_string().contains("创建用户：王五"));
}

#[test]
fn test_tool_alias_third() {
    let tools = IntegrationTools;

    // 测试别名 3 调用
    let result = tools
        .call_tool(
            "register_user",
            &json!({
                "name": "赵六",
                "email": "zhaoliu@example.com"
            }),
        )
        .unwrap();

    assert!(result.to_string().contains("创建用户：赵六"));
}

#[test]
fn test_cache_and_rate_limit_in_schema() {
    let tools = IntegrationTools::tool_definitions();

    // 查找 get_cached_data 工具
    let tool = tools.iter().find(|t| t.name == "get_cached_data").unwrap();

    // 解析 schema
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();

    // 验证 x-cache 和 x-rate-limit 扩展字段
    assert_eq!(schema["x-cache"].as_str().unwrap(), "ttl=3600");
    assert_eq!(schema["x-rate-limit"].as_str().unwrap(), "100/min");
}

#[test]
fn test_combined_features_in_schema() {
    let tools = IntegrationTools::tool_definitions();

    // 查找 search 工具
    let tool = tools.iter().find(|t| t.name == "search").unwrap();

    // 解析 schema
    let schema: serde_json::Value = serde_json::from_str(&tool.input_schema).unwrap();

    // 验证 x-cache 和 x-rate-limit 扩展字段
    assert_eq!(schema["x-cache"].as_str().unwrap(), "ttl=300");
    assert_eq!(schema["x-rate-limit"].as_str().unwrap(), "50/hour");
}

#[test]
fn test_search_alias() {
    let tools = IntegrationTools;

    // 测试 search 的别名
    let result = tools
        .call_tool(
            "search_items",
            &json!({
                "query": "笔记本电脑"
            }),
        )
        .unwrap();

    assert!(result.to_string().contains("搜索结果：笔记本电脑"));
}

#[test]
fn test_search_alias_second() {
    let tools = IntegrationTools;

    // 测试 search 的第二个别名
    let result = tools
        .call_tool(
            "find_products",
            &json!({
                "query": "手机"
            }),
        )
        .unwrap();

    assert!(result.to_string().contains("搜索结果：手机"));
}
