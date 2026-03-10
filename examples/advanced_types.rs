//! 高级类型支持示例
//!
//! 展示 Tokitai 对复杂类型的支持：
//! - 嵌套 Option 类型
//! - Vec 数组元素类型
//! - HashMap
//! - 参数 doc comment 提取
//! - 参数属性（验证、转换、别名）
//!
//! 运行：cargo run --example advanced_types

use std::collections::HashMap;
use tokitai::tool;
use tokitai::ToolProvider;

/// 高级工具服务
pub struct AdvancedTools;

#[tool]
impl AdvancedTools {
    /// 处理用户信息
    ///
    /// 支持可选参数和复杂类型
    ///
    /// - `user_id`: 用户 ID
    /// - `name`: 用户名称
    /// - `email`: 可选的邮箱地址
    /// - `tags`: 用户的标签列表
    pub fn process_user(
        &self,
        user_id: i32,
        name: String,
        _email: Option<String>,
        tags: Vec<String>,
    ) -> String {
        format!("处理用户 {} ({}): 标签数 = {}", user_id, name, tags.len())
    }

    /// 计算数值统计
    ///
    /// - `numbers`: 要分析的数值列表
    /// - `include_median`: 是否包含中位数
    pub fn calculate_stats(
        &self,
        numbers: Vec<f64>,
        include_median: Option<bool>,
    ) -> HashMap<String, f64> {
        let mut stats = HashMap::new();

        if !numbers.is_empty() {
            let sum: f64 = numbers.iter().sum();
            let count = numbers.len() as f64;
            stats.insert("sum".to_string(), sum);
            stats.insert("count".to_string(), count);
            stats.insert("average".to_string(), sum / count);

            if include_median.unwrap_or(false) {
                let mut sorted = numbers.clone();
                sorted.sort_by(|a, b| a.partial_cmp(b).unwrap());
                let mid = sorted.len() / 2;
                let median = if sorted.len().is_multiple_of(2) {
                    (sorted[mid - 1] + sorted[mid]) / 2.0
                } else {
                    sorted[mid]
                };
                stats.insert("median".to_string(), median);
            }
        }

        stats
    }

    /// 处理嵌套可选参数
    ///
    /// - `id`: 必需的 ID
    /// - `name`: 可选的名称
    /// - `count`: 可选的数量
    /// - `note`: 双重可选
    pub fn process_optional(
        &self,
        id: i32,
        name: Option<String>,
        count: Option<i32>,
        note: Option<Option<String>>,
    ) -> String {
        format!(
            "ID: {}, Name: {:?}, Count: {:?}, Note: {:?}",
            id, name, count, note
        )
    }

    /// 处理数组引用
    ///
    /// - `data`: 数据切片
    pub fn process_slice(&self, data: Vec<i32>) -> i32 {
        data.iter().sum()
    }

    /// 处理元组参数（简化为数组）
    ///
    /// - `coordinates`: 坐标数据
    pub fn process_tuple_data(&self, coordinates: (i32, i32)) -> String {
        format!("坐标：({}, {})", coordinates.0, coordinates.1)
    }

    // ========== 参数属性功能演示 ==========

    /// 创建用户（自动验证版）
    ///
    /// 演示使用 doc comment 语法进行自动验证和转换
    ///
    /// @param name 用户名（不能为空）
    /// @validate name !value.is_empty()
    /// @param email 用户邮箱
    /// @param age 用户年龄（必须在 0 到 150 之间）
    /// @validate age value > 0 && value < 150
    /// @required name
    /// @required age
    #[tool(alias = ["create_user_account", "add_user"])]
    pub fn create_user(
        &self,
        name: String,
        email: String,
        age: i32,
    ) -> Result<String, tokitai::ToolError> {
        // 验证由宏自动生成
        Ok(format!(
            "创建用户：{} (邮箱：{}, 年龄：{})",
            name, email, age
        ))
    }

    /// 获取用户
    ///
    /// @param user_id 用户 ID
    #[tool(alias = ["get_user_info", "fetch_user"])]
    pub fn get_user(&self, user_id: i32) -> Result<String, tokitai::ToolError> {
        Ok(format!("获取用户：{}", user_id))
    }

