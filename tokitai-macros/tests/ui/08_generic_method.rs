//! 测试 08: 泛型方法不支持 - 应该编译失败

use tokitai::tool;

#[derive(Default)]
pub struct GenericTools;

#[tool]
impl GenericTools {
    /// 泛型方法目前不支持 - 应该编译失败
    pub fn generic_method<T: ToString>(&self, value: T) -> String {
        value.to_string()
    }
}

// 注意：这个测试应该编译失败，因为宏不支持泛型方法
// 错误信息应该提示用户使用具体类型

fn main() {}
