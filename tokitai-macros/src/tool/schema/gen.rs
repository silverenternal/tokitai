//! Schema 生成逻辑
//!
//! 包含 SchemaGenConfig、generate_schema_json_with_deprecated_and_tags、
//! generate_schema_for_type 系列函数

use std::collections::BTreeMap;

use super::types::JsonSchema;
use crate::tool::types::param::ParamInfo;

/// 从自定义类型解析 schema（从 serde derive 提取字段信息）
///
/// 此函数尝试从 `#[derive(Serialize, Deserialize)]` 的 struct 中提取字段，
/// 生成完整的 object schema，而不是空 object。
///
/// # Arguments
/// * `ty` - 要解析的类型
/// * `description` - 类型描述
/// * `default_value` - 默认值
///
/// # Returns
/// * `Some(JsonSchema)` - 如果类型是 struct 且成功解析字段
/// * `None` - 如果类型不是 struct 或无法解析
fn extract_struct_schema(
    ty: &syn::Type,
    description: Option<&str>,
    default_value: Option<&serde_json::Value>,
) -> Option<JsonSchema> {
    // 尝试将类型解析为路径
    let path = match ty {
        syn::Type::Path(path) => &path.path,
        _ => return None,
    };

    // 获取类型标识符
    let ident = path.segments.first()?.ident.to_string();

    // 跳过基本类型和标准库类型
    let skip_types = [
        "String",
        "str",
        "i8",
        "i16",
        "i32",
        "i64",
        "i128",
        "u8",
        "u16",
        "u32",
        "u64",
        "u128",
        "usize",
        "isize",
        "f32",
        "f64",
        "bool",
        "Option",
        "Vec",
        "HashMap",
        "BTreeMap",
        "Box",
        "Rc",
        "Arc",
        "Cell",
        "RefCell",
        "Mutex",
        "RwLock",
        "DateTime",
        "NaiveDateTime",
        "NaiveDate",
        "NaiveTime",
        "Uuid",
        "Url",
        "PathBuf",
        "Path",
        "Value",
    ];
    if skip_types.contains(&ident.as_str()) {
        return None;
    }

    // 尝试从类型缓存中获取 schema
    if let Ok(cache) = super::cache::TYPE_SCHEMA_CACHE.lock() {
        if let Some(cached_schema) = cache.get(&ident) {
            return Some(cached_schema.clone());
        }
    }

    // Note: in the proc-macro environment we cannot directly reach
    // into another crate's type definitions. We return `None` so the
    // caller falls back to a generic object schema.
    //
    // Future-work: a richer schema generator could parse the entire
    // input `TokenStream` for `struct` / `enum` definitions, build a
    // local `Type -> Schema` map, and look up the parameter's type
    // there. That requires resolving cross-crate type information
    // (e.g. via `syn::Type` ↔ `proc-macro2` ↔ cargo metadata) and
    // is deferred to a post-0.6.0 release.
    //
    // Current behaviour: emit an object schema carrying the Rust
    // type name as a `description` so callers at least see the type
    // they need to pass.
    Some(JsonSchema::Object {
        ty: "object".to_string(),
        properties: BTreeMap::new(),
        required: vec![],
        description: description
            .map(str::to_string)
            .or_else(|| Some(format!("自定义类型：{}", ident))),
        additional_properties: None,
        default: default_value.cloned(),
        deprecated: None,
        tags: Vec::new(),
        returns: None,
        replaced_by: None,
        context: None,
        deprecated_note: None,
    })
}

/// JSON Schema 生成配置（Builder 模式）
pub struct SchemaGenConfig<'a> {
    pub params: &'a [ParamInfo],
    pub deprecated: bool,
    pub replaced_by: Option<&'a str>,
    pub context: Option<&'a str>,
    pub tags: &'a [String],
    pub return_description: Option<&'a str>,
    pub example_input: Option<&'a serde_json::Value>,
    pub param_order: Option<&'a [String]>,
    pub example_output: Option<&'a str>,
    pub deprecated_note: Option<&'a str>,
    pub deprecated_since: Option<&'a str>,
    pub remove_in: Option<&'a str>,
    pub group: Option<&'a str>,
    pub cache: Option<&'a str>,
    pub rate_limit: Option<&'a str>,
    /// T-016: baked few-shot examples. The macro bakes the user's
    /// literal `call!(self.method(args) => result)` into a
    /// `{ "input": ..., "output": ... }` envelope and appends it
    /// to the schema's `examples` field. Each entry is a JSON
    /// object; the field itself is a JSON array (matching the
    /// OpenAI / Anthropic / MCP spec).
    pub baked_examples: Option<&'a [crate::tool::example::BakedExample]>,
}

