//! 类型 Schema 缓存
//!
//! 提供全局的类型 schema 缓存，用于缓存自定义类型的 JSON Schema

use std::collections::BTreeMap;
use std::sync::{LazyLock, Mutex};

use super::types::JsonSchema;

/// 全局类型 schema 缓存（使用 LazyLock + Mutex 实现线程安全）
pub(crate) static TYPE_SCHEMA_CACHE: LazyLock<Mutex<BTreeMap<String, JsonSchema>>> =
    LazyLock::new(|| Mutex::new(BTreeMap::new()));
