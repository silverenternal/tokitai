//! 测试 validate、transform 和 alias 功能的示例
//! 演示宏自动验证和转换功能（不需要手动写验证代码）

use tokitai::tool;
use tokitai::ToolProvider;

#[tool]
pub struct MyTools;

#[tool]
impl MyTools {
    /// 创建用户（自动验证版）
    ///
    /// 演示使用 doc comment 语法进行自动验证和转换
    ///
    /// @param name 用户名（不能为空）
    /// @validate name !value.is_empty()
    /// @param email 用户邮箱
    /// @param age 用户年龄（必须在 0 到 150 之间）
    /// @validate age value > 0 && value < 150
    /// @required name
    /// @required age
    #[tool(alias = ["create_user_account", "add_user"])]
    pub fn create_user(
        &self,
        name: String,
        email: String,
        age: i32,
    ) -> Result<String, tokitai::ToolError> {
        // 验证由宏自动生成
        Ok(format!(
            "创建用户：{} (邮箱：{}, 年龄：{})",
            name, email, age
        ))
    }

    /// 获取用户
    ///
    /// @param user_id 用户 ID
    #[tool(alias = ["get_user_info", "fetch_user"])]
    pub fn get_user(&self, user_id: i32) -> Result<String, tokitai::ToolError> {
        Ok(format!("获取用户：{}", user_id))
    }
}

fn main() {
    let tools = MyTools;

    // 测试别名
    println!("=== 测试工具定义 ===");
    for def in MyTools::tool_definitions() {
        println!("工具：{}", def.name);
        println!("描述：{}", def.description);
        println!("Schema: {}", def.input_schema);
        println!();
    }

    // 测试 create_user（带验证和转换）
    println!("=== 测试 create_user ===");

    // 测试 1: 正常调用
    let args = tokitai::json!({
        "name": "张三",
        "email": "ZHANGSAN@EXAMPLE.COM",
        "age": 25
    });
    match tools.call_tool("create_user", &args) {
        Ok(result) => println!("成功：{}", result),
        Err(e) => println!("错误：{}", e),
    }

    // 测试 2: 使用别名
    let args = tokitai::json!({
        "name": "李四",
        "email": "LISI@EXAMPLE.COM",
        "age": 30
    });
    match tools.call_tool("create_user_account", &args) {
        Ok(result) => println!("使用别名成功：{}", result),
        Err(e) => println!("错误：{}", e),
    }

    // 测试 3: 使用另一个别名
    let args = tokitai::json!({
        "name": "王五",
        "email": "WANGWU@EXAMPLE.COM",
        "age": 35
    });
    match tools.call_tool("add_user", &args) {
        Ok(result) => println!("使用另一个别名成功：{}", result),
        Err(e) => println!("错误：{}", e),
    }

    // 测试 4: 验证失败 - 空名字
    let args = tokitai::json!({
        "name": "",
        "email": "test@example.com",
        "age": 25
    });
    match tools.call_tool("create_user", &args) {
        Ok(result) => println!("成功：{}", result),
        Err(e) => println!("验证失败（预期）：{}", e),
    }

    // 测试 5: 验证失败 - 年龄超出范围
    let args = tokitai::json!({
        "name": "赵六",
        "email": "zhaoliu@example.com",
        "age": 200
    });
    match tools.call_tool("create_user", &args) {
        Ok(result) => println!("成功：{}", result),
        Err(e) => println!("验证失败（预期）：{}", e),
    }

    // 测试 6: 测试邮箱转换（应该转为小写）
    let args = tokitai::json!({
        "name": "测试用户",
        "email": "TEST@EXAMPLE.COM",
        "age": 28
    });
    match tools.call_tool("create_user", &args) {
        Ok(result) => println!("邮箱转换结果：{}", result),
        Err(e) => println!("错误：{}", e),
    }

    // 测试 7: 测试 get_user 别名
    println!("\n=== 测试 get_user 别名 ===");
    let args = tokitai::json!({"user_id": 123});
    match tools.call_tool("get_user", &args) {
        Ok(result) => println!("get_user: {}", result),
        Err(e) => println!("错误：{}", e),
    }

    let args = tokitai::json!({"user_id": 456});
    match tools.call_tool("get_user_info", &args) {
        Ok(result) => println!("get_user_info: {}", result),
        Err(e) => println!("错误：{}", e),
    }

    let args = tokitai::json!({"user_id": 789});
    match tools.call_tool("fetch_user", &args) {
        Ok(result) => println!("fetch_user: {}", result),
        Err(e) => println!("错误：{}", e),
    }
}
