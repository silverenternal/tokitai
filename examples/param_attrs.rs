//! Parameter attribute example
//!
//! Demonstrates the three tool-description styles supported by Tokitai v0.3.4+:
//! 1. Automatic extraction from doc comments
//! 2. Override via `#[tool]` attributes
//! 3. `tokitai!` configuration macro
//!
//! Run with: `cargo run --example param_attrs`

use serde_json::Value;
use tokitai::tool;
use tokitai::ToolProvider;

/// Parameter attribute test tools
pub struct ParamTools;

#[tool]
impl ParamTools {
    /// Style 1: doc comments (simplest)
    ///
    /// @param name user's name
    /// @param age user's age
    pub fn method_with_doc(&self, name: String, age: i32) -> String {
        format!("{} is {} years old", name, age)
    }

    /// Style 2: `#[tool]` attribute override for the method description
    #[tool(
        desc = "Custom method description",
        tags = ["demo", "test"]
    )]
    pub fn method_with_custom_desc(&self, name: String, age: i32) -> String {
        format!("{} is {} years old", name, age)
    }

    /// Style 3: parameter-level attributes
    ///
    /// @param name user's name
    /// @param age user's age
    /// @param email email address
    #[tool(
        example_name = "Alice",
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
    println!("=== Parameter Attribute Example ===\n");

    for tool in ParamTools::tool_definitions() {
        println!("Method: {}", tool.name);
        println!("Description: {}\n", tool.description);
        println!("Schema: {}\n", pretty_json(&tool.input_schema));
    }
}

fn pretty_json(json_str: &str) -> String {
    let value: Value = serde_json::from_str(json_str).unwrap();
    serde_json::to_string_pretty(&value).unwrap()
}
