//! 提取模块
//!
//! 包含文档提取、参数提取、工具信息提取、验证等功能

pub(crate) mod docs;
pub(crate) mod params;
pub(crate) mod tool_info;
pub(crate) mod validate;

pub(crate) use tool_info::collect_tool_methods;
