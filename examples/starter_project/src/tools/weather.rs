//! Tokitai 入门项目 - 天气工具
//!
//! 演示如何定义一个简单的天气查询工具

use tokitai::tool;

/// 天气查询工具结构体
pub struct WeatherTool;

#[tool]
impl WeatherTool {
    /// 查询指定城市的天气
    pub fn get_weather(&self, city: String) -> String {
        // 模拟天气数据
        match city.to_lowercase().as_str() {
            "北京" | "beijing" => "北京：晴朗，温度 25°C，湿度 40%".to_string(),
            "上海" | "shanghai" => "上海：多云，温度 22°C，湿度 60%".to_string(),
            "广州" | "guangzhou" => "广州：小雨，温度 28°C，湿度 80%".to_string(),
            "深圳" | "shenzhen" => "深圳：晴朗，温度 30°C，湿度 70%".to_string(),
            _ => format!("{}：天气晴朗，温度 26°C", city),
        }
    }

    /// 获取多个城市的天气
    pub fn get_weather_batch(&self, cities: Vec<String>) -> Vec<String> {
        cities.into_iter().map(|city| self.get_weather(city)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_weather_beijing() {
        let tool = WeatherTool;
        let result = tool.get_weather("北京".to_string());
        assert!(result.contains("晴朗"));
    }

    #[test]
    fn test_get_weather_batch() {
        let tool = WeatherTool;
        let cities = vec!["北京".to_string(), "上海".to_string()];
        let results = tool.get_weather_batch(cities);
        assert_eq!(results.len(), 2);
    }
}
