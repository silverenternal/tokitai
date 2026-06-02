//! Argument and signature extraction for `#[delegate(...)]`.
//!
//! This module is responsible for:
//!
//! 1. Parsing the `to = "expr"` literal argument of the `#[delegate(...)]`
//!    attribute. The string is itself parsed as a Rust `syn::Expr` so that we
//!    can splat it back into the generated method body unchanged.
//!
//! 2. Parsing the method signature (which the user wrote without a body) and
//!    building a `ToolMethodInfo` from it. The `ToolMethodInfo` is then fed
//!    to the existing `definitions::generate_tool_def_consts` and
//!    `wrappers::generate_helper_methods` helpers so that the generated
//!    `__TOOL_DEF_*` and `__call_*` items are byte-for-byte identical to
//!    those produced by `#[tool]`.

use syn::{parse::ParseStream, Expr, LitStr, Signature};

use crate::tool::extract::docs::extract_doc_comment;
use crate::tool::extract::params::{extract_params, is_result_type};
use crate::tool::types::tool_method::ToolMethodInfo;

/// The pieces of a method signature that `#[delegate]` needs. The user
/// writes something like:
///
/// ```text
/// #[doc = "..."]
/// pub async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, OpenAIError>;
/// ```
///
/// We split that into the leading `#[..]` attributes (which we splice
/// into the generated `ImplItemFn` so things like `///` doc comments
/// survive the round trip) and the `Signature` itself.
#[derive(Clone)]
pub struct MethodSig {
    pub attrs: Vec<syn::Attribute>,
    pub sig: Signature,
}

/// Parsed `to = "..."` argument of the `#[delegate(...)]` attribute.
pub struct DelegateArgs {
    /// The raw string the user wrote inside the `to = "..."` literal.
    pub to_expr_str: String,
}

impl syn::parse::Parse for DelegateArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let key: syn::Ident = input.parse()?;
        if key != "to" {
            return Err(syn::Error::new_spanned(
                key,
                "expected `to` in #[delegate(to = \"...\")]",
            ));
        }
        input.parse::<syn::token::Eq>()?;
        let value: LitStr = input.parse()?;
        let raw = value.value();
        if raw.trim().is_empty() {
            return Err(syn::Error::new_spanned(
                value,
                "the `to` expression cannot be empty. \
                 Write `#[delegate(to = \"self.inner\")]` (or any other valid Rust expression).",
            ));
        }
        Ok(DelegateArgs {
            to_expr_str: raw,
        })
    }
}

/// Parse the user-supplied `to` string as a Rust expression. We do this so
/// we can splat it back into the generated method body verbatim.
///
/// This is the second line of defence for item 17 (`to` is not a
/// valid expression). The first line is the `Parse` impl above,
/// which catches structurally-malformed attribute args. This
/// function catches the *content* being unparseable as a Rust
/// expression — for example, `to = "not valid rust"`.
pub fn parse_to_expr(to_str: &str) -> syn::Result<Expr> {
    syn::parse_str::<Expr>(to_str).map_err(|e| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "#[delegate]: failed to parse `to = \"{}\"` as a valid Rust expression. \
                 Examples of valid values: `\"self.inner\"`, `\"self.pool\"`, `\"self.db.get()\"`. \
                 Underlying parse error: {}",
                to_str, e
            ),
        )
    })
}

/// Build a `ToolMethodInfo` from a method signature. We deliberately do not
/// try to support every knob that `#[tool]` supports (no `#[tool(name=...)]`
/// forwarding, no per-param overrides, etc.). Delegate is meant to be the
/// "I just want to forward this thing" path; users that need fine-grained
/// schema control should write a real method body.
///
/// The input is a `MethodSig` (a leading-attribute list + a
/// `Signature`). The signature is taken straight from the user; the
/// leading attributes are the user's `#[doc = "..."]` lines and
/// friends.
pub fn build_tool_method_info(method: &MethodSig) -> syn::Result<ToolMethodInfo> {
    let method_name = method.sig.ident.to_string();

    if !method.sig.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &method.sig.ident,
            "#[delegate] does not support generic methods (use a concrete type)",
        ));
    }

    let is_async = method.sig.asyncness.is_some();
    let is_result = is_result_type(&method.sig.output);
    let description = extract_doc_comment(&method.attrs)
        .unwrap_or_else(|| format!("调用 {} 方法", method_name));

    // Build a `ParamInfo` for each non-`self` parameter. We do not support
    // the rich per-param `#[tool(...)]` overrides in the delegate path; the
    // signature itself is the source of truth.
    let params = extract_params(&method.sig.inputs, &method.attrs, &[], &[]);

    Ok(ToolMethodInfo {
        name: method_name.clone(),
        tool_name: method_name,
        description,
        params,
        is_async,
        is_result,
        is_generic: false,
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
        return_type: method.sig.output.clone(),
        doc: None,
        alias: Vec::new(),
        allow: Vec::new(),
        cache: None,
        rate_limit: None,
        param_validations: Vec::new(),
    })
}
