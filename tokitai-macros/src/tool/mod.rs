//! `#[tool]` 宏实现
//!
//! 核心设计：
//! 1. 单一宏同时处理 impl 块和方法
//! 2. 编译期生成所有工具定义
//! 3. 使用 JsonSchema AST + serde_json 生成规范的 JSON Schema
//! 4. 支持自定义 struct 字段解析
//!
//! 警告控制：
//! - 测试环境下自动抑制警告
//! - 可通过环境变量 `TOKITAI_SHOW_WARNINGS=1` 启用警告输出

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use syn::{parse_macro_input, parse_quote, ImplItem, ItemImpl, ItemStruct};

pub(crate) mod attrs;
pub(crate) mod codegen;
pub(crate) mod config;
pub(crate) mod extract;
pub(crate) mod schema;
pub(crate) mod types;

use attrs::method::ToolAttributes;
use codegen::{definitions, dispatcher, wrappers};
use extract::collect_tool_methods;

/// 检查是否应该显示警告
///
/// 测试环境下默认抑制警告
/// 可通过环境变量 `TOKITAI_SHOW_WARNINGS=1` 启用警告
/// 或通过 `TOKITAI_QUIET=1` 禁用警告
fn should_show_warnings() -> bool {
    // 检查是否显式启用了警告
    if option_env!("TOKITAI_SHOW_WARNINGS").is_some() {
        return true;
    }
    
    // 检查是否显式禁用了警告
    if option_env!("TOKITAI_QUIET").is_some() {
        return false;
    }
    
    // 默认行为：显示警告
    // 用户可以通过设置 TOKITAI_QUIET=1 来禁用警告
    true
}

/// `#[tool]` 宏入口
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    // 尝试解析为 impl 块
    if let Ok(impl_item) = syn::parse::<ItemImpl>(item.clone()) {
        let attr_args = parse_macro_input!(attr as ToolAttributes);
        generate_for_impl(impl_item, attr_args).into()
    }
    // 尝试解析为 struct（用于标记工具提供者类型）
    else if let Ok(_struct_item) = syn::parse::<ItemStruct>(item.clone()) {
        // struct 上不需要生成代码，直接返回
        item
    }
    // 其他情况直接返回
    else {
        item
    }
}

/// `#[tool_type]` 宏入口 - 用于注册自定义类型的 schema
pub fn tool_type(attr: TokenStream, item: TokenStream) -> TokenStream {
    let struct_name = if let Ok(struct_item) = syn::parse::<ItemStruct>(item.clone()) {
        struct_item.ident.to_string()
    } else {
        return item;
    };

    if let Ok(schema_attrs) = syn::parse::<ToolTypeAttrs>(attr) {
        let schema = schema_attrs.to_json_schema();

        if let Ok(mut cache) = schema::cache::TYPE_SCHEMA_CACHE.lock() {
            cache.insert(struct_name, schema);
        }
    }

    item
}

/// `#[tool_type]` 属性参数
struct ToolTypeAttrs {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    properties: Vec<(String, String)>,
    #[allow(dead_code)]
    required: Vec<String>,
}

impl syn::parse::Parse for ToolTypeAttrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut properties = Vec::new();
        let mut required = Vec::new();

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::token::Eq>()?;

            match key.to_string().as_str() {
                "name" => {
                    let value: syn::LitStr = input.parse()?;
                    name = Some(value.value());
                }
                "properties" => {
                    let value: syn::LitStr = input.parse()?;
                    for prop in value.value().split(',') {
                        let parts: Vec<&str> = prop.trim().split(':').collect();
                        if parts.len() == 2 {
                            properties
                                .push((parts[0].trim().to_string(), parts[1].trim().to_string()));
                        }
                    }
                }
                "required" => {
                    let value: syn::LitStr = input.parse()?;
                    required = value
                        .value()
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect();
                }
                _ => {
                    let _value: syn::LitStr = input.parse()?;
                }
            }

            if input.peek(syn::token::Comma) {
                input.parse::<syn::token::Comma>()?;
            }
        }

        Ok(ToolTypeAttrs {
            name: name.unwrap_or_default(),
            properties,
            required,
        })
    }
}

