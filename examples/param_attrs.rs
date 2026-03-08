//! 参数描述示例
//!
//! 演示使用 /// @param 语法提取参数描述
//!
//! 运行：cargo run --example param_attrs

use tokitai::tool;

/// 参数属性测试工具集
pub struct ParamAttrTools;

#[tool]
impl ParamAttrTools {
    /// 处理用户信息
    ///
    /// 这个函数展示了使用 @param 语法描述参数
    ///
    /// @param user_id 用户唯一标识符，必须是正整数
    /// @param name 显示名称，可选参数
    /// @param email 用户的电子邮件地址
    /// @param note 备注信息
    #[tool(name = "process_user", desc = "处理用户信息")]
    pub fn process_user(
        &self,
        user_id: i32,
        name: Option<String>,
        email: String,
        note: Option<String>,
    ) -> String {
        format!(
            "处理用户：{} ({}) - {} - note: {}",
            user_id,
            name.unwrap_or_else(|| "匿名".to_string()),
            email,
            note.unwrap_or_else(|| "无备注".to_string())
        )
    }

    /// 搜索商品
    ///
    /// @param query 搜索关键词
    /// @param category 商品分类
    /// @param max_price 价格上限
    /// @param tags 标签列表
    #[tool(name = "search_products", desc = "搜索商品列表")]
    pub fn search(
        &self,
        query: String,
        category: Option<String>,
        max_price: Option<f64>,
        tags: Vec<String>,
    ) -> String {
        format!(
            "搜索：{} - 分类：{:?} - 最高价：{:?} - 标签：{:?}",
            query, category, max_price, tags
        )
    }

    /// 更新配置
    ///
    /// @param key 配置键名
    /// @param value 配置值
    /// @param force 是否强制更新
    pub fn update_config(
        &self,
        key: String,
        value: String,
        force: bool,
    ) -> String {
        format!("更新配置：{} = {} (force: {})", key, value, force)
    }
}

fn main() {
    let tools = ParamAttrTools;

    println!("=== 参数描述示例 (@param 语法) ===\n");
    
    // 打印所有工具定义
    for def in ParamAttrTools::TOOL_DEFINITIONS {
        println!("工具名称：{}", def.name);
        println!("描述：{}", def.description);
        println!("输入 Schema: {}", def.input_schema);
        println!();
    }
    
    // 演示调用
    println!("=== 演示调用 ===\n");
    
    // 同步调用示例
    match tools.call_tool("process_user", &tokitai::json!({
        "user_id": 12345,
        "name": "张三",
        "email": "zhangsan@example.com",
        "note": "VIP 客户"
    })) {
        Ok(result) => println!("process_user 结果：{}", result),
        Err(e) => println!("错误：{:?}", e),
    }

    match tools.call_tool("search_products", &tokitai::json!({
        "query": "笔记本电脑",
        "category": "electronics",
        "max_price": 8000.0,
        "tags": ["hot", "sale"]
    })) {
        Ok(result) => println!("search_products 结果：{}", result),
        Err(e) => println!("错误：{:?}", e),
    }

    match tools.call_tool("update_config", &tokitai::json!({
        "key": "app.theme",
        "value": "dark",
        "force": true
    })) {
        Ok(result) => println!("update_config 结果：{}", result),
        Err(e) => println!("错误：{:?}", e),
    }
}
