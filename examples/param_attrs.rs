//! 参数属性示例
//!
//! 演示 Tokitai v0.3.4 支持的三种工具描述方式：
//! 1. 文档注释自动提取
//! 2. #[tool] 属性覆盖
//! 3. tokitai! 配置宏
//!
//! 运行：cargo run --example param_attrs

use serde_json::Value;
use tokitai::tool;
use tokitai::ToolProvider;

/// 参数属性测试工具集
pub struct ParamTools;

#[tool]
impl ParamTools {
    /// 方式 1：文档注释（最简单）
    ///
    /// @param name 用户姓名
    /// @param age 用户年龄
    pub fn method_with_doc(&self, name: String, age: i32) -> String {
        format!("{} is {} years old", name, age)
    }

    /// 方式 2：#[tool] 属性覆盖方法描述
    #[tool(
        desc = "自定义方法描述",
        tags = ["demo", "test"]
    )]
    pub fn method_with_custom_desc(&self, name: String, age: i32) -> String {
        format!("{} is {} years old", name, age)
    }

    /// 方式 3：参数级属性
    ///
    /// @param name 用户姓名
    /// @param age 用户年龄
    /// @param email 邮箱地址
    #[tool(
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

fn main() {
    println!("=== 参数属性示例 ===\n");

    for tool in ParamTools::tool_definitions() {
        println!("方法：{}", tool.name);
        println!("描述：{}\n", tool.description);
        println!("Schema: {}\n", pretty_json(&tool.input_schema));
    }
}

fn pretty_json(json_str: &str) -> String {
    let value: Value = serde_json::from_str(json_str).unwrap();
    serde_json::to_string_pretty(&value).unwrap()
}
