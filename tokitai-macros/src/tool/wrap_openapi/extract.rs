//! Attribute extraction for `#[openapi(...)]` (impl-block level) and
//! `#[openapi_op(...)]` (per-method level).
//!
//! The impl-block attribute carries the spec path, an optional base
//! URL, and an optional target-type override. The per-method attribute
//! is just a `operation_id = "..."` that ties the method to a
//! specific OpenAPI operation.

use syn::{
    parse::{Parse, ParseStream},
    Attribute, Ident, LitStr, Result, Token,
};

/// Attributes attached to the impl block: `#[openapi(spec = "...",
/// base_url = "...", target = SomeType)]`.
#[derive(Debug, Default)]
pub(crate) struct OpenApiArgs {
    /// Path to the OpenAPI JSON file (will be `include_str!`-ed at the
    /// consumer's compile time).
    pub spec: Option<String>,
    /// Optional base URL prefix; stored verbatim on the impl so
    /// generated code can reference it if it wants to.
    pub base_url: Option<String>,
    /// Optional target type override.
    pub target: Option<Ident>,
}

impl Parse for OpenApiArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut args = OpenApiArgs::default();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "spec" => {
                    let v: LitStr = input.parse()?;
                    args.spec = Some(v.value());
                }
                "base_url" => {
                    let v: LitStr = input.parse()?;
                    args.base_url = Some(v.value());
                }
                "target" => {
                    args.target = Some(input.parse()?);
                }
                other => {
                    // Swallow the value to keep the parse moving; warn via
                    // syn's parser-error mechanism.
                    return Err(syn::Error::new_spanned(
                        key,
                        format!(
                            "unknown `#[openapi]` argument `{}` (expected: spec, base_url, target)",
                            other
                        ),
                    ));
                }
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(args)
    }
}

/// Attributes attached to a single method: `#[openapi_op(operation_id =
/// "...")]`.
#[derive(Debug, Default)]
pub(crate) struct OpenApiOpArgs {
    /// The `operationId` this method should be bound to.
    pub operation_id: Option<String>,
}

impl Parse for OpenApiOpArgs {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut args = OpenApiOpArgs::default();

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "operation_id" => {
                    let v: LitStr = input.parse()?;
                    args.operation_id = Some(v.value());
                }
                other => {
                    return Err(syn::Error::new_spanned(
                        key,
                        format!(
                            "unknown `#[openapi_op]` argument `{}` (expected: operation_id)",
                            other
                        ),
                    ));
                }
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(args)
    }
}

/// Walk the attributes on a `syn::ImplItemFn` and return the parsed
/// `#[openapi_op(...)]` arguments, if any. The attribute is consumed
/// (i.e. left in place on the method) so downstream codegen still
/// sees it.
pub(crate) fn extract_op_attr(attrs: &[Attribute]) -> Option<OpenApiOpArgs> {
    for attr in attrs {
        if attr.path().is_ident("openapi_op") {
            if let Ok(parsed) = attr.parse_args::<OpenApiOpArgs>() {
                return Some(parsed);
            }
        }
    }
    None
}
