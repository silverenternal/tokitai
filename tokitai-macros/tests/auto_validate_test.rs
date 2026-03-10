//! 自动验证和转换功能的集成测试
//! 测试 @validate 和 @transform 文档语法自动生成的验证和转换代码

use serde_json::json;
use tokitai::tool;

#[derive(Default)]
pub struct UserService;

#[tool]
impl UserService {
    /// 创建用户（完全自动验证版）
    ///
    /// @param name 用户名（不能为空）
    /// @validate name !value.is_empty()
    /// @param email 邮箱（自动转换为小写）
    /// @transform email value.to_lowercase()
    /// @param age 年龄（0-150 之间）
    /// @validate age value > 0 && value < 150
    /// @required name
    /// @required age
    pub fn create_user(
        &self,
        name: String,
        email: String,
        age: i32,
    ) -> Result<String, tokitai::ToolError> {
        // 注意：不需要手动验证！宏会自动生成验证和转换代码
        Ok(format!(
            "创建用户：{} (邮箱：{}, 年龄：{})",
            name, email, age
        ))
    }

    /// 处理订单（使用 @validate 进行运行时验证）
    ///
    /// @param status 订单状态
    /// @validate status value == "pending" || value == "shipped" || value == "delivered"
    /// @param amount 订单金额（必须大于 0）
    /// @validate amount value > 0.0
    pub fn process_order(
        &self,
        status: String,
        amount: f64,
        _discount: Option<f64>,
    ) -> Result<String, tokitai::ToolError> {
        Ok(format!("处理订单：状态={}, 金额={}", status, amount))
    }

    /// 搜索商品（使用 @validate 进行运行时验证）
    ///
    /// @param keyword 搜索关键词（长度 3-20 字符）
    /// @validate keyword value.len() >= 3 && value.len() <= 20
    /// @param _category 商品分类
    /// @param _price_range 价格范围（格式：YYYY-MM-DD）
    pub fn search_products(
        &self,
        keyword: String,
        _category: Option<String>,
        _price_range: Option<String>,
    ) -> Result<String, tokitai::ToolError> {
        Ok(format!(
            "搜索：{} ({})",
            keyword,
            _category.unwrap_or_default()
        ))
    }
}

#[test]
fn test_auto_validate_empty_name() {
    let service = UserService;
    let result = service.call_tool(
        "create_user",
        &json!({
            "name": "",
            "email": "test@example.com",
            "age": 25
        }),
    );

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("验证失败"));
    assert!(err_msg.contains("name"));
}

#[test]
fn test_auto_validate_age_out_of_range() {
    let service = UserService;

    // 年龄 > 150
    let result = service.call_tool(
        "create_user",
        &json!({
            "name": "张三",
            "email": "test@example.com",
            "age": 200
        }),
    );

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("验证失败"));
    assert!(err_msg.contains("age"));

    // 年龄 <= 0
    let result = service.call_tool(
        "create_user",
        &json!({
            "name": "张三",
            "email": "test@example.com",
            "age": 0
        }),
    );

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("验证失败"));
    assert!(err_msg.contains("age"));
}

#[test]
fn test_auto_transform_lowercase() {
    let service = UserService;
    let result = service.call_tool(
        "create_user",
        &json!({
            "name": "张三",
            "email": "TEST@EXAMPLE.COM",
            "age": 25
        }),
    );

    assert!(result.is_ok());
    let output = result.unwrap().as_str().unwrap().to_string();
    // 验证 email 被转换为小写
    assert!(output.contains("test@example.com"));
}

#[test]
fn test_one_of_validation() {
    let service = UserService;

    // 无效的 status
    let result = service.call_tool(
        "process_order",
        &json!({
            "status": "invalid_status",
            "amount": 100.0,
            "discount": null
        }),
    );

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("验证失败"));
    assert!(err_msg.contains("status"));

    // 有效的 status
    let result = service.call_tool(
        "process_order",
        &json!({
            "status": "pending",
            "amount": 100.0,
            "discount": null
        }),
    );

    assert!(result.is_ok());
}

#[test]
fn test_min_max_validation() {
    let service = UserService;

    // amount <= 0
    let result = service.call_tool(
        "process_order",
        &json!({
            "status": "pending",
            "amount": 0.0,
            "discount": null
        }),
    );

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("验证失败"));
    assert!(err_msg.contains("amount"));

    // 有效值
    let result = service.call_tool(
        "process_order",
        &json!({
            "status": "pending",
            "amount": 100.0,
            "discount": null
        }),
    );

    assert!(result.is_ok());
}

#[test]
fn test_min_max_length_validation() {
    let service = UserService;

    // keyword 长度 < 3
    let result = service.call_tool(
        "search_products",
        &json!({
            "keyword": "ab",
            "category": null,
            "price_range": null
        }),
    );

    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("验证失败"));

    // keyword 长度 > 20
    let result = service.call_tool(
        "search_products",
        &json!({
            "keyword": "this_is_a_very_long_keyword_over_20",
            "category": null,
            "price_range": null
        }),
    );

    assert!(result.is_err());

    // 有效长度
    let result = service.call_tool(
        "search_products",
        &json!({
            "keyword": "laptop",
            "category": null,
            "price_range": null
        }),
    );

    assert!(result.is_ok());
}

#[test]
fn test_pattern_validation_manual() {
    // 注意：pattern 属性只生成 JSON Schema，不生成运行时验证代码
    // 如需运行时验证，请使用 @validate
    let service = UserService;

    // 有效输入
    let result = service.call_tool(
        "search_products",
        &json!({
            "keyword": "laptop",
            "category": null,
            "price_range": "2024-01-01"
        }),
    );

    assert!(result.is_ok());
}

#[test]
fn test_multiple_of_validation_manual() {
    // 注意：multiple_of 属性只生成 JSON Schema，不生成运行时验证代码
    // 如需运行时验证，请使用 @validate
    let service = UserService;

    // 有效输入
    let result = service.call_tool(
        "process_order",
        &json!({
            "status": "pending",
            "amount": 100.0,
            "discount": 0.15
        }),
    );

    assert!(result.is_ok());
}

#[test]
fn test_valid_input() {
    let service = UserService;

    // 所有验证都通过
    let result = service.call_tool(
        "create_user",
        &json!({
            "name": "张三",
            "email": "ZHANGSAN@EXAMPLE.COM",
            "age": 25
        }),
    );

    assert!(result.is_ok());
    let output = result.unwrap().as_str().unwrap().to_string();
    assert!(output.contains("张三"));
    assert!(output.contains("zhangsan@example.com")); // 验证转换
    assert!(output.contains("25"));
}
