//! Splices generated items into the user's `impl` block and filters
//! the listed methods down to a `ToolMethodInfo` slice.

use proc_macro2::TokenStream as TokenStream2;
use syn::{ImplItem, ItemImpl};

use crate::error::MacroError;
use crate::tool::extract::collect_tool_methods;

/// Filter the impl's public methods to those listed in `methods`,
/// then run the same `collect_tool_methods` pipeline `#[tool]` uses.
///
/// # Errors
///
/// Returns a `MacroError` (E0001) if a method listed in
/// `methods = [...]` is not present (or not public) in the impl
/// block. Without this check the macro would silently drop
/// user-specified methods, which is a worse failure mode than a
/// clear compile error. The error includes a "did you mean"
/// suggestion computed against the public methods that *are*
/// present, so the user can fix the typo without re-reading the
/// entire impl block.
pub(crate) fn filter_tool_methods(
    impl_item: &ItemImpl,
    methods: &[syn::Ident],
) -> Result<Vec<crate::tool::types::tool_method::ToolMethodInfo>, MacroError> {
    // 0. Collect every public method on the impl block so the
    //    "method not found" error can suggest the closest match.
    let all_public_methods: Vec<String> = impl_item
        .items
        .iter()
        .filter_map(|item| {
            if let ImplItem::Fn(fn_item) = item {
                if let syn::Visibility::Public(_) = fn_item.vis {
                    return Some(fn_item.sig.ident.to_string());
                }
            }
            None
        })
        .collect();

    // 1. Build a temporary impl containing only the listed methods so
    //    the existing extractor sees exactly the same shape as a
    //    hand-written `#[tool]` block.
    let mut tmp = impl_item.clone();
    tmp.items.retain(|item| {
        if let ImplItem::Fn(fn_item) = item {
            if let syn::Visibility::Public(_) = fn_item.vis {
                let name = fn_item.sig.ident.to_string();
                return methods.iter().any(|m| m == &name);
            }
        }
        false
    });

    // 2. Validate that every listed method has a matching item.
    let found: Vec<String> = tmp
        .items
        .iter()
        .filter_map(|item| {
            if let ImplItem::Fn(fn_item) = item {
                Some(fn_item.sig.ident.to_string())
            } else {
                None
            }
        })
        .collect();

    for requested in methods {
        let requested_str = requested.to_string();
        if !found.iter().any(|f| f == &requested_str) {
            // Re-use the centralised "method_not_found" builder
            // so the diagnostic format is identical to the one
            // the `#[tool]` pipeline emits.
            return Err(MacroError::method_not_found(
                requested.span(),
                &requested_str,
                &all_public_methods,
            ));
        }
    }

    // 3. Run the standard extractor on the filtered impl.
    Ok(collect_tool_methods(&tmp))
}

/// Append the macro-generated items to the impl block, parsing them
/// back as `ImplItem`s so the resulting `ItemImpl` is well-formed.
///
/// We collect any parsing errors into a single `syn::Error` to keep
/// the user-facing diagnostic compact.
pub(crate) fn append_items(
    impl_item: &mut ItemImpl,
    generated: &[TokenStream2],
) -> syn::Result<()> {
    for tokens in generated {
        let item: ImplItem = syn::parse2(tokens.clone()).map_err(|e| {
            syn::Error::new_spanned(
                tokens,
                format!("internal `#[wrap]` error: generated item failed to parse: {}", e),
            )
        })?;
        impl_item.items.push(item);
    }
    Ok(())
}
