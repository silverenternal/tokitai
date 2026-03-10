//! 测试 13: 复杂返回类型支持

use tokitai::tool;
use tokitai::ToolProvider;
use std::collections::HashMap;

#[derive(Default)]
pub struct ComplexReturnTools;

#[tool]
impl ComplexReturnTools {
    /// 返回 HashMap
    pub fn get_user_map(&self, id: i32) -> HashMap<String, String> {
        let mut map = HashMap::new();
        map.insert("id".to_string(), id.to_string());
        map.insert("name".to_string(), format!("User {}", id));
        map
    }
    
    /// 返回元组
    pub fn get_coordinates(&self) -> (f64, f64) {
        (40.7128, -74.0060)
    }
    
    /// 返回嵌套 Vec
    pub fn get_matrix(&self, size: i32) -> Vec<Vec<i32>> {
        (0..size).map(|i| (0..size).map(|j| i * j).collect()).collect()
    }
}

fn main() {
    let tools = ComplexReturnTools;
    
    // 验证工具定义生成
    let defs = ComplexReturnTools::tool_definitions();
    assert_eq!(defs.len(), 3);
    
    // 验证 get_user_map 的 schema
    let map_tool = defs.iter().find(|t| t.name == "get_user_map").unwrap();
    let schema: serde_json::Value = serde_json::from_str(&map_tool.input_schema).unwrap();
    assert!(schema["properties"]["id"].is_object());
    
    // 调用测试
    let result = tools.call_tool("get_user_map", &serde_json::json!({"id": 123})).unwrap();
    assert!(result.is_object());
    assert_eq!(result["id"], "123");
    
    // 测试元组返回
    let result = tools.call_tool("get_coordinates", &serde_json::json!({})).unwrap();
    assert!(result.is_array());
    assert_eq!(result.as_array().unwrap().len(), 2);
    
    // 测试嵌套 Vec
    let result = tools.call_tool("get_matrix", &serde_json::json!({"size": 3})).unwrap();
    assert!(result.is_array());
}