    /// 参数属性测试
    ///
    /// 演示三种工具描述方式和参数级属性
    ///
    /// @param name 用户姓名
    /// @param age 用户年龄
    /// @param email 邮箱地址
    #[tool(
        desc = "自定义方法描述，演示参数属性功能",
        tags = ["demo", "test"],
        example_name = "张三",
        min_length_name = 1,
        max_length_name = 50,
        min_age = 0,
        max_age = 150,
        example_email = "test@example.com"
    )]
    pub fn method_with_param_attrs(
        &self,
        name: String,
        _age: i32,
        email: Option<String>,
    ) -> String {
        format!("{} <{}>", name, email.unwrap_or_default())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== 高级类型支持示例 ===\n");

    let tools = AdvancedTools;

    // ========== 第一部分：高级类型支持 ==========
    println!("【第一部分】高级类型支持\n");

    // 展示工具定义
    println!("1. 工具定义（查看生成的 JSON Schema）");
    for tool in AdvancedTools::tool_definitions() {
        println!("\n   工具：{}", tool.name);
        println!("   描述：{}", tool.description);
        println!("   Schema: {}", tool.input_schema);
    }
    println!();

    // 测试 process_user
    println!("2. 测试 process_user");
    let result = tools.call_tool(
        "process_user",
        &tokitai::json!({
            "user_id": 123,
            "name": "Alice",
            "email": "alice@example.com",
            "tags": ["admin", "active"]
        }),
    )?;
    println!("   结果：{}", result);
    println!();

    // 测试 calculate_stats
    println!("3. 测试 calculate_stats");
    let result = tools.call_tool(
        "calculate_stats",
        &tokitai::json!({
            "numbers": [1.0, 2.0, 3.0, 4.0, 5.0],
            "include_median": true
        }),
    )?;
    println!("   统计结果：{}", result);
    println!();

    // 测试可选参数
    println!("4. 测试可选参数");
    let result = tools.call_tool(
        "process_optional",
        &tokitai::json!({
            "id": 42,
            "name": "Test",
            "count": null,
            "note": null
        }),
    )?;
    println!("   结果：{}", result);
    println!();

    // 测试切片求和
    println!("5. 测试 process_slice");
    let result = tools.call_tool(
        "process_slice",
        &tokitai::json!({
            "data": [1, 2, 3, 4, 5]
        }),
    )?;
    println!("   求和结果：{}", result);
    println!();

    // ========== 第二部分：参数属性功能 ==========
    println!("\n【第二部分】参数属性功能（验证、转换、别名）\n");

    // 测试 create_user（带验证和转换）
    println!("6. 测试 create_user（带验证）");

    // 测试 1: 正常调用
    let args = tokitai::json!({
        "name": "张三",
        "email": "ZHANGSAN@EXAMPLE.COM",
        "age": 25
    });
    match tools.call_tool("create_user", &args) {
        Ok(result) => println!("   成功：{}", result),
        Err(e) => println!("   错误：{}", e),
    }

    // 测试 2: 使用别名
    let args = tokitai::json!({
        "name": "李四",
        "email": "LISI@EXAMPLE.COM",
        "age": 30
    });
    match tools.call_tool("create_user_account", &args) {
        Ok(result) => println!("   使用别名成功：{}", result),
        Err(e) => println!("   错误：{}", e),
    }

    // 测试 3: 验证失败 - 空名字
    let args = tokitai::json!({
        "name": "",
        "email": "test@example.com",
        "age": 25
    });
    match tools.call_tool("create_user", &args) {
        Ok(result) => println!("   成功：{}", result),
        Err(e) => println!("   验证失败（预期）：{}", e),
    }

    // 测试 4: 验证失败 - 年龄超出范围
    let args = tokitai::json!({
        "name": "赵六",
        "email": "zhaoliu@example.com",
        "age": 200
    });
    match tools.call_tool("create_user", &args) {
        Ok(result) => println!("   成功：{}", result),
        Err(e) => println!("   验证失败（预期）：{}", e),
    }

    // 测试 5: 测试 get_user 别名
    println!("\n7. 测试 get_user 别名");
    let args = tokitai::json!({"user_id": 123});
    match tools.call_tool("get_user", &args) {
        Ok(result) => println!("   get_user: {}", result),
        Err(e) => println!("   错误：{}", e),
    }

    let args = tokitai::json!({"user_id": 456});
    match tools.call_tool("get_user_info", &args) {
        Ok(result) => println!("   get_user_info: {}", result),
        Err(e) => println!("   错误：{}", e),
    }

    // 测试 6: 参数属性
    println!("\n8. 测试 method_with_param_attrs");
    let result = tools.call_tool(
        "method_with_param_attrs",
        &tokitai::json!({
            "name": "王五",
            "age": 30,
            "email": "wangwu@example.com"
        }),
    )?;
    println!("   结果：{}", result);

    println!("\n=== 所有测试完成 ===");

    Ok(())
}
