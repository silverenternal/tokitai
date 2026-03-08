//! 测试 JSON Schema 验证属性
//! 测试 one_of, enum_values, pattern, min, max, min_length, max_length 等属性

use tokitai::tool;

#[tool]
pub struct ValidationTools;

#[tool]
impl ValidationTools {
    /// 设置用户角色
    ///
    /// @param role 用户角色（必须是 admin、user 或 guest）
    /// @param priority 优先级（1-5 之间）
    /// @param username 用户名（3-20 个字符）
    /// @param tags 标签列表（1-5 个标签）
    /// @param score 分数（必须是 5 的倍数）
    /// @param email 邮箱（必须符合邮箱格式）
    #[tool(
        one_of_role = ["admin", "user", "guest"],
        min_priority = 1,
        max_priority = 5,
        min_length_username = 3,
        max_length_username = 20,
        min_items_tags = 1,
        max_items_tags = 5,
        multiple_of_score = 5.0,
        pattern_email = "@"
    )]
    pub fn set_user_config(
        &self,
        role: String,
        priority: i32,
        username: String,
        tags: Vec<String>,
        score: f64,
        email: String,
    ) -> Result<String, tokitai::ToolError> {
        Ok(format!(
            "设置用户配置：role={}, priority={}, username={}, tags={:?}, score={}, email={}",
            role, priority, username, tags, score, email
        ))
    }

    /// 创建产品
    ///
    /// @param name 产品名称
    /// @param price 价格（必须是 0.01 的倍数）
    /// @param quantity 数量（必须是 5 的倍数）
    /// @param category 分类（枚举值）
    #[tool(
        multiple_of_price = 0.01,
        multiple_of_quantity = 5,
        enum_values_category = [1, 2, 3, 4, 5]
    )]
    pub fn create_product(
        &self,
        name: String,
        price: f64,
        quantity: i32,
        category: i32,
    ) -> Result<String, tokitai::ToolError> {
        Ok(format!(
            "创建产品：name={}, price={}, quantity={}, category={}",
            name, price, quantity, category
        ))
    }
}

fn main() {
    let tools = ValidationTools;

    println!("=== JSON Schema 验证属性测试 ===\n");

    // 打印所有工具定义
    for def in ValidationTools::TOOL_DEFINITIONS {
        println!("工具：{}", def.name);
        println!("描述：{}", def.description);
        println!("Schema: {}\n", def.input_schema);
    }

    // 测试调用
    println!("=== 测试调用 ===\n");

    let result = tools.call_tool("set_user_config",
        &tokitai::json!({
            "role": "admin",
            "priority": 3,
            "username": "zhangsan",
            "tags": ["vip", "active"],
            "score": 95.0,
            "email": "zhangsan@example.com"
        })
    ).unwrap();
    println!("set_user_config: {}", result);

    let result = tools.call_tool("create_product",
        &tokitai::json!({
            "name": "测试产品",
            "price": 100.0,
            "quantity": 10,
            "category": 1
        })
    ).unwrap();
    println!("create_product: {}", result);
}
