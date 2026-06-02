//! Debug tool example: demonstrates helper methods
//!
//! This example shows:
//! 1. Using `input_schema_pretty()` to print formatted Schemas
//! 2. Using `input_schema_value()` to access specific fields
//! 3. Testing tool invocations

use tokitai::json;
use tokitai::tool;
use tokitai::ToolProvider;

#[tool]
pub struct DebugTools;

#[tool]
impl DebugTools {
    /// Create a user
    ///
    /// @param name user's name (3-20 characters)
    /// @param email email address
    /// @param age age (0-150)
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
            "Created user: {} (email: {}, age: {})",
            name, email, age
        ))
    }

    /// Search products
    ///
    /// @param keyword search keyword
    /// @param category product category (optional)
    /// @param max_price maximum price (optional)
    #[tool(allow = ["option_no_default"])]
    pub fn search_products(
        &self,
        keyword: String,
        category: Option<String>,
        max_price: Option<f64>,
    ) -> Result<String, tokitai::ToolError> {
        Ok(format!(
            "Search: {} (category: {:?}, max price: {:?})",
            keyword, category, max_price
        ))
    }

    /// Compute statistics
    ///
    /// @param values numeric array
    /// @param include_median whether to include the median
    #[tool]
    pub fn calculate_stats(
        &self,
        values: Vec<f64>,
        include_median: bool,
    ) -> Result<String, tokitai::ToolError> {
        let mean = values.iter().sum::<f64>() / values.len() as f64;
        Ok(format!(
            "Mean: {:.2}, Median: {}",
            mean,
            if include_median {
                "included"
            } else {
                "excluded"
            }
        ))
    }
}

fn main() {
    println!("=== Debug Tool Example ===\n");

    // 1. Print the formatted schema
    println!("1. Formatted schema:");
    for def in DebugTools::tool_definitions() {
        println!("\nTool: {}", def.name);
        println!("Description: {}", def.description);
        if let Ok(schema) = def.input_schema_pretty() {
            println!("Schema:\n{}", schema);
        }
    }

    // 2. Access specific fields
    println!("\n\n2. Access specific fields:");
    let tools = DebugTools::tool_definitions();
    let tool = tools.iter().find(|t| t.name == "create_user").unwrap();
    let schema = tool.input_schema_value().unwrap();

    let name_schema = &schema["properties"]["name"];
    println!("name field description: {}", name_schema["description"]);
    println!("name field type: {}", name_schema["type"]);
    println!("name min length: {}", name_schema["minLength"]);
    println!("name max length: {}", name_schema["maxLength"]);

    // 3. Test invocations
    println!("\n\n3. Test invocations:");
    let tools = DebugTools;

    // Test create_user
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
    println!("create_user result: {}", result);

    // Test search_products
    let result = tools
        .call_tool(
            "search_products",
            &json!({
                "keyword": "laptop",
                "category": "electronics",
                "max_price": 8000.0
            }),
        )
        .unwrap();
    println!("search_products result: {}", result);

    // Test calculate_stats
    let result = tools
        .call_tool(
            "calculate_stats",
            &json!({
                "values": [1.0, 2.0, 3.0, 4.0, 5.0],
                "include_median": true
            }),
        )
        .unwrap();
    println!("calculate_stats result: {}", result);

    // 4. Display version and deprecation info (if any)
    println!("\n\n4. Tool version info:");
    for def in DebugTools::tool_definitions() {
        println!("\nTool: {}", def.name);
        if let Some(version) = &def.version {
            println!("  Version: {}", version);
        }
        if let Some(since) = &def.deprecated_since {
            println!("  Deprecated since: {}", since);
        }
        if let Some(remove) = &def.remove_in {
            println!("  Removed in: {}", remove);
        }
        if let Some(replaced_by) = &def.replaced_by {
            println!("  Replaced by: {}", replaced_by);
        }
    }
}