impl ToolTypeAttrs {
    fn to_json_schema(&self) -> schema::types::JsonSchema {
        use std::collections::BTreeMap;

        let properties: BTreeMap<String, schema::types::JsonSchema> = self
            .properties
            .iter()
            .map(|(name, ty)| {
                let schema = match ty.as_str() {
                    "string" => schema::types::JsonSchema::string(None, None),
                    "integer" => schema::types::JsonSchema::integer(None),
                    "number" => schema::types::JsonSchema::number(None),
                    "boolean" => schema::types::JsonSchema::boolean(None),
                    "array" => schema::types::JsonSchema::Array {
                        ty: "array".to_string(),
                        items: Box::new(schema::types::JsonSchema::Any {
                            description: None,
                            default: None,
                            deprecated: None,
                        }),
                        description: None,
                        prefix_items: None,
                        min_items: None,
                        max_items: None,
                        example: None,
                        default: None,
                        deprecated: None,
                        enum_values: None,
                    },
                    "object" => schema::types::JsonSchema::Object {
                        ty: "object".to_string(),
                        properties: BTreeMap::new(),
                        required: vec![],
                        description: None,
                        additional_properties: None,
                        default: None,
                        deprecated: None,
                        tags: Vec::new(),
                        returns: None,
                        replaced_by: None,
                        context: None,
                        deprecated_note: None,
                    },
                    _ => schema::types::JsonSchema::Any {
                        description: None,
                        default: None,
                        deprecated: None,
                    },
                };
                (name.clone(), schema)
            })
            .collect();

        schema::types::JsonSchema::Object {
            ty: "object".to_string(),
            properties,
            required: self.required.clone(),
            description: None,
            additional_properties: None,
            default: None,
            deprecated: None,
            tags: Vec::new(),
            returns: None,
            replaced_by: None,
            context: None,
            deprecated_note: None,
        }
    }
}