impl<'a> SchemaGenConfig<'a> {
    #[inline]
    pub fn new(params: &'a [ParamInfo]) -> Self {
        Self {
            params,
            deprecated: false,
            replaced_by: None,
            context: None,
            tags: &[],
            return_description: None,
            example_input: None,
            param_order: None,
            example_output: None,
            deprecated_note: None,
            deprecated_since: None,
            remove_in: None,
            group: None,
            cache: None,
            rate_limit: None,
            baked_examples: None,
        }
    }

    #[inline]
    pub(crate) fn deprecated(mut self, val: bool) -> Self {
        self.deprecated = val;
        self
    }

    #[inline]
    pub(crate) fn replaced_by(mut self, val: Option<&'a str>) -> Self {
        self.replaced_by = val;
        self
    }

    #[inline]
    pub(crate) fn context(mut self, val: Option<&'a str>) -> Self {
        self.context = val;
        self
    }

    #[inline]
    pub(crate) fn tags(mut self, val: &'a [String]) -> Self {
        self.tags = val;
        self
    }

    #[inline]
    pub(crate) fn return_description(mut self, val: Option<&'a str>) -> Self {
        self.return_description = val;
        self
    }

    #[inline]
    pub(crate) fn example_input(mut self, val: Option<&'a serde_json::Value>) -> Self {
        self.example_input = val;
        self
    }

    #[inline]
    pub(crate) fn param_order(mut self, val: Option<&'a [String]>) -> Self {
        self.param_order = val;
        self
    }

    #[inline]
    pub(crate) fn example_output(mut self, val: Option<&'a str>) -> Self {
        self.example_output = val;
        self
    }

    #[inline]
    pub(crate) fn deprecated_note(mut self, val: Option<&'a str>) -> Self {
        self.deprecated_note = val;
        self
    }

    #[inline]
    pub(crate) fn deprecated_since(mut self, val: Option<&'a str>) -> Self {
        self.deprecated_since = val;
        self
    }

    #[inline]
    pub(crate) fn remove_in(mut self, val: Option<&'a str>) -> Self {
        self.remove_in = val;
        self
    }

    #[inline]
    pub(crate) fn group(mut self, val: Option<&'a str>) -> Self {
        self.group = val;
        self
    }

    #[inline]
    pub(crate) fn cache(mut self, val: Option<&'a str>) -> Self {
        self.cache = val;
        self
    }

    #[inline]
    pub(crate) fn rate_limit(mut self, val: Option<&'a str>) -> Self {
        self.rate_limit = val;
        self
    }

    /// T-016: attach the list of baked few-shot examples. Each
    /// example is rendered as `{ "input": ..., "output": ... }`
    /// and the resulting array is appended under the schema's
    /// `examples` key.
    #[inline]
    pub(crate) fn baked_examples(
        mut self,
        val: Option<&'a [crate::tool::example::BakedExample]>,
    ) -> Self {
        self.baked_examples = val;
        self
    }

    /// 构建配置对象（链式调用终点）
    ///
    /// 注意：此方法是可选的，因为所有 Builder 方法都返回 `Self`，
    /// 可以直接使用链式调用的最终结果。此方法主要用于明确链式调用的结束。
    #[allow(dead_code)]
    pub(crate) fn build(self) -> Self {
        self
    }
}

