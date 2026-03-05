//! Result 返回类型测试

use tokitai::tool;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MathError {
    #[error("除数不能为零")]
    DivisionByZero,
}

pub struct MathService;

#[tool]
impl MathService {
    /// 两个数相除
    pub fn divide(&self, a: f64, b: f64) -> Result<f64, MathError> {
        if b == 0.0 {
            Err(MathError::DivisionByZero)
        } else {
            Ok(a / b)
        }
    }
}

#[tokio::main]
async fn main() {
    let math = MathService;
    
    // 验证 TOOL_DEFINITIONS 生成
    let tools = MathService::TOOL_DEFINITIONS;
    assert_eq!(tools.len(), 1);
    
    // 成功情况
    let result = math.call_tool("divide", &serde_json::json!({"a": 10.0, "b": 2.0})).await.unwrap();
    assert_eq!(result, 5.0);
    
    // 错误情况（会返回 Err）
    let err = math.call_tool("divide", &serde_json::json!({"a": 10.0, "b": 0.0})).await;
    assert!(err.is_err());
}
