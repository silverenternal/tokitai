//! 高级类型支持示例
//!
//! 展示改进后的宏对复杂类型的支持：
//! - 嵌套 Option 类型
//! - Vec 数组元素类型
//! - HashMap
//! - 参数 doc comment 提取

use std::collections::HashMap;
use tokitai::tool;

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
        format!(
            "处理用户 {} ({}): 标签数 = {}",
            user_id,
            name,
            tags.len()
        )
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
                let median = if sorted.len() % 2 == 0 {
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
    pub fn process_slice(
        &self,
        data: Vec<i32>,
    ) -> i32 {
        data.iter().sum()
    }

    /// 处理元组参数（简化为数组）
    ///
    /// - `coordinates`: 坐标数据
    pub fn process_tuple_data(
        &self,
        coordinates: (i32, i32),
    ) -> String {
        format!("坐标：({}, {})", coordinates.0, coordinates.1)
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== 高级类型支持示例 ===\n");

    let tools = AdvancedTools;

    // 展示工具定义
    println!("1. 工具定义（查看生成的 JSON Schema）");
    for tool in AdvancedTools::TOOL_DEFINITIONS {
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

    println!("=== 所有测试完成 ===");

    Ok(())
}
