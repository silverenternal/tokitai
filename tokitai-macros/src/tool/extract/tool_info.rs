//! 工具信息提取
//!
//! 包含 collect_tool_methods、extract_tool_info 等函数

use syn::{ImplItem, ImplItemFn, ItemImpl, Visibility};

use super::docs::extract_doc_comment;
use super::params::{extract_params, is_result_type};
use crate::tool::attrs::method::MethodToolAttrs;
use crate::tool::types::param::ParamToolAttrs;
use crate::tool::types::tool_method::ToolMethodInfo;

/// 收集所有被标记为工具的方法
#[inline]
pub fn collect_tool_methods(impl_item: &ItemImpl) -> Vec<ToolMethodInfo> {
    let mut tools = Vec::new();

    for item in &impl_item.items {
        if let ImplItem::Fn(fn_item) = item {
            if !matches!(fn_item.vis, Visibility::Public(_)) {
                continue;
            }

            if let Some(tool_info) = extract_tool_info(fn_item) {
                tools.push(tool_info);
            }
        }
    }

    tools
}

/// 提取工具方法信息
pub fn extract_tool_info(fn_item: &ImplItemFn) -> Option<ToolMethodInfo> {
    let method_name = fn_item.sig.ident.to_string();

    if method_name.starts_with("__") {
        return None;
    }

    if !fn_item.sig.generics.params.is_empty() {
        return Some(ToolMethodInfo {
            ident_span: fn_item.sig.ident.span(),
            name: method_name.clone(),
            tool_name: method_name.clone(),
            description: String::new(),
            params: vec![],
            is_async: false,
            is_result: false,
            is_generic: true,
            deprecated: false,
            replaced_by: None,
            deprecated_note: None,
            deprecated_since: None,
            remove_in: None,
            version: None,
            visible: true,
            tags: Vec::new(),
            group: None,
            return_description: None,
            context: None,
            example_input: None,
            param_order: None,
            hidden_params: Vec::new(),
            example_output: None,
            return_type: fn_item.sig.output.clone(),
            doc: None,
            alias: Vec::new(),
            allow: Vec::new(),
            cache: None,
            rate_limit: None,
            param_validations: Vec::new(),
            description_explicit: false,
        });
    }

    let mut custom_name = None;
    let mut custom_desc = None;
    let mut should_skip = false;
    let mut is_deprecated = false;
    let mut replaced_by = None;
    let mut deprecated_note = None;
    let mut deprecated_since = None;
    let mut remove_in = None;
    let mut version = None;
    let mut is_visible = true;
    let mut tool_tags = Vec::new();
    let mut group = None;
    let mut return_description = None;
    let mut context = None;
    let mut example_input: Option<serde_json::Value> = None;
    let mut param_order: Option<Vec<String>> = None;
    let mut hidden_params = Vec::new();
    let mut example_output = None;
    let mut alias = Vec::new();
    let mut allow = Vec::new();
    let mut cache: Option<String> = None;
    let mut rate_limit: Option<String> = None;
    let mut param_validations: Vec<(String, ParamToolAttrs)> = Vec::new();

    for attr in &fn_item.attrs {
        if attr.path().is_ident("tool") {
            if let Ok(args) = attr.parse_args::<MethodToolAttrs>() {
                if args.skip {
                    should_skip = true;
                    break;
                }
                custom_name = args.name;
                custom_desc = args.desc;
                is_deprecated = args.deprecated;
                replaced_by = args.replaced_by;
                deprecated_note = args.deprecated_note;
                deprecated_since = args.deprecated_since;
                remove_in = args.remove_in;
                version = args.version;
                is_visible = args.visible;
                tool_tags = args.tags;
                group = args.group;
                return_description = args.return_description;
                context = args.context;
                example_input = args.example_input;
                param_order = args.param_order;
                hidden_params = args.hidden_params;
                example_output = args.example_output;
                alias = args.alias;
                allow = args.allow;
                cache = args.cache;
                rate_limit = args.rate_limit;
                param_validations = args.param_validations;
            }
        }
    }

    if should_skip {
        return None;
    }

    if !is_visible {
        return None;
    }

    let tool_name = custom_name.unwrap_or_else(|| method_name.clone());

    // T-002: capture whether the description came from an explicit
    // `#[tool(desc = "...")]` attribute so the codegen can mark the
    // resulting `ToolDefinition` and prevent the runtime `tokitai!`
    // config from overriding it. Doc-comment and synthesized default
    // descriptions stay open to runtime override.
    let description_explicit = custom_desc.is_some();

    let description = custom_desc
        .or_else(|| extract_doc_comment(&fn_item.attrs))
        .unwrap_or_else(|| format!("调用 {} 方法", method_name));

    let params = extract_params(
        &fn_item.sig.inputs,
        &fn_item.attrs,
        &hidden_params,
        &param_validations,
    );
    let is_async = fn_item.sig.asyncness.is_some();
    let is_result = is_result_type(&fn_item.sig.output);

    Some(ToolMethodInfo {
        ident_span: fn_item.sig.ident.span(),
        name: method_name,
        tool_name,
        description,
        params,
        is_async,
        is_result,
        is_generic: false,
        deprecated: is_deprecated,
        replaced_by,
        deprecated_note,
        deprecated_since,
        remove_in,
        version,
        visible: is_visible,
        tags: tool_tags,
        group,
        return_description,
        context,
        example_input,
        param_order,
        hidden_params,
        example_output,
        return_type: fn_item.sig.output.clone(),
        doc: None,
        alias,
        allow,
        cache,
        rate_limit,
        param_validations,
        description_explicit,
    })
}
