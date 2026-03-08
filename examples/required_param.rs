//! 测试参数级属性 - 使用 doc comment 中的特殊语法
//!
//! 演示：使用 @required param_name 标记 Option 类型参数为必需
//!
//! 运行：cargo run --example required_param

use tokitai::tool;

/// 测试 required 属性的工具集
pub struct RequiredParamTools;

#[tool]
impl RequiredParamTools {
    /// 搜索商品
    ///
    /// @param query 搜索关键词
    /// @param category 商品分类
    /// @param max_price 价格上限（普通可选参数）
    /// @required category
    #[tool(name = "search_products", desc = "搜索商品列表")]
    pub fn search(
        &self,
        query: String,
        category: Option<String>,
        max_price: Option<f64>,
    ) -> String {
        format!(
            "搜索：{} - 分类：{:?} - 最高价：{:?}",
            query, category, max_price
        )
    }

    /// 创建订单
    ///
    /// @param product_id 商品 ID
    /// @param quantity 购买数量
    /// @param gift_message 礼品留言（可选）
    /// @required quantity
    /// @param_desc quantity 购买数量，必须大于 0
    #[tool(name = "create_order", desc = "创建新订单")]
    pub fn create_order(
        &self,
        product_id: i32,
        quantity: Option<i32>,
        gift_message: Option<String>,
    ) -> String {
        format!(
            "订单：商品 {} - 数量：{:?} - 留言：{:?}",
            product_id, quantity, gift_message
        )
    }
}

fn main() {
    println!("=== #[tool(required)] 参数级属性测试 ===\n");

    let tools = RequiredParamTools;

    // 打印所有工具定义
    for def in RequiredParamTools::TOOL_DEFINITIONS {
        println!("工具名称：{}", def.name);
        println!("描述：{}", def.description);
        println!("输入 Schema: {}", def.input_schema);
        println!();
    }

    // 验证调用
    println!("=== 调用测试 ===\n");

    // 测试 1: 提供所有必需参数
    println!("测试 1: 提供 category（必需）");
    match tools.call_tool("search_products", &tokitai::json!({
        "query": "笔记本电脑",
        "category": "electronics"  // 必需参数
    })) {
        Ok(result) => println!("  ✅ 成功：{}", result),
        Err(e) => println!("  ❌ 错误：{}", e),
    }

    // 测试 2: 不提供可选参数 max_price
    println!("\n测试 2: 不提供 max_price（可选）");
    match tools.call_tool("search_products", &tokitai::json!({
        "query": "手机",
        "category": "electronics"
    })) {
        Ok(result) => println!("  ✅ 成功：{}", result),
        Err(e) => println!("  ❌ 错误：{}", e),
    }

    // 测试 3: 提供所有参数
    println!("\n测试 3: 提供所有参数");
    match tools.call_tool("search_products", &tokitai::json!({
        "query": "耳机",
        "category": Some("electronics"),
        "max_price": 500.0
    })) {
        Ok(result) => println!("  ✅ 成功：{}", result),
        Err(e) => println!("  ❌ 错误：{}", e),
    }

    // 测试 4: create_order 测试
    println!("\n测试 4: create_order 提供必需参数 quantity");
    match tools.call_tool("create_order", &tokitai::json!({
        "product_id": 12345,
        "quantity": 2  // 必需参数
    })) {
        Ok(result) => println!("  ✅ 成功：{}", result),
        Err(e) => println!("  ❌ 错误：{}", e),
    }

    println!("\n✅ 测试完成！");
}