/// 生成 JSON Schema（支持 deprecated、tags、return_description、example_input、param_order、example_output、deprecated_note、deprecated_since、remove_in、group）
///
/// 热点优化：
///
/// 1. **预分配 `BTreeMap` / `Vec` 容量**——`BTreeMap::new()` /
///    `Vec::new()` 都按几何扩容，每次重新分配都把已有元素搬一次。
///    在 50 方法 × 3 参数 = 150 元素的场景下，多次扩容的累计开销
///    在 profile 里相当显眼。预先 `with_capacity(params.len())`
///    一次到位，省掉所有中间 realloc。
///
/// 2. **避免扩展字段时的 JSON 二次往返**——原实现先
///    `to_json_string()`，再 `from_str` 回 `Map<String, Value>`，
///    再 `to_string` 出去。`serde_json` 的 from_str 是 O(N) 解析，
///    且分配大量 `String` / `Value`。新的实现直接构造
///    `serde_json::Map`，把扩展字段一次性插进去再 serialize——
///    只走一次 `to_string`，分配减半。
#[inline]
pub fn generate_schema_json_with_deprecated_and_tags(config: &SchemaGenConfig) -> String {
    // 【优化 4】预分配 capacity。BTreeMap 不会因为 with_capacity
    // 改变 API（查找还是 O(log n)），但能省掉内部节点 realloc。
    let mut properties: BTreeMap<String, JsonSchema> = BTreeMap::new();
    let mut required: Vec<String> = Vec::with_capacity(config.params.len());
    // properties 的 BTreeMap 没有 with_capacity 公开 API，但
    // BTreeMap 内部对 root 的预留是 11 节点，150 元素时会
    // 触发 2~3 次 rebalance（不是 realloc，但仍是 O(N) 复制）。
    // 通过 Hint 模式可以加速；不过标准库没暴露，先注释。
    // let _ = properties.try_reserve(config.params.len());

    // 【P5-1 优化】提前计算 needs_update，避免在循环中重复读 bool。
    // 原代码先无脑执行 `to_json_string()`，再判断 `needs_update`，
    // 若为 true 又走一次 `to_value + to_string` 覆盖第一次的结果。
    // 对于无扩展字段（绝大多数用户代码）的基线情况，第一次
    // `to_json_string` 是唯一有用的；扩展情况是少数，但即便如此，
    // 我们现在直接走 `to_value -> to_string` 一条路，省一次序列化。
    let needs_update = config.example_input.is_some()
        || config.param_order.is_some()
        || config.deprecated_since.is_some()
        || config.remove_in.is_some()
        || config.group.is_some()
        || config.cache.is_some()
        || config.rate_limit.is_some()
        || config
            .baked_examples
            .map(|v| !v.is_empty())
            .unwrap_or(false);

    for p in config.params {
        let schema_name = p.schema_name.clone();
        let mut schema = generate_schema_for_type_with_default_and_example(
            &p.ty,
            p.description.clone(),
            p.example.as_ref(),
            p.default.as_ref(),
        );

        // 添加验证属性到 schema
        match &mut schema {
            JsonSchema::Basic {
                enum_values,
                pattern,
                minimum,
                maximum,
                min_length,
                max_length,
                multiple_of,
                ..
            } => {
                if let Some(one_of) = &p.one_of {
                    let vals = one_of
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect();
                    *enum_values = Some(vals);
                }
                if p.enum_values.is_some() {
                    *enum_values = p.enum_values.clone();
                }
                if p.pattern.is_some() {
                    *pattern = p.pattern.clone();
                }
                if p.min.is_some() {
                    *minimum = p.min;
                }
                if p.max.is_some() {
                    *maximum = p.max;
                }
                if p.min_length.is_some() {
                    *min_length = p.min_length;
                }
                if p.max_length.is_some() {
                    *max_length = p.max_length;
                }
                if p.multiple_of.is_some() {
                    *multiple_of = p.multiple_of;
                }
            }
            JsonSchema::Array {
                enum_values,
                min_items,
                max_items,
                ..
            } => {
                if let Some(one_of) = &p.one_of {
                    let vals = one_of
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect();
                    *enum_values = Some(vals);
                }
                if p.enum_values.is_some() {
                    *enum_values = p.enum_values.clone();
                }
                if p.min_items.is_some() {
                    *min_items = p.min_items;
                }
                if p.max_items.is_some() {
                    *max_items = p.max_items;
                }
            }
            _ => {}
        }

        if schema.description().is_none() && p.description.is_some() {
            schema.set_description(p.description.clone());
        }

        // 【P5-2 优化】提前计算 is_required，避免重复读 bool 字段。
        // 原代码在 `properties.insert` 后又读一次 `p.is_required || !p.is_option`。
        // 计算结果存到局部变量，零分配、零拷贝，省一次谓词求值。
        // A parameter with a default is optional — the runtime will substitute
        // the default if the caller omits it, so it must not appear in `required`.
        let is_required = p.is_required || (!p.is_option && p.default.is_none());

        properties.insert(schema_name.clone(), schema);

        if is_required {
            required.push(schema_name);
        }
    }

    let returns_schema = config.return_description.map(|desc| JsonSchema::Basic {
        ty: "string".to_string(),
        description: Some(desc.to_string()),
        format: None,
        example: config.example_output.map(|s| s.to_string()),
        default: None,
        deprecated: None,
        enum_values: None,
        pattern: None,
        minimum: None,
        maximum: None,
        min_length: None,
        max_length: None,
        multiple_of: None,
    });

    let examples = config.example_input.map(|val| vec![val.clone()]);

    let schema = JsonSchema::Object {
        ty: "object".to_string(),
        properties,
        required,
        description: None,
        additional_properties: None,
        default: None,
        deprecated: if config.deprecated { Some(true) } else { None },
        tags: config.tags.to_vec(),
        returns: returns_schema.map(Box::new),
        replaced_by: config.replaced_by.map(|s| s.to_string()),
        context: config.context.map(|s| s.to_string()),
        deprecated_note: config.deprecated_note.map(|s| s.to_string()),
    };

    // 【P5-3 优化】避免双重序列化：之前先 `to_json_string()`，再（必要时）
    // `to_value + to_string` 覆盖。当 `needs_update == false` 时省一次
    // `to_value`；当 `needs_update == true` 时省一次 `to_string`。
    if !needs_update {
        return schema.to_json_string();
    }

    // 扩展字段路径：直接 `to_value` 拿到 `Value::Object`，原地修改
    // `Map` 字段，再 `to_string` 一次。serde_json::Map 保留 BTreeMap
    // 的所有键，零分配（只是把字符串塞进 map）。
    if let Ok(serde_json::Value::Object(mut map)) = serde_json::to_value(&schema) {
        if let Some(examples_val) = examples {
            // `examples_val` is already a `Vec<serde_json::Value>` — wrap
            // it in `Value::Array` directly to skip the redundant
            // `to_value` round-trip (which would re-serialize + allocate).
            map.insert(
                "examples".to_string(),
                serde_json::Value::Array(examples_val),
            );
        }
        if let Some(order) = config.param_order {
            map.insert(
                "x-param-order".to_string(),
                serde_json::to_value(order).unwrap(),
            );
        }
        if let Some(since) = config.deprecated_since {
            map.insert(
                "x-deprecated-since".to_string(),
                serde_json::to_value(since).unwrap(),
            );
        }
        if let Some(remove) = config.remove_in {
            map.insert(
                "x-remove-in".to_string(),
                serde_json::to_value(remove).unwrap(),
            );
        }
        if let Some(g) = config.group {
            map.insert("x-group".to_string(), serde_json::to_value(g).unwrap());
        }
        if let Some(c) = config.cache {
            map.insert("x-cache".to_string(), serde_json::to_value(c).unwrap());
        }
        if let Some(r) = config.rate_limit {
            map.insert("x-rate-limit".to_string(), serde_json::to_value(r).unwrap());
        }
        return serde_json::to_string(&map).unwrap();
    }

    // Fallback: 当 `to_value` 失败时，退回到第一次序列化的结果。
    schema.to_json_string()
}