/// impl 块级别的工具属性
fn generate_for_impl(mut impl_item: ItemImpl, _attrs: ToolAttributes) -> TokenStream2 {
    let tool_methods = collect_tool_methods(&impl_item);

    if tool_methods.is_empty() {
        return quote! { #impl_item };
    }

    for tool in &tool_methods {
        if should_show_warnings()
            && tool.deprecated
            && tool.replaced_by.is_none()
            && !tool
                .allow
                .contains(&"deprecated_missing_replaced_by".to_string())
        {
            eprintln!(
                "[tokitai] [W001] deprecated method `{}` missing `replaced_by`",
                tool.name
            );
            eprintln!("  --> help: add `replaced_by = \"new_method\"`");
        }

        for param in &tool.params {
            if param.is_option
                && param.default.is_none()
                && param.example.is_none()
                && !tool.allow.contains(&"option_no_default".to_string())
                && should_show_warnings()
            {
                let display_name = &param.schema_name;
                eprintln!(
                    "[tokitai] [W002] optional param `{}` lacks default/example",
                    display_name
                );
                eprintln!("  --> help: add `#[tool(default_{0} = \"null\")]`", display_name);
            }
        }

        // 检查 context=async 与非异步方法的冲突
        if should_show_warnings()
            && tool.context.as_deref() == Some("async")
            && !tool.is_async
            && !tool.allow.contains(&"context_async_mismatch".to_string())
        {
            eprintln!(
                "[tokitai] [W003] method `{}` has `context=\"async\"` but is not async",
                tool.name
            );
            eprintln!("  --> help: use `async fn` or remove `context`");
        }
    }

    let impl_type = &impl_item.self_ty;
    let tool_def_consts = definitions::generate_tool_def_consts(&tool_methods);
    let all_tool_defs = definitions::generate_all_tool_defs_array(&tool_methods, impl_type);
    let call_tool_methods = dispatcher::generate_call_tool_method(&tool_methods);
    let helper_methods = wrappers::generate_helper_methods(&tool_methods);
    let tool_count_const = definitions::generate_tool_count_const(&tool_methods);

    let mut new_items: Vec<ImplItem> = impl_item.items.clone();

    // tool_def_consts 返回 TokenStream2，需要解析为 ImplItem
    for static_def in tool_def_consts {
        if let Ok(item) = syn::parse2::<ImplItem>(static_def) {
            new_items.push(item);
        }
    }

    // 【P3 优化】添加编译期工具计数常量
    if let Ok(item) = syn::parse2::<ImplItem>(tool_count_const) {
        new_items.push(item);
    }

    let all_tool_defs_tokens = &all_tool_defs;

    let get_tool_definitions_fn: ImplItem = parse_quote! {
        /// 所有工具定义（运行时初始化，支持配置覆盖）
        ///
        /// # 注意
        /// 此函数使用 `LazyLock` 进行延迟初始化。在初始化过程中会访问
        /// `GLOBAL_CONFIG_REGISTRY`，如果配置注册表也在 LazyLock 中初始化，
        /// 可能存在死锁风险。当前实现已确保初始化顺序安全。
        fn __get_tool_definitions() -> &'static [::tokitai_core::ToolDefinition] {
            static TOOLS: ::std::sync::LazyLock<::std::vec::Vec<::tokitai_core::ToolDefinition>> = ::std::sync::LazyLock::new(|| {
                let mut defs = ::std::vec::Vec::from([#(#all_tool_defs_tokens.clone()),*]);

                for def in &mut defs {
                    let configs = ::tokitai_core::GLOBAL_CONFIG_REGISTRY.get(&def.name);
                    if !configs.is_empty() {
                        def.apply_configs(&configs);
                    }
                }

                defs
            });

            &TOOLS
        }
    };
    new_items.push(get_tool_definitions_fn);

    for method in call_tool_methods {
        if let Ok(item) = syn::parse2::<ImplItem>(method) {
            new_items.push(item);
        }
    }

    for helper in helper_methods {
        if let Ok(item) = syn::parse2::<ImplItem>(helper) {
            new_items.push(item);
        }
    }

    new_items.push(parse_quote! {
        /// 配置工具属性（运行时覆盖）
        ///
        /// 此方法由 `tokitai!` 配置宏调用，用于在运行时覆盖工具定义。
        ///
        /// # 注意
        ///
        /// 此方法需要在首次访问工具定义前调用，否则配置可能不会生效。
        pub fn configure_tool(_tool_name: &str, _configs: &[::tokitai_core::ToolConfig]) {
            ::tokitai_core::GLOBAL_CONFIG_REGISTRY.configure(_tool_name, _configs);
            let _ = Self::__get_tool_definitions();
        }
    });

    impl_item.items = new_items;

    let impl_type = &impl_item.self_ty;

    // ToolCaller trait 实现 - 直接委托给 impl 块中生成的 call_tool 方法
    // 使用完全限定语法避免递归调用
    let tool_caller_impl = quote! {
        impl ::tokitai_core::ToolCaller for #impl_type {
            fn call_tool(&self, name: &str, args: &::tokitai_core::serde_types::Value) -> Result<::tokitai_core::serde_types::Value, ::tokitai_core::ToolError> {
                // 直接调用 impl 块中生成的 call_tool 方法
                // Rust 方法解析规则会优先选择 impl 块中的具体方法
                self.call_tool(name, args)
            }
        }
    };

    quote! {
        #impl_item

        impl ::tokitai_core::ToolProvider for #impl_type {
            fn tool_definitions() -> &'static [::tokitai_core::ToolDefinition] {
                Self::__get_tool_definitions()
            }

            /// 【P3 优化】编译期工具计数
            fn tool_count() -> usize {
                Self::__TOOL_COUNT
            }
        }

        #tool_caller_impl
    }
}

/// 配置宏主函数
pub fn config(item: TokenStream) -> TokenStream {
    config::registry::config(item)
}
