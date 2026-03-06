//! Tokitai 入门项目 - 计算器工具
//!
//! 演示如何定义数学计算工具

use tokitai::tool;

/// 计算器工具结构体
pub struct Calculator;

#[tool]
impl Calculator {
    /// 两个数相加
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// 两个数相减
    pub fn subtract(&self, a: i32, b: i32) -> i32 {
        a - b
    }

    /// 两个数相乘
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }

    /// 两个数相除
    pub fn divide(&self, dividend: i32, divisor: i32) -> Result<i32, String> {
        if divisor == 0 {
            Err("除数不能为零".to_string())
        } else {
            Ok(dividend / divisor)
        }
    }

    /// 计算平方
    pub fn square(&self, n: i32) -> i32 {
        n * n
    }

    // 内部辅助方法，不暴露给 AI
    #[tool(skip)]
    fn internal_check(&self, value: i32) -> bool {
        value >= 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let calc = Calculator;
        assert_eq!(calc.add(2, 3), 5);
    }

    #[test]
    fn test_divide_by_zero() {
        let calc = Calculator;
        assert!(calc.divide(10, 0).is_err());
    }

    #[test]
    fn test_square() {
        let calc = Calculator;
        assert_eq!(calc.square(5), 25);
    }
}
