//! 测试 11: 无效的验证表达式 - 应该编译失败

use tokitai::tool;

#[derive(Default)]
pub struct InvalidValidationTools;

#[tool]
impl InvalidValidationTools {
    /// 使用无效的验证表达式 - 应该编译失败
    pub fn invalid_validate(
        &self,
        #[param_tool(validate = "invalid_syntax(")]
        value: String
    ) -> String {
        value
    }
}

// 注意：这个测试应该编译失败，因为验证表达式语法错误
fn main() {}