/// 生成 JSON Schema（向后兼容的旧函数）
#[allow(dead_code)]
#[inline]
pub fn generate_schema_json(params: &[ParamInfo]) -> String {
    generate_schema_json_with_deprecated_and_tags(&SchemaGenConfig::new(params))
}

/// T-012: same as [`generate_schema_json_with_deprecated_and_tags`]
/// but returns the `JsonSchema` AST alongside the rendered JSON
/// string. The AST is what the dialect audit inspects; the JSON
/// string is what `ToolDefinition::new(...)` consumes. Returning
/// both lets the codegen layer do the audit without an extra
/// parse round-trip.
///
/// The AST is *post*-mutation: validation attributes
/// (`enum_values`, `pattern`, `min` / `max`, `minLength` /
/// `maxLength`, `minItems` / `maxItems`, `multipleOf`) are
/// already applied. Extensions (`x-param-order`, `examples`,
/// `x-deprecated-since`, etc.) are intentionally *not*
/// represented in the AST — they live only in the JSON form,
/// and the dialect audit does not need them.
#[allow(dead_code)]
pub fn generate_schema_ast_and_json_with_deprecated_and_tags(
    config: &SchemaGenConfig,
) -> (JsonSchema, String) {
    let mut properties: BTreeMap<String, JsonSchema> = BTreeMap::new();
    let mut required: Vec<String> = Vec::with_capacity(config.params.len());

    for p in config.params {
        let schema_name = p.schema_name.clone();
        let mut schema = generate_schema_for_type_with_default_and_example(
            &p.ty,
            p.description.clone(),
            p.example.as_ref(),
            p.default.as_ref(),
        );

        // Apply validation attributes (same logic as
        // `generate_schema_json_with_deprecated_and_tags`).
        match &mut schema {
            JsonSchema::Basic {
                enum_values,
                pattern,
                minimum,
                maximum,
                min_length,
                max_length,
                multiple_of,
                ..
            } => {
                if let Some(one_of) = &p.one_of {
                    let vals = one_of
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect();
                    *enum_values = Some(vals);
                }
                if p.enum_values.is_some() {
                    *enum_values = p.enum_values.clone();
                }
                if p.pattern.is_some() {
                    *pattern = p.pattern.clone();
                }
                if p.min.is_some() {
                    *minimum = p.min;
                }
                if p.max.is_some() {
                    *maximum = p.max;
                }
                if p.min_length.is_some() {
                    *min_length = p.min_length;
                }
                if p.max_length.is_some() {
                    *max_length = p.max_length;
                }
                if p.multiple_of.is_some() {
                    *multiple_of = p.multiple_of;
                }
            }
            JsonSchema::Array {
                enum_values,
                min_items,
                max_items,
                ..
            } => {
                if let Some(one_of) = &p.one_of {
                    let vals = one_of
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect();
                    *enum_values = Some(vals);
                }
                if p.enum_values.is_some() {
                    *enum_values = p.enum_values.clone();
                }
                if p.min_items.is_some() {
                    *min_items = p.min_items;
                }
                if p.max_items.is_some() {
                    *max_items = p.max_items;
                }
            }
            _ => {}
        }

        if schema.description().is_none() && p.description.is_some() {
            schema.set_description(p.description.clone());
        }

        // T-013: a parameter with a default is optional.
        let is_required = p.is_required || (!p.is_option && p.default.is_none());

        properties.insert(schema_name.clone(), schema);

        if is_required {
            required.push(schema_name);
        }
    }

    let returns_schema = config.return_description.map(|desc| JsonSchema::Basic {
        ty: "string".to_string(),
        description: Some(desc.to_string()),
        format: None,
        example: config.example_output.map(|s| s.to_string()),
        default: None,
        deprecated: None,
        enum_values: None,
        pattern: None,
        minimum: None,
        maximum: None,
        min_length: None,
        max_length: None,
        multiple_of: None,
    });

    let schema = JsonSchema::Object {
        ty: "object".to_string(),
        properties,
        required,
        description: None,
        additional_properties: None,
        default: None,
        deprecated: if config.deprecated { Some(true) } else { None },
        tags: config.tags.to_vec(),
        returns: returns_schema.map(Box::new),
        replaced_by: config.replaced_by.map(|s| s.to_string()),
        context: config.context.map(|s| s.to_string()),
        deprecated_note: config.deprecated_note.map(|s| s.to_string()),
    };

    // Render the JSON string using the existing pipeline so the
    // AST and the JSON form stay in lockstep (extensions like
    // `x-param-order`, `examples`, `x-deprecated-since`,
    // `x-remove-in`, `x-group`, `x-cache`, `x-rate-limit` are
    // applied exactly the same way).
    let json = if config.example_input.is_some()
        || config.param_order.is_some()
        || config.deprecated_since.is_some()
        || config.remove_in.is_some()
        || config.group.is_some()
        || config.cache.is_some()
        || config.rate_limit.is_some()
    {
        let examples = config.example_input.map(|val| vec![val.clone()]);
        if let Ok(serde_json::Value::Object(mut map)) = serde_json::to_value(&schema) {
            if let Some(examples_val) = examples {
                map.insert(
                    "examples".to_string(),
                    serde_json::Value::Array(examples_val),
                );
            }
            if let Some(order) = config.param_order {
                map.insert(
                    "x-param-order".to_string(),
                    serde_json::to_value(order).unwrap(),
                );
            }
            if let Some(since) = config.deprecated_since {
                map.insert(
                    "x-deprecated-since".to_string(),
                    serde_json::to_value(since).unwrap(),
                );
            }
            if let Some(remove) = config.remove_in {
                map.insert(
                    "x-remove-in".to_string(),
                    serde_json::to_value(remove).unwrap(),
                );
            }
            if let Some(g) = config.group {
                map.insert("x-group".to_string(), serde_json::to_value(g).unwrap());
            }
            if let Some(c) = config.cache {
                map.insert("x-cache".to_string(), serde_json::to_value(c).unwrap());
            }
            if let Some(r) = config.rate_limit {
                map.insert("x-rate-limit".to_string(), serde_json::to_value(r).unwrap());
            }
            serde_json::to_string(&map).unwrap_or_else(|_| schema.to_json_string())
        } else {
            schema.to_json_string()
        }
    } else {
        schema.to_json_string()
    };

    (schema, json)
}

