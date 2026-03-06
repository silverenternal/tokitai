//! `#[tool]` 宏实现
//!
//! 核心设计：
//! 1. 单一宏同时处理 impl 块和方法
//! 2. 编译期生成所有工具定义
//! 3. 使用 `quote!` 直接生成 JSON 字符串，避免依赖 serde_json

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::{format_ident, quote};
use syn::{
    parse::{Parse, ParseStream},
    parse_macro_input, parse_quote,
    punctuated::Punctuated,
    token, Expr, FnArg, Ident, ImplItem, ImplItemFn, ItemImpl, Lit, LitStr, Meta,
    Pat, PatType, ReturnType, Type, Visibility,
};

/// `#[tool]` 宏入口
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    // 如果 attr 不为空，说明是方法级别的 #[tool(...)] 属性
    // 直接保留原样，由 impl 块级别的 #[tool] 处理
    if !attr.is_empty() {
        return item;
    }
    
    let attr_args = parse_macro_input!(attr as ToolAttributes);

    // 尝试解析为 impl 块
    if let Ok(impl_item) = syn::parse::<ItemImpl>(item.clone()) {
        generate_for_impl(impl_item, attr_args).into()
    } else {
        // 不是 impl 块，保留原样（可能是方法级别的属性）
        item
    }
}

/// impl 块级别的工具属性
#[derive(Default)]
struct ToolAttributes {
    #[allow(dead_code)]
    name: Option<String>,
    #[allow(dead_code)]
    description: Option<String>,
}

impl Parse for ToolAttributes {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut description = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<token::Eq>()?;

            let value: LitStr = input.parse()?;
            match key.to_string().as_str() {
                "name" => name = Some(value.value()),
                "desc" | "description" => description = Some(value.value()),
                _ => {}
            }

            if input.peek(token::Comma) {
                input.parse::<token::Comma>()?;
            }
        }

        Ok(ToolAttributes { name, description })
    }
}

/// 方法级别的工具属性
#[derive(Default)]
struct MethodToolAttrs {
    name: Option<String>,
    desc: Option<String>,
    skip: bool,
}

impl Parse for MethodToolAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // 检查第一个 token 是否是 skip
        if input.peek(Ident) {
            let key: Ident = input.fork().parse()?;
            if key == "skip" {
                // 消耗掉 skip
                input.parse::<Ident>()?;
                // 处理后面可能跟的逗号
                if input.peek(token::Comma) {
                    input.parse::<token::Comma>()?;
                }
                return Ok(MethodToolAttrs {
                    name: None,
                    desc: None,
                    skip: true,
                });
            }
        }

        // 否则解析 name = "..." desc = "..."
        let mut name = None;
        let mut desc = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<token::Eq>()?;
            let value: LitStr = input.parse()?;

            match key.to_string().as_str() {
                "name" => name = Some(value.value()),
                "desc" | "description" => desc = Some(value.value()),
                _ => {}
            }

            if input.peek(token::Comma) {
                input.parse::<token::Comma>()?;
            }
        }

        Ok(MethodToolAttrs { name, desc, skip: false })
    }
}

