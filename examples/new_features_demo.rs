//! 新功能演示示例
//!
//! 演示以下新功能：
//! 1. #[tool(allow = [...])] - 警告抑制
//! 2. example_input 支持对象字面量
//! 3. cache 和 rate_limit 支持

use tokitai::tool;

pub struct NewFeaturesDemo;

#[tool]
impl NewFeaturesDemo {
    /// 演示警告抑制功能
    /// 
    /// 这个方法的参数是 Option 类型但没有 default/example，
    /// 使用 allow 抑制警告
    #[tool(allow = ["option_no_default"])]
    pub fn suppressed_warning(&self, _optional_param: Option<String>) -> String {
        "警告被抑制".to_string()
    }

    /// 演示 example_input 支持对象字面量
    /// 
    /// 这个工具的示例输入使用 Rust 字面量而不是 JSON 字符串
    #[tool(example_input = {"name": "张三", "age": 25})]
    pub fn object_example_input(&self, name: String, age: i32) -> String {
        format!("{} 今年 {} 岁", name, age)
    }

    /// 演示 cache 和 rate_limit 支持
    /// 
    /// 这些配置会添加到 JSON Schema 的扩展字段中
    #[tool(cache = "ttl=60", rate_limit = "10/min")]
    pub fn cached_and_limited(&self, query: String) -> String {
        format!("查询结果：{}", query)
    }

    /// 演示组合使用多个新功能
    /// 
    /// @param user_id 用户 ID
    #[tool(
        allow = ["option_no_default"],
        cache = "ttl=300",
        rate_limit = "100/hour",
        example_input = {"user_id": 123}
    )]
    pub fn combined_features(
        &self,
        user_id: i32,
    ) -> String {
        format!("用户 {} 的详情", user_id)
    }
}

fn main() {
    let demo = NewFeaturesDemo;

    println!("=== 新功能演示 ===\n");
    
    // 打印所有工具定义
    println!("可用的工具：");
    for def in NewFeaturesDemo::TOOL_DEFINITIONS {
        println!("\n工具：{}", def.name);
        println!("描述：{}", def.description);
        println!("Schema: {}", def.input_schema);
    }
    
    // 演示调用
    println!("\n=== 测试调用 ===\n");
    
    // 测试对象示例输入
    let result = demo.call_tool("object_example_input",
        &tokitai::json!({"name": "李四", "age": 30})).unwrap();
    println!("object_example_input: {}", result);
    
    // 测试缓存和限流工具
    let result = demo.call_tool("cached_and_limited",
        &tokitai::json!({"query": "测试查询"})).unwrap();
    println!("cached_and_limited: {}", result);
    
    // 测试组合功能
    let result = demo.call_tool("combined_features",
        &tokitai::json!({"user_id": 789})).unwrap();
    println!("combined_features: {}", result);
    
    // 测试警告抑制
    let result = demo.call_tool("suppressed_warning",
        &tokitai::json!({})).unwrap();
    println!("suppressed_warning: {}", result);
}
