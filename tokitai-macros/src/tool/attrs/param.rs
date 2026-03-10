//! 参数级工具属性解析
//!
//! 此模块主要 re-export types::param 中的解析函数

pub use crate::tool::types::param::{
    parse_json_value, parse_lit_to_f64, parse_lit_to_string, parse_lit_to_usize, parse_value_string,
};
