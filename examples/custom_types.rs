//! 自定义类型支持示例
//!
//! 展示如何使用 #[tool_type] 宏注册自定义类型的 schema

use serde::Deserialize;
use tokitai::tool;
use tokitai::tool_type;

/// 位置信息 - 使用 #[tool_type] 注册 schema
#[tool_type(
    name = "Location",
    properties = "latitude: number, longitude: number",
    required = "latitude, longitude"
)]
#[derive(Debug, Clone, Deserialize)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
}

/// 用户信息 - 使用 #[tool_type] 注册 schema
#[tool_type(
    name = "UserProfile",
    properties = "id: integer, name: string, email: string, location: object",
    required = "id, name"
)]
#[derive(Debug, Clone, Deserialize)]
pub struct UserProfile {
    pub id: i32,
    pub name: String,
    pub email: Option<String>,
    pub location: Option<Location>,
}

/// 地理信息服务
pub struct GeoService;

#[tool]
impl GeoService {
    /// 计算两个位置之间的距离
    ///
    /// - `from`: 起始位置
    /// - `to`: 目标位置
    pub fn calculate_distance(
        &self,
        from: Location,
        to: Location,
    ) -> f64 {
        // 简化的距离计算（实际应该使用 Haversine 公式）
        let lat_diff = to.latitude - from.latitude;
        let lon_diff = to.longitude - from.longitude;
        ((lat_diff * lat_diff) + (lon_diff * lon_diff)).sqrt() * 111.0
    }

    /// 获取用户的推荐地点
    ///
    /// - `user`: 用户信息
    /// - `radius_km`: 搜索半径（公里）
    pub fn get_recommendations(
        &self,
        _user: UserProfile,
        radius_km: f64,
    ) -> Vec<String> {
        vec![
            format!("推荐地点 A (距离 {} 公里)", radius_km),
            format!("推荐地点 B (距离 {} 公里)", radius_km * 1.5),
        ]
    }

    /// 处理位置列表
    ///
    /// - `locations`: 位置列表
    pub fn process_locations(
        &self,
        locations: Vec<Location>,
    ) -> String {
        format!("处理了 {} 个位置", locations.len())
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    println!("=== 自定义类型支持示例 ===\n");

    let service = GeoService;

    // 展示工具定义
    println!("1. 工具定义（查看生成的 JSON Schema）");
    for tool in GeoService::TOOL_DEFINITIONS {
        println!("\n   工具：{}", tool.name);
        println!("   描述：{}", tool.description);
        println!("   Schema: {}", tool.input_schema);
    }
    println!();

    // 测试 calculate_distance
    println!("2. 测试 calculate_distance");
    let result = service.call_tool(
        "calculate_distance",
        &tokitai::json!({
            "from": {"latitude": 39.9, "longitude": 116.4},
            "to": {"latitude": 31.2, "longitude": 121.5}
        }),
    )?;
    println!("   距离：{} 公里", result);
    println!();

    // 测试 get_recommendations
    println!("3. 测试 get_recommendations");
    let result = service.call_tool(
        "get_recommendations",
        &tokitai::json!({
            "_user": {"id": 123, "name": "Alice", "email": "alice@example.com"},
            "radius_km": 50.0
        }),
    )?;
    println!("   推荐：{}", result);
    println!();

    // 测试 process_locations
    println!("4. 测试 process_locations");
    let result = service.call_tool(
        "process_locations",
        &tokitai::json!({
            "locations": [
                {"latitude": 39.9, "longitude": 116.4},
                {"latitude": 31.2, "longitude": 121.5},
                {"latitude": 23.1, "longitude": 113.3}
            ]
        }),
    )?;
    println!("   结果：{}", result);
    println!();

    println!("=== 所有测试完成 ===");

    Ok(())
}