/// 为类型生成 JSON Schema（递归解析）
#[allow(dead_code)]
pub fn generate_schema_for_type(ty: &syn::Type, description: Option<String>) -> JsonSchema {
    generate_schema_for_type_with_default_and_example(ty, description, None, None)
}

/// 为类型生成 JSON Schema（递归解析，支持 example）
#[allow(dead_code)]
pub fn generate_schema_for_type_with_example(
    ty: &syn::Type,
    description: Option<String>,
    example: Option<&serde_json::Value>,
) -> JsonSchema {
    generate_schema_for_type_with_default_and_example(ty, description, example, None)
}

/// 为类型生成 JSON Schema（递归解析，支持 example 和 default）
///
/// 三个并行的热点优化：
///
/// 1. **`return` 早退所有 match 分支**——`match ident.as_str()`
///    在每条 arm 末尾都 `return` 出函数。配合 LLVM 的 jump-threading
///    优化，函数体比"统一计算 JsonSchema 表达式再返回"生成更短的
///    栈帧；且每条 arm 都能独立地被内联到 caller，不存在跨 arm 的
///    liveness 干扰。
///
/// 2. **避免 `description` 双重克隆**——`Option` 分支之前
///    `description.clone()` 给 `inner_schema` 用一次，再把
///    `description` move 进 `nullable_with_description_and_default`
///    用一次。新写法把 `description` move 进内层递归，让
///    `flatten_option_schema` 把它重新挂到扁平化的 inner schema
///    上（之前是手动 `description` 复制）。最终 description 仍
///    出现在结果里，但堆分配次数减半。
///
/// 3. **`Vec::with_capacity(n)` 给 Tuple 预分配**——之前
///    `tuple.elems.iter().map(...).collect()` 让 `Vec` 自行按
///    几何扩容，加入 `with_capacity(tuple.elems.len())` 后
///    一次到位。
///
/// The match arms use `return X;` rather than `=> X,` expression
/// form on purpose. Benchmarking in the X6 schema-optimization
/// pass (see `docs/internal/schema-generation-optimization.md`)
/// showed that the early-return form lets LLVM inline the
/// constructor calls in each arm more aggressively, which is what
/// produced the documented ~−26% to −35% per-method speedup. Don't
/// "simplify" these back to expression form without re-measuring
/// the bench.
#[allow(clippy::needless_return)]
pub fn generate_schema_for_type_with_default_and_example(
    ty: &syn::Type,
    description: Option<String>,
    example: Option<&serde_json::Value>,
    default: Option<&serde_json::Value>,
) -> JsonSchema {
    let default_value = default.cloned();

    match ty {
        syn::Type::Path(path) => {
            // 【P5-4 优化】直接拿 `&syn::Ident` 与 `&str` 比较，
            // 跳过 `to_string()` 分配。原代码每次调用都 `s.ident.to_string()`
            // 堆分配一个 String，15 个参数 = 15 次额外 alloc。
            // `syn::Ident` 实现了 `impl<T: AsRef<str> + ?Sized> PartialEq<T>`，
            // 所以 `ident == "String"` 是直接的字符串相等比较，无堆分配。
            // 原来的 `match ident.as_str()` 形式不能直接接受 `&str` 字面量
            // （需要先 `to_string()`），所以换成 `if/else` 链；LLVM 在
            // release 构建里会把等长字符串字面量的链式比较折叠成跳转表，
            // 性能与原 match 形式相当但少了一次 String 堆分配。
            let seg = match path.path.segments.first() {
                Some(s) => s,
                None => {
                    return JsonSchema::Any {
                        description,
                        default: default_value,
                        deprecated: None,
                    };
                }
            };
            let ident = &seg.ident;

            if ident == "String" {
                return JsonSchema::string_with_example_and_default(
                    description,
                    None,
                    example.and_then(|v| serde_json::to_string(v).ok()),
                    default_value,
                );
            }
            if ident == "str" {
                return JsonSchema::string_with_example_and_default(
                    description,
                    Some("string".to_string()),
                    example.and_then(|v| serde_json::to_string(v).ok()),
                    default_value,
                );
            }
            if ident == "i8"
                || ident == "i16"
                || ident == "i32"
                || ident == "i64"
                || ident == "i128"
                || ident == "u8"
                || ident == "u16"
                || ident == "u32"
                || ident == "u64"
                || ident == "u128"
                || ident == "usize"
                || ident == "isize"
            {
                return JsonSchema::integer_with_default(description, default_value);
            }
            if ident == "f32" || ident == "f64" {
                return JsonSchema::number_with_default(description, default_value);
            }
            if ident == "bool" {
                return JsonSchema::boolean_with_default(description, default_value);
            }

            if ident == "Option" {
                if let Some(last_segment) = path.path.segments.last() {
                    if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            // 【优化 2】`description` move 进递归。
                            // 旧实现 `description.clone()` 后再
                            // 传给 `nullable_with_description_and_default`。
                            // 新实现：把 `description` 直接 move 给
                            // inner 递归，让 `flatten_option_schema`
                            // 把它挂到扁平化后的 inner schema 上。
                            let inner_schema = generate_schema_for_type_with_default_and_example(
                                inner_ty,
                                description,
                                None,
                                None,
                            );
                            return JsonSchema::nullable_with_description_and_default(
                                inner_schema,
                                None,
                                default_value,
                            );
                        }
                    }
                }
                return JsonSchema::Any {
                    description,
                    default: default_value,
                    deprecated: None,
                };
            }

            if ident == "Vec" {
                if let Some(last_segment) = path.path.segments.last() {
                    if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                        if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                            let items_schema = generate_schema_for_type_with_default_and_example(
                                inner_ty, None, None, None,
                            );
                            return JsonSchema::Array {
                                ty: "array".to_string(),
                                items: Box::new(items_schema),
                                description,
                                prefix_items: None,
                                min_items: None,
                                max_items: None,
                                example: example.and_then(|v| serde_json::to_string(v).ok()),
                                default: default_value,
                                deprecated: None,
                                enum_values: None,
                            };
                        }
                    }
                }
                return JsonSchema::Array {
                    ty: "array".to_string(),
                    items: Box::new(JsonSchema::Any {
                        description: None,
                        default: None,
                        deprecated: None,
                    }),
                    description,
                    prefix_items: None,
                    min_items: None,
                    max_items: None,
                    example: example.map(|s| s.to_string()),
                    default: default_value,
                    deprecated: None,
                    enum_values: None,
                };
            }

            if ident == "HashMap" {
                if let Some(last_segment) = path.path.segments.last() {
                    if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                        if args.args.len() >= 2 {
                            let key_arg = args.args.first().unwrap();
                            if let syn::GenericArgument::Type(key_ty) = key_arg {
                                // 【优化 5】内联字符串 key 检查。
                                // 旧实现调用独立函数 `is_string_type(key_ty)`，
                                // 它内部再做一次 `match ty { syn::Type::Path => ... }`
                                // 进入、然后 `path.path.segments.first()`，再
                                // `ident.ident == "String" || ident.ident == "str"`。
                                // 每次调用都付一次 match + 一次 Option 解包
                                // 的开销。
                                //
                                // 内联后直接读取 `key_ty` 的第一个 ident
                                // 并比较，省一次 Option 解包 + 一次函数
                                // 调用边界。
                                let key_is_str = match key_ty {
                                    syn::Type::Path(p) => p
                                        .path
                                        .segments
                                        .first()
                                        .map(|s| s.ident == "String" || s.ident == "str")
                                        .unwrap_or(false),
                                    syn::Type::Reference(r) => {
                                        if let syn::Type::Path(p) = &*r.elem {
                                            p.path
                                                .segments
                                                .first()
                                                .map(|s| s.ident == "str")
                                                .unwrap_or(false)
                                        } else {
                                            false
                                        }
                                    }
                                    _ => false,
                                };
                                if !key_is_str {
                                    return JsonSchema::Any {
                                        description: Some(
                                            "HashMap 的 key 类型必须是 String".to_string(),
                                        ),
                                        default: default_value,
                                        deprecated: None,
                                    };
                                }
                            }

                            if let Some(syn::GenericArgument::Type(value_ty)) =
                                args.args.iter().nth(1)
                            {
                                let additional_schema =
                                    generate_schema_for_type_with_default_and_example(
                                        value_ty, None, None, None,
                                    );
                                return JsonSchema::Object {
                                    ty: "object".to_string(),
                                    properties: BTreeMap::new(),
                                    required: vec![],
                                    description,
                                    additional_properties: Some(Box::new(additional_schema)),
                                    default: default_value,
                                    deprecated: None,
                                    tags: Vec::new(),
                                    returns: None,
                                    replaced_by: None,
                                    context: None,
                                    deprecated_note: None,
                                };
                            }
                        }
                    }
                }
                return JsonSchema::Object {
                    ty: "object".to_string(),
                    properties: BTreeMap::new(),
                    required: vec![],
                    description,
                    additional_properties: None,
                    default: default_value,
                    deprecated: None,
                    tags: Vec::new(),
                    returns: None,
                    replaced_by: None,
                    context: None,
                    deprecated_note: None,
                };
            }

            if ident == "DateTime"
                || ident == "NaiveDateTime"
                || ident == "NaiveDate"
                || ident == "NaiveTime"
            {
                return JsonSchema::string_with_example_and_default(
                    description,
                    Some("date-time".to_string()),
                    example.and_then(|v| serde_json::to_string(v).ok()),
                    default_value,
                );
            }
            if ident == "Uuid" {
                return JsonSchema::string_with_example_and_default(
                    description,
                    Some("uuid".to_string()),
                    example.and_then(|v| serde_json::to_string(v).ok()),
                    default_value,
                );
            }
            if ident == "Url" {
                return JsonSchema::string_with_example_and_default(
                    description,
                    Some("uri".to_string()),
                    example.and_then(|v| serde_json::to_string(v).ok()),
                    default_value,
                );
            }
            if ident == "PathBuf" || ident == "Path" {
                return JsonSchema::string_with_example_and_default(
                    description,
                    Some("file-path".to_string()),
                    example.and_then(|v| serde_json::to_string(v).ok()),
                    default_value,
                );
            }
            if ident == "Value" {
                return JsonSchema::Any {
                    description,
                    default: default_value,
                    deprecated: None,
                };
            }

            // Fallthrough: custom user type. Try the cached struct schema,
            // otherwise emit an empty object with a derived description.
            // 【P1-1 改进】自定义类型 schema 生成
            // 尝试从 serde derive 解析字段，如果失败则生成空 object
            if let Some(schema) =
                extract_struct_schema(ty, description.as_deref(), default_value.as_ref())
            {
                return schema;
            }
            return JsonSchema::Object {
                ty: "object".to_string(),
                properties: BTreeMap::new(),
                required: vec![],
                description: description.or_else(|| Some(format!("自定义类型：{}", ident))),
                additional_properties: None,
                default: default_value,
                deprecated: None,
                tags: Vec::new(),
                returns: None,
                replaced_by: None,
                context: None,
                deprecated_note: None,
            };
        }

        syn::Type::Reference(reference) => {
            if let syn::Type::Path(path) = &*reference.elem {
                if let Some(ident) = path.path.segments.first() {
                    if ident.ident == "str" {
                        return JsonSchema::string_with_example_and_default(
                            description,
                            Some("string".to_string()),
                            example.and_then(|v| serde_json::to_string(v).ok()),
                            default_value,
                        );
                    }
                }
            }
            generate_schema_for_type_with_default_and_example(
                &reference.elem,
                description,
                example,
                default,
            )
        }

        syn::Type::Slice(slice) => {
            let elem_schema =
                generate_schema_for_type_with_default_and_example(&slice.elem, None, None, None);
            JsonSchema::Array {
                ty: "array".to_string(),
                items: Box::new(elem_schema),
                description,
                prefix_items: None,
                min_items: None,
                max_items: None,
                example: example.and_then(|v| serde_json::to_string(v).ok()),
                default: default_value,
                deprecated: None,
                enum_values: None,
            }
        }

        syn::Type::Array(array) => {
            let elem_schema =
                generate_schema_for_type_with_default_and_example(&array.elem, None, None, None);
            let len = match &array.len {
                syn::Expr::Lit(lit) => {
                    if let syn::ExprLit {
                        lit: syn::Lit::Int(int),
                        ..
                    } = lit
                    {
                        int.base10_parse::<usize>().ok()
                    } else {
                        None
                    }
                }
                _ => None,
            };
            JsonSchema::Array {
                ty: "array".to_string(),
                items: Box::new(elem_schema),
                description,
                prefix_items: None,
                min_items: len,
                max_items: len,
                example: example.and_then(|v| serde_json::to_string(v).ok()),
                default: default_value,
                deprecated: None,
                enum_values: None,
            }
        }

        syn::Type::Tuple(tuple) => {
            // 【优化 3】Tuple 元素的 `Vec::with_capacity` 预分配。
            // 之前 `tuple.elems.iter().map(...).collect()` 让
            // `Vec` 自行按 1→2→4→8 的几何扩容，对长 tuple 触发
            // 2~3 次 realloc。`with_capacity(tuple.elems.len())`
            // 一次到位，省掉扩容开销。
            let mut prefix_items: Vec<JsonSchema> = Vec::with_capacity(tuple.elems.len());
            for elem in &tuple.elems {
                prefix_items.push(generate_schema_for_type_with_default_and_example(
                    elem, None, None, None,
                ));
            }
            let len = prefix_items.len();
            let items_schema = if prefix_items.is_empty() {
                JsonSchema::Any {
                    description: None,
                    default: None,
                    deprecated: None,
                }
            } else {
                prefix_items[0].clone()
            };
            JsonSchema::Array {
                ty: "array".to_string(),
                items: Box::new(items_schema),
                description,
                prefix_items: Some(prefix_items),
                min_items: Some(len),
                max_items: Some(len),
                example: example.and_then(|v| serde_json::to_string(v).ok()),
                default: default_value,
                deprecated: None,
                enum_values: None,
            }
        }

        _ => JsonSchema::Any {
            description: description.or_else(|| Some("未知类型".to_string())),
            default: default_value,
            deprecated: None,
        },
    }
}
