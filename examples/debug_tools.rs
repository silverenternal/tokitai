//! 调试工具示例：展示如何使用辅助方法
//!
//! 本示例演示：
//! 1. 使用 `input_schema_pretty()` 打印格式化的 Schema
//! 2. 使用 `input_schema_value()` 访问特定字段
//! 3. 测试工具调用

use tokitai::json;
use tokitai::tool;
use tokitai::ToolProvider;

#[tool]
pub struct DebugTools;

#[tool]
impl DebugTools {
    /// 创建用户
    ///
    /// @param name 用户名（3-20 字符）
    /// @param email 邮箱地址
    /// @param age 年龄（0-150）
    #[tool(
        min_length_name = 3,
        max_length_name = 20,
        pattern_email = "@",
        min_age = 0,
        max_age = 150
    )]
    pub fn create_user(
        &self,
        name: String,
        email: String,
        age: i32,
    ) -> Result<String, tokitai::ToolError> {
        Ok(format!(
            "创建用户：{} (邮箱：{}, 年龄：{})",
            name, email, age
        ))
    }

    /// 搜索产品
    ///
    /// @param keyword 搜索关键词
    /// @param category 产品分类（可选）
    /// @param max_price 最高价格（可选）
    #[tool(allow = ["option_no_default"])]
    pub fn search_products(
        &self,
        keyword: String,
        category: Option<String>,
        max_price: Option<f64>,
    ) -> Result<String, tokitai::ToolError> {
        Ok(format!(
            "搜索：{} (分类：{:?}, 最高价：{:?})",
            keyword, category, max_price
        ))
    }

    /// 计算统计数据
    ///
    /// @param values 数值数组
    /// @param include_median 是否包含中位数
    #[tool]
    pub fn calculate_stats(
        &self,
        values: Vec<f64>,
        include_median: bool,
    ) -> Result<String, tokitai::ToolError> {
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        Ok(format!(
            "平均值：{:.2}, 中位数：{}",
            mean,
            if include_median {
                "包含"
            } else {
                "不包含"
            }
        ))
    }
}

fn main() {
    println!("=== 调试工具示例 ===\n");

    // 1. 打印格式化的 Schema
    println!("1. 格式化 Schema:");
    for def in DebugTools::tool_definitions() {
        println!("\n工具：{}", def.name);
        println!("描述：{}", def.description);
        if let Ok(schema) = def.input_schema_pretty() {
            println!("Schema:\n{}", schema);
        }
    }

    // 2. 访问特定字段
    println!("\n\n2. 访问特定字段:");
    let tools = DebugTools::tool_definitions();
    let tool = tools.iter().find(|t| t.name == "create_user").unwrap();
    let schema = tool.input_schema_value().unwrap();

    let name_schema = &schema["properties"]["name"];
    println!("name 字段描述：{}", name_schema["description"]);
    println!("name 字段类型：{}", name_schema["type"]);
    println!("name 最小长度：{}", name_schema["minLength"]);
    println!("name 最大长度：{}", name_schema["maxLength"]);

    // 3. 测试调用
    println!("\n\n3. 测试调用:");
    let tools = DebugTools;

    // 测试 create_user
    let result = tools
        .call_tool(
            "create_user",
            &json!({
                "name": "zhangsan",
                "email": "zhangsan@example.com",
                "age": 25
            }),
        )
        .unwrap();
    println!("调用 create_user 结果：{}", result);

    // 测试 search_products
    let result = tools
        .call_tool(
            "search_products",
            &json!({
                "keyword": "笔记本电脑",
                "category": "电子产品",
                "max_price": 8000.0
            }),
        )
        .unwrap();
    println!("调用 search_products 结果：{}", result);

    // 测试 calculate_stats
    let result = tools
        .call_tool(
            "calculate_stats",
            &json!({
                "values": [1.0, 2.0, 3.0, 4.0, 5.0],
                "include_median": true
            }),
        )
        .unwrap();
    println!("调用 calculate_stats 结果：{}", result);

    // 4. 展示 version 和 deprecated 信息（如果有）
    println!("\n\n4. 工具版本信息:");
    for def in DebugTools::tool_definitions() {
        println!("\n工具：{}", def.name);
        if let Some(version) = &def.version {
            println!("  版本：{}", version);
        }
        if let Some(since) = &def.deprecated_since {
            println!("  废弃于：{}", since);
        }
        if let Some(remove) = &def.remove_in {
            println!("  移除于：{}", remove);
        }
        if let Some(replaced_by) = &def.replaced_by {
            println!("  替代者：{}", replaced_by);
        }
    }
}