/// 为 impl 块生成代码
fn generate_for_impl(mut impl_item: ItemImpl, _attrs: ToolAttributes) -> TokenStream2 {
    let _impl_type = &impl_item.self_ty;

    // 收集所有工具方法
    let tool_methods = collect_tool_methods(&impl_item);

    if tool_methods.is_empty() {
        return quote! { #impl_item };
    }

    // 生成工具定义（编译期 const）
    let tool_def_consts = generate_tool_def_consts(&tool_methods);

    // 生成所有工具定义的数组
    let all_tool_defs = generate_all_tool_defs_array(&tool_methods);

    // 生成 call_tool 分发方法（可能是多个：异步 + 同步）
    let call_tool_methods = generate_call_tool_method(&tool_methods);

    // 生成参数解析辅助方法
    let helper_methods = generate_helper_methods(&tool_methods);

    // 添加生成的项到 impl 块
    let mut new_items: Vec<ImplItem> = impl_item.items.clone();

    // 添加 const 定义
    for const_def in tool_def_consts {
        new_items.push(parse_quote! { #const_def });
    }

    // 添加所有工具定义的 const 数组
    let all_tool_defs_tokens = &all_tool_defs;
    new_items.push(parse_quote! {
        /// 所有工具定义（编译期生成）
        pub const TOOL_DEFINITIONS: &'static [::tokitai::ToolDefinition] = &[#(#all_tool_defs_tokens),*];
    });

    // 添加 call_tool 方法（可能多个）
    for method in call_tool_methods {
        new_items.push(parse_quote! { #method });
    }

    // 添加辅助方法
    for helper in helper_methods {
        new_items.push(parse_quote! { #helper });
    }

    impl_item.items = new_items;

    quote! {
        #impl_item
    }
}

/// 收集所有被标记为工具的方法
fn collect_tool_methods(impl_item: &ItemImpl) -> Vec<ToolMethodInfo> {
    let mut tools = Vec::new();

    for item in &impl_item.items {
        if let ImplItem::Fn(fn_item) = item {
            // 检查是否是 pub 方法
            if !matches!(fn_item.vis, Visibility::Public(_)) {
                continue;
            }

            // 检查是否有 #[tool] 属性或应该自动包含
            if let Some(tool_info) = extract_tool_info(fn_item) {
                tools.push(tool_info);
            }
        }
    }

    tools
}

/// 提取工具方法信息
fn extract_tool_info(fn_item: &ImplItemFn) -> Option<ToolMethodInfo> {
    let method_name = fn_item.sig.ident.to_string();

    // 跳过内部方法（以 __ 开头）
    if method_name.starts_with("__") {
        return None;
    }

    // 检查是否有泛型参数（不支持）
    if !fn_item.sig.generics.params.is_empty() {
        return Some(ToolMethodInfo {
            name: method_name.clone(),
            tool_name: method_name.clone(),
            description: String::new(),
            params: vec![],
            is_async: false,
            is_result: false,
            is_generic: true,
            return_type: fn_item.sig.output.clone(),
            doc: None,
        });
    }

    // 检查是否有 #[tool(...)] 属性
    let mut custom_name = None;
    let mut custom_desc = None;
    let mut should_skip = false;

    for attr in &fn_item.attrs {
        if attr.path().is_ident("tool") {
            // 解析 #[tool(name = "...", desc = "...")] 或 #[tool(skip)]
            if let Ok(args) = attr.parse_args::<MethodToolAttrs>() {
                if args.skip {
                    should_skip = true;
                    break;
                }
                custom_name = args.name;
                custom_desc = args.desc;
            }
        }
    }
    
    if should_skip {
        return None;
    }

    let tool_name = custom_name.unwrap_or_else(|| method_name.clone());

    // 优先使用自定义描述，其次使用 doc comment
    let description = custom_desc.or_else(|| extract_doc_comment(&fn_item.attrs))
        .unwrap_or_else(|| format!("调用 {} 方法", method_name));

    let params = extract_params(&fn_item.sig.inputs);
    let is_async = fn_item.sig.asyncness.is_some();
    let is_result = is_result_type(&fn_item.sig.output);

    Some(ToolMethodInfo {
        name: method_name,
        tool_name,
        description,
        params,
        is_async,
        is_result,
        is_generic: false,
        return_type: fn_item.sig.output.clone(),
        doc: None,
    })
}

/// 工具方法信息
struct ToolMethodInfo {
    name: String,
    tool_name: String,
    description: String,
    params: Vec<ParamInfo>,
    is_async: bool,
    is_result: bool,
    is_generic: bool,
    #[allow(dead_code)]
    return_type: ReturnType,
    #[allow(dead_code)]
    doc: Option<String>,
}

/// 参数信息
struct ParamInfo {
    name: Ident,
    ty: Type,
    description: Option<String>,
    is_option: bool,
}

/// 生成编译期工具定义 const
fn generate_tool_def_consts(tools: &[ToolMethodInfo]) -> Vec<TokenStream2> {
    tools
        .iter()
        .map(|tool| {
            // 检查泛型方法
            if tool.is_generic {
                let name = &tool.name;
                return quote! {
                    compile_error!(concat!(
                        "工具方法 `",
                        #name,
                        "` 不支持泛型参数。请使用具体类型或 serde_json::Value 代替泛型参数"
                    ));
                };
            }

            let const_name = format_ident!("__TOOL_DEF_{}", tool.name.to_uppercase());
            let tool_name = &tool.tool_name;
            let description = &tool.description;

            // 生成 JSON schema
            let schema_json = generate_schema_json(&tool.params);

            quote! {
                const #const_name: ::tokitai::ToolDefinition = ::tokitai::ToolDefinition {
                    name: #tool_name,
                    description: #description,
                    input_schema: #schema_json,
                };
            }
        })
        .collect()
}

/// 生成所有工具定义的数组
fn generate_all_tool_defs_array(tools: &[ToolMethodInfo]) -> Vec<TokenStream2> {
    tools
        .iter()
        .map(|tool| {
            let const_name = format_ident!("__TOOL_DEF_{}", tool.name.to_uppercase());
            quote! { Self::#const_name }
        })
        .collect()
}

/// 生成 call_tool 分发方法（同步和异步双版本）
fn generate_call_tool_method(tools: &[ToolMethodInfo]) -> Vec<TokenStream2> {
    let mut methods = Vec::new();

    // 检查是否有异步方法
    let has_async = tools.iter().any(|t| t.is_async);

    // 生成异步版本（如果有异步方法，或者用户明确需要异步）
    if has_async {
        let match_arms = tools.iter().map(|tool| {
            let tool_name = &tool.tool_name;
            let method_name = Ident::new(&tool.name, Span::call_site());
            let wrapper_name = format_ident!("__call_{}", method_name);

            quote! {
                #tool_name => {
                    self.#wrapper_name(args).await
                }
            }
        });

        methods.push(quote! {
            /// 根据工具名称调用工具（异步版本）
            pub async fn call_tool(
                &self,
                name: &str,
                args: &serde_json::Value,
            ) -> Result<serde_json::Value, ::tokitai::ToolError> {
                match name {
                    #(#match_arms)*
                    _ => Err(::tokitai::ToolError::not_found("未知工具")),
                }
            }
        });
    } else {
        // 全是同步方法，生成同步版本
        let match_arms = tools.iter().map(|tool| {
            let tool_name = &tool.tool_name;
            let method_name = Ident::new(&tool.name, Span::call_site());
            let wrapper_name = format_ident!("__call_{}", method_name);

            quote! {
                #tool_name => {
                    self.#wrapper_name(args)
                }
            }
        });

        methods.push(quote! {
            /// 根据工具名称调用工具（同步版本）
            pub fn call_tool(
                &self,
                name: &str,
                args: &serde_json::Value,
            ) -> Result<serde_json::Value, ::tokitai::ToolError> {
                match name {
                    #(#match_arms)*
                    _ => Err(::tokitai::ToolError::not_found("未知工具")),
                }
            }
        });
    }

    // 如果包含异步方法，额外生成一个同步包装版本（用于不支持异步的场景）
    if has_async {
        let match_arms = tools.iter().map(|tool| {
            let tool_name = &tool.tool_name;
            let method_name = Ident::new(&tool.name, Span::call_site());
            let wrapper_name_sync = format_ident!("__call_{}_sync", method_name);

            quote! {
                #tool_name => {
                    self.#wrapper_name_sync(args)
                }
            }
        });

        methods.push(quote! {
            /// 根据工具名称调用工具（同步阻塞版本，不推荐用于异步方法）
            pub fn call_tool_sync(
                &self,
                name: &str,
                args: &serde_json::Value,
            ) -> Result<serde_json::Value, ::tokitai::ToolError> {
                match name {
                    #(#match_arms)*
                    _ => Err(::tokitai::ToolError::not_found("未知工具")),
                }
            }
        });
    }

    methods
}

/// 生成参数解析辅助方法
fn generate_helper_methods(tools: &[ToolMethodInfo]) -> Vec<TokenStream2> {
    let mut methods = Vec::new();
    
    // 检查是否有异步方法
    let has_async = tools.iter().any(|t| t.is_async);
    
    for tool in tools {
        if has_async {
            // 如果有异步方法，生成异步包装方法和同步包装方法
            methods.push(generate_wrapper_method(tool, true));
            if tool.is_async {
                methods.push(generate_wrapper_method(tool, false));
            } else {
                // 同步方法也可以有同步包装版本
                methods.push(generate_wrapper_method_sync(tool));
            }
        } else {
            // 全是同步方法，只生成同步包装方法
            methods.push(generate_wrapper_method_sync(tool));
        }
    }
    
    methods
}

/// 生成同步包装方法
fn generate_wrapper_method_sync(tool: &ToolMethodInfo) -> TokenStream2 {
    let method_name = Ident::new(&tool.name, Span::call_site());
    let wrapper_name = format_ident!("__call_{}", method_name);
    let params = &tool.params;

    // 生成参数解析代码
    let param_parsing = params.iter().map(|p| {
        let param_name = &p.name;
        let param_name_str = param_name.to_string();

        if p.is_option {
            quote! {
                let #param_name = args.get(#param_name_str)
                    .and_then(|v| v.as_null().map(|_| None))
                    .unwrap_or_else(|| {
                        args.get(#param_name_str).map(|v| serde_json::from_value(v.clone()).ok())
                    })
                    .flatten();
            }
        } else {
            quote! {
                let #param_name = args.get(#param_name_str)
                    .ok_or_else(|| ::tokitai::ToolError::validation_error(concat!("缺少必需参数：", #param_name_str)))?;
                let #param_name: _ = serde_json::from_value(#param_name.clone())
                    .map_err(|e| ::tokitai::ToolError::validation_error(concat!("参数类型错误：", #param_name_str)))?;
            }
        }
    });

    let param_names: Vec<&Ident> = params.iter().map(|p| &p.name).collect();

    // 处理返回值
    let result_handling = if tool.is_result {
        quote! {
            match result {
                Ok(v) => Ok(serde_json::to_value(v).unwrap()),
                Err(_e) => Err(::tokitai::ToolError::internal_error("方法执行失败")),
            }
        }
    } else {
        quote! {
            Ok(serde_json::to_value(result).unwrap())
        }
    };

    quote! {
        #[allow(clippy::all)]
        fn #wrapper_name(&self, args: &serde_json::Value) -> Result<serde_json::Value, ::tokitai::ToolError> {
            use serde_json::Value;

            #(#param_parsing)*

            let result = self.#method_name(#(#param_names),*);
            #result_handling
        }
    }
}

/// 生成单个方法的包装函数（异步版本）
fn generate_wrapper_method(tool: &ToolMethodInfo, is_async: bool) -> TokenStream2 {
    let method_name = Ident::new(&tool.name, Span::call_site());
    let wrapper_name = if is_async {
        format_ident!("__call_{}", method_name)
    } else {
        format_ident!("__call_{}_sync", method_name)
    };
    let params = &tool.params;

    // 生成参数解析代码
    let param_parsing = params.iter().map(|p| {
        let param_name = &p.name;
        let param_name_str = param_name.to_string();

        if p.is_option {
            quote! {
                let #param_name = args.get(#param_name_str)
                    .and_then(|v| v.as_null().map(|_| None))
                    .unwrap_or_else(|| {
                        args.get(#param_name_str).map(|v| serde_json::from_value(v.clone()).ok())
                    })
                    .flatten();
            }
        } else {
            quote! {
                let #param_name = args.get(#param_name_str)
                    .ok_or_else(|| ::tokitai::ToolError::validation_error(concat!("缺少必需参数：", #param_name_str)))?;
                let #param_name: _ = serde_json::from_value(#param_name.clone())
                    .map_err(|e| ::tokitai::ToolError::validation_error(concat!("参数类型错误：", #param_name_str)))?;
            }
        }
    });

    let param_names: Vec<&Ident> = params.iter().map(|p| &p.name).collect();

    // 处理返回值
    let result_handling = if tool.is_result {
        quote! {
            match result {
                Ok(v) => Ok(serde_json::to_value(v).unwrap()),
                Err(_e) => Err(::tokitai::ToolError::internal_error("方法执行失败")),
            }
        }
    } else {
        quote! {
            Ok(serde_json::to_value(result).unwrap())
        }
    };

    let fn_sig = if is_async {
        quote! { async fn #wrapper_name(&self, args: &serde_json::Value) -> Result<serde_json::Value, ::tokitai::ToolError> }
    } else {
        quote! { fn #wrapper_name(&self, args: &serde_json::Value) -> Result<serde_json::Value, ::tokitai::ToolError> }
    };

    let method_call = if tool.is_async && !is_async {
        // 同步调用异步方法，使用 block_on
        // 使用 try_current() 避免在没有运行时的线程上 panic
        quote! {
            match tokio::runtime::Handle::try_current() {
                Ok(handle) => handle.block_on(async { self.#method_name(#(#param_names),*).await }),
                Err(_) => return Err(::tokitai::ToolError::internal_error(
                    "无法在同步上下文中调用异步工具：当前线程没有 tokio 运行时。请确保在 tokio 运行时中调用，或将工具方法改为同步"
                )),
            }
        }
    } else {
        quote! { self.#method_name(#(#param_names),*) }
    };

    quote! {
        #[allow(clippy::all)]
        #fn_sig {
            use serde_json::Value;

            #(#param_parsing)*

            let result = #method_call;
            #result_handling
        }
    }
}

/// 生成 JSON Schema
fn generate_schema_json(params: &[ParamInfo]) -> String {
    let mut properties = Vec::new();
    let mut required = Vec::new();

    for p in params {
        let name = p.name.to_string();
        let param_type = get_json_type(&p.ty);
        let desc = p.description.as_deref().unwrap_or("");

        properties.push(format!(
            r#""{}":{{"type":"{}","description":"{}"}}"#,
            name, param_type, desc
        ));

        if !p.is_option {
            required.push(format!(r#""{}""#, name));
        }
    }

    let properties_str = properties.join(",");
    let required_str = required.join(",");

    if required_str.is_empty() {
        format!(r#"{{"type":"object","properties":{{{}}}}}"#, properties_str)
    } else {
        format!(
            r#"{{"type":"object","properties":{{{}}},"required":[{}]}}"#,
            properties_str, required_str
        )
    }
}

/// 获取 JSON 类型字符串
fn get_json_type(ty: &Type) -> &'static str {
    match ty {
        Type::Path(path) => {
            let ident = path
                .path
                .segments
                .first()
                .map(|s| s.ident.to_string())
                .unwrap_or_default();
            match ident.as_str() {
                "String" | "str" => "string",
                "i8" | "i16" | "i32" | "i64" | "i128" | "u8" | "u16" | "u32" | "u64" | "u128" | "usize" | "isize" => {
                    "integer"
                }
                "f32" | "f64" => "number",
                "bool" => "boolean",
                "Vec" => "array",
                "Option" => {
                    // 对于 Option<T>，递归获取 T 的类型
                    if let Some(last_segment) = path.path.segments.last() {
                        if let syn::PathArguments::AngleBracketed(args) = &last_segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner_ty)) = args.args.first() {
                                return get_json_type(inner_ty);
                            }
                        }
                    }
                    "object" // 默认类型
                }
                _ => "object",
            }
        }
        Type::Reference(reference) => {
            if let Type::Path(path) = &*reference.elem {
                if let Some(ident) = path.path.segments.first() {
                    if ident.ident == "str" {
                        return "string";
                    }
                }
            }
            "object"
        }
        _ => "object",
    }
}

/// 从函数签名提取参数
fn extract_params(inputs: &Punctuated<FnArg, token::Comma>) -> Vec<ParamInfo> {
    let mut params = Vec::new();

    for arg in inputs {
        if let FnArg::Typed(PatType { pat, ty, attrs, .. }) = arg {
            if let Pat::Ident(ident) = pat.as_ref() {
                if ident.ident == "self" || ident.ident == "_self" {
                    continue;
                }

                let description = extract_doc_comment(attrs);
                let is_option = is_option_type(ty);

                params.push(ParamInfo {
                    name: ident.ident.clone(),
                    ty: ty.as_ref().clone(),
                    description,
                    is_option,
                });
            }
        }
    }

    params
}

/// 检查类型是否为 Option
fn is_option_type(ty: &Type) -> bool {
    if let Type::Path(path) = ty {
        if let Some(segment) = path.path.segments.first() {
            return segment.ident == "Option";
        }
    }
    false
}

/// 检查返回类型是否为 Result
fn is_result_type(output: &ReturnType) -> bool {
    match output {
        ReturnType::Type(_, ty) => {
            if let Type::Path(path) = ty.as_ref() {
                return path
                    .path
                    .segments
                    .first()
                    .map(|s| s.ident == "Result")
                    .unwrap_or(false);
            }
            false
        }
        _ => false,
    }
}

/// 提取 doc comment
fn extract_doc_comment(attrs: &[syn::Attribute]) -> Option<String> {
    let mut doc_lines = Vec::new();

    for attr in attrs {
        if attr.path().is_ident("doc") {
            if let Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(expr_lit) = &nv.value {
                    if let Lit::Str(lit) = &expr_lit.lit {
                        let value = lit.value();
                        let line = value.trim().trim_start_matches(':').trim();
                        if !line.is_empty() {
                            doc_lines.push(line.to_string());
                        }
                    }
                }
            }
        }
    }

    if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines.join(" "))
    }
}
