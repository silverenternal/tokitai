//! 工具信息提取
//!
//! 包含 collect_tool_methods、extract_tool_info 等函数

use quote::ToTokens;
#[allow(unused_imports)] // used by callers below
use syn::spanned::Spanned;
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

/// T-013: collect `replaced_by` redirect entries from any method on
/// the impl that opted into a `replaced_by` (including methods that
/// were dropped from the active `__call_*` match arms by
/// `#[tool(skip)]` or `visible = false`). Each entry is the
/// `(from, to)` pair: the caller-named `from` string and the
/// successor's `to` string.
///
/// Skipped methods still need their redirect entries so the
/// dispatcher's `_ => replaced_by` arm can route calls to the
/// successor. Active methods' redirects are added too — they are
/// harmless because the active match arm wins first; the redirect
/// only fires when the source name is *not* in the match.
pub fn collect_replaced_by_redirects(impl_item: &ItemImpl) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    for item in &impl_item.items {
        if let ImplItem::Fn(fn_item) = item {
            if !matches!(fn_item.vis, Visibility::Public(_)) {
                continue;
            }
            for attr in &fn_item.attrs {
                if attr.path().is_ident("tool") {
                    if let Ok(args) = attr.parse_args::<MethodToolAttrs>() {
                        if let Some(replaced_by) = args.replaced_by {
                            if !replaced_by.is_empty() {
                                let from =
                                    args.name.unwrap_or_else(|| fn_item.sig.ident.to_string());
                                entries.push((from, replaced_by));
                            }
                        }
                        break;
                    }
                }
            }
        }
    }
    entries
}

/// T-018: find the `desc = "..."` literal inside a `#[tool(...)]`
/// attribute's token tree and return its span. We need the literal's
/// own span (not the attribute's overall span) so the description
/// quality lint can anchor its `compile_error!` at the exact text
/// the user wrote.
///
/// The search is intentionally shallow: we walk the attribute's
/// `tokens` stream, look for an `=` followed by a `LitStr` whose
/// preceding identifier was `desc` or `description`. Punctuation
/// and whitespace are skipped. Nested groups (e.g. `min_desc_score`
/// argument values) are walked recursively.
///
/// Returns `None` when no `desc = "..."` literal is present (the
/// lint then has no anchor to use; the literal value still flows
/// through to the codegen layer from the `MethodToolAttrs` parse).
fn find_desc_literal_span(attr: &syn::Attribute) -> Option<proc_macro2::Span> {
    // We do not re-parse; we walk the raw token stream. `attr.meta`
    // gives us the inside of the attribute (everything between
    // `#[tool(` and `)]`), already normalised to a `syn::Meta`.
    use proc_macro2::{Delimiter, Spacing, TokenTree};
    let stream: proc_macro2::TokenStream = match &attr.meta {
        syn::Meta::Path(_) => proc_macro2::TokenStream::new(),
        syn::Meta::List(list) => list.tokens.clone(),
        syn::Meta::NameValue(nv) => nv.value.to_token_stream(),
    };
    let mut prev_ident: Option<String> = None;
    // The `_ =` assignment for the underscore-prefixed `Spacing`
    // import is fine: we are checking the punctuation style in
    // case future parsers need to know it.
    let _ = Spacing::Alone;
    let mut iter = stream.into_iter().peekable();
    while let Some(tok) = iter.next() {
        match &tok {
            TokenTree::Ident(i) => prev_ident = Some(i.to_string()),
            TokenTree::Punct(p) if p.as_char() == '=' => {
                // The next token should be a LitStr. If so, and the
                // preceding ident was `desc` or `description`, record
                // the literal's span.
                if let Some(TokenTree::Literal(lit)) = iter.peek() {
                    let s = lit.to_string();
                    // Quick shape check: a string literal starts
                    // and ends with `"`. We don't need to unescape;
                    // we only need to know it's a string-shaped
                    // literal.
                    if s.starts_with('"') && s.ends_with('"') {
                        if let Some(prev) = &prev_ident {
                            if prev == "desc" || prev == "description" {
                                return Some(lit.span());
                            }
                        }
                    }
                }
                prev_ident = None;
            }
            TokenTree::Group(g) => {
                // Nested groups (e.g. `alias = [...]`, `baked_examples
                // = [...]`, `allow = [...]`) do not contain the
                // outer `desc = "..."` literal in the shape we want.
                // We still recurse in case the user nests `desc`
                // inside one (the parser does not forbid it, though
                // it is unusual).
                if let Some(found) = scan_group_for_desc_literal(g) {
                    return Some(found);
                }
                prev_ident = None;
            }
            _ => {
                prev_ident = None;
            }
        }
    }
    // Silence the unused-import warning for `Delimiter`; we keep
    // the import in case future maintenance reaches into nested
    // group structure directly.
    let _ = Delimiter::Parenthesis;
    None
}

fn scan_group_for_desc_literal(g: &proc_macro2::Group) -> Option<proc_macro2::Span> {
    use proc_macro2::{Literal, TokenTree};
    let mut prev_ident: Option<String> = None;
    let mut iter = g.stream().into_iter().peekable();
    while let Some(tok) = iter.next() {
        match &tok {
            TokenTree::Ident(i) => prev_ident = Some(i.to_string()),
            TokenTree::Punct(p) if p.as_char() == '=' => {
                if let Some(TokenTree::Literal(lit)) = iter.peek() {
                    let s = lit.to_string();
                    if s.starts_with('"') && s.ends_with('"') {
                        if let Some(prev) = &prev_ident {
                            if prev == "desc" || prev == "description" {
                                return Some(lit.span());
                            }
                        }
                    }
                }
                prev_ident = None;
            }
            TokenTree::Group(nested) => {
                if let Some(found) = scan_group_for_desc_literal(nested) {
                    return Some(found);
                }
                prev_ident = None;
            }
            _ => {
                prev_ident = None;
            }
        }
    }
    // Drop the peekable iter explicitly so the unused-warning
    // doesn't fire if the future MSRV tightens dropck.
    drop(iter);
    let _ = Literal::string("");
    None
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
            baked_examples: Vec::new(),
            desc_span: None,
            min_desc_score: None,
            allow_short_desc: false,
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
    let mut baked_examples = Vec::new();
    let mut desc_span: Option<proc_macro2::Span> = None;
    let mut min_desc_score: Option<u8> = None;
    let mut allow_short_desc = false;

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
                baked_examples = args.baked_examples;
                min_desc_score = args.min_desc_score;
                allow_short_desc = args.allow_short_desc;
                // T-018: capture the desc literal's span from the
                // raw token tree. We do this AFTER the structured
                // parse so we only run the scan when there is
                // actually a `desc` value to anchor to.
                if custom_desc.is_some() {
                    desc_span = find_desc_literal_span(attr);
                }
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
        baked_examples,
        desc_span,
        min_desc_score,
        allow_short_desc,
    })
}
