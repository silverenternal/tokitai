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
}

impl Parse for MethodToolAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
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

        Ok(MethodToolAttrs { name, desc })
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

    // 生成 call_tool 分发方法
    let call_tool_method = generate_call_tool_method(&tool_methods);

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

    // 添加 call_tool 方法
    new_items.push(parse_quote! { #call_tool_method });

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

    // 检查是否有 #[tool(...)] 属性
    let mut custom_name = None;
    let mut custom_desc = None;

    for attr in &fn_item.attrs {
        if attr.path().is_ident("tool") {
            if let Ok(args) = attr.parse_args::<MethodToolAttrs>() {
                custom_name = args.name;
                custom_desc = args.desc;
            }
        }
    }

    // 如果没有 #[tool] 属性，但方法是 pub 的，也自动包含（使用默认设置）
    // 这里我们选择：所有 pub 方法都自动成为工具，除非明确标记为 #[tool(skip)]

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
        return_type: fn_item.sig.output.clone(),
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
    #[allow(dead_code)]
    return_type: ReturnType,
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

/// 生成 call_tool 分发方法
fn generate_call_tool_method(tools: &[ToolMethodInfo]) -> TokenStream2 {
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

    quote! {
        /// 根据工具名称调用工具
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
    }
}

/// 生成参数解析辅助方法
fn generate_helper_methods(tools: &[ToolMethodInfo]) -> Vec<TokenStream2> {
    tools
        .iter()
        .map(|tool| generate_wrapper_method(tool))
        .collect()
}

/// 生成单个方法的包装函数
fn generate_wrapper_method(tool: &ToolMethodInfo) -> TokenStream2 {
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

    let await_keyword = if tool.is_async { Some(quote! { .await }) } else { None };

    // 处理返回值
    let result_handling = if tool.is_result {
        quote! {
            match result {
                Ok(v) => Ok(serde_json::to_value(v).unwrap()),
                Err(e) => Err(::tokitai::ToolError::internal_error("方法执行失败")),
            }
        }
    } else {
        quote! {
            Ok(serde_json::to_value(result).unwrap())
        }
    };

    quote! {
        async fn #wrapper_name(
            &self,
            args: &serde_json::Value,
        ) -> Result<serde_json::Value, ::tokitai::ToolError> {
            use serde_json::Value;

            #(#param_parsing)*

            let result = self.#method_name(#(#param_names),*) #await_keyword;
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
                "Option" => "string",
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
