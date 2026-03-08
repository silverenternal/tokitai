//! 测试 example_input 输出到 examples 字段

use tokitai::tool;

pub struct TestExamples;

#[tool]
impl TestExamples {
    /// 测试 example_input
    #[tool(example_input = {"name": "张三", "age": 25})]
    pub fn test_method(&self, name: String, age: i32) -> String {
        format!("{} 今年 {} 岁", name, age)
    }
}

fn main() {
    println!("=== 测试 example_input 输出 ===\n");
    
    for def in TestExamples::TOOL_DEFINITIONS {
        println!("工具：{}", def.name);
        println!("Schema: {}", def.input_schema);
        println!();
    }
}
