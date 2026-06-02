//! Parses the `#[wrap(client = TYPE, methods = [name1, name2])]` arguments.

use proc_macro2::TokenStream as TokenStream2;
use syn::{
    parse::{Parse, ParseStream},
    spanned::Spanned,
    Ident, Token, Type,
};

use super::WrapArgs;

/// Parse the contents of the `#[wrap(...)]` attribute into a [`WrapArgs`].
pub(crate) fn parse_wrap_args(args: TokenStream2) -> syn::Result<WrapArgs> {
    syn::parse2::<RawWrapArgs>(args).map(Into::into)
}

/// Internal parse target: `client = TYPE, methods = [a, b, c]`.
struct RawWrapArgs {
    client_ty: Type,
    /// Field name on the wrapper struct. Defaults to `client`; can be
    /// overridden with `field = "my_client"` for wrappers that prefer
    /// a different field name.
    field: Option<Ident>,
    methods: Vec<Ident>,
}

impl Parse for RawWrapArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut client_ty = None;
        let mut field = None;
        let mut methods = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;
            input.parse::<Token![=]>()?;

            match key.to_string().as_str() {
                "client" => {
                    if client_ty.is_some() {
                        return Err(input.error("`client` specified more than once"));
                    }
                    client_ty = Some(input.parse::<Type>()?);
                }
                "field" => {
                    if field.is_some() {
                        return Err(input.error("`field` specified more than once"));
                    }
                    let value: syn::LitStr = input.parse()?;
                    let ident = syn::parse_str::<Ident>(&value.value()).map_err(|_| {
                        input.error(format!(
                            "invalid `field = {:?}`: expected a valid Rust identifier",
                            value.value()
                        ))
                    })?;
                    field = Some(ident);
                }
                "methods" => {
                    if methods.is_some() {
                        return Err(input.error("`methods` specified more than once"));
                    }
                    let content;
                    syn::bracketed!(content in input);
                    let mut names = Vec::new();
                    while !content.is_empty() {
                        let name: Ident = content.parse()?;
                        if !is_valid_method_name(&name) {
                            return Err(content.error(format!(
                                "invalid method name `{}` in `methods = [...]`",
                                name
                            )));
                        }
                        names.push(name);
                        if content.peek(Token![,]) {
                            content.parse::<Token![,]>()?;
                        }
                    }
                    methods = Some(names);
                }
                other => {
                    return Err(input.error(format!(
                        "unknown `#[wrap]` argument `{}`; expected `client`, `field`, or `methods`",
                        other
                    )));
                }
            }

            if input.peek(Token![,]) {
                input.parse::<Token![,]>()?;
            }
        }

        let client_ty = client_ty
            .ok_or_else(|| input.error("missing `client = TYPE` in `#[wrap(...)]`"))?;
        let methods = methods.ok_or_else(|| {
            input.error("missing `methods = [name1, name2, ...]` in `#[wrap(...)]`")
        })?;
        if methods.is_empty() {
            return Err(input.error(
                "at least one method must be listed in `methods = [...]`; \
                 use `#[tool]` instead if you want every public method to be exposed",
            ));
        }

        Ok(RawWrapArgs {
            client_ty,
            field,
            methods,
        })
    }
}

/// Reject names that obviously cannot be methods (e.g. starting with
/// a digit, or `r#`-style raw identifiers that the rest of the macro
/// pipeline wouldn't accept either).
fn is_valid_method_name(ident: &Ident) -> bool {
    let s = ident.to_string();
    !s.is_empty()
        && s.chars()
            .next()
            .map(|c| c.is_alphabetic() || c == '_')
            .unwrap_or(false)
}

impl From<RawWrapArgs> for WrapArgs {
    fn from(r: RawWrapArgs) -> Self {
        // The struct field defaults to the literal name `client`. Users
        // can override it with `field = "my_field"` for naming reasons.
        let client_field = r.field.unwrap_or_else(|| Ident::new("client", r.client_ty.span()));
        WrapArgs {
            client_ty: r.client_ty,
            client_field,
            methods: r.methods,
        }
    }
}
