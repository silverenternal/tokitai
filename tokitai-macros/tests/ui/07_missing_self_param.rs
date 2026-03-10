//! 测试 07: 缺失 self 参数 - 应该编译失败

use tokitai::tool;

#[derive(Default)]
pub struct NoSelfTools;

#[tool]
impl NoSelfTools {
    /// 这个方法缺少 self 参数 - 宏应该处理这种情况
    pub fn method_without_self(a: i32, b: i32) -> i32 {
        a + b
    }
}

// 注意：这个测试应该编译失败，因为方法缺少 self 参数
// 宏应该报错提示用户添加 self 参数

fn main() {}
