//! 简单测试宏展开

use tokitai::tool;

struct TestTools;

#[tool]
impl TestTools {
    /// 测试方法
    pub fn test_method(&self, a: i32) -> i32 {
        a
    }
}

fn main() {
    println!("Test");
}
