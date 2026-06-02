//! Code generation for the `#[openapi(...)]` proc-macro.
//!
//! For each method annotated with `#[openapi_op(operation_id = "...")]`
//! we:
//!
//! 1. Build a [`ToolMethodInfo`] so the existing `#[tool]` codegen
//!    pipeline (which we share, per the design constraint that
//!    `tokitai-macros/src/tool/codegen/*` is read-only for us) can do
//!    the heavy lifting of generating the per-tool wrapper, the
//!    `__TOOL_DEF_*` constant, the `call_tool` dispatcher arm, and the
//!    `ToolProvider` impl.
//! 2. Override the description and tool_name fields with the values
//!    we pulled out of the spec — that's the only piece of metadata
//!    the user can't supply on the Rust side.
//! 3. Emit the spec static (`OPENAPI_OPS` + `__OpenApiOp_*` struct)
//!    alongside the impl block.
//!
//! The generated impl block also includes a `configure_tool` stub for
//! parity with the `#[tool]`-generated code, but it is a no-op
//! because OpenAPI-derived metadata is fixed at compile time.

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{ImplItem, ItemImpl, ReturnType, Type};

use crate::tool::codegen::{definitions, dispatcher, wrappers};
use crate::tool::extract::params::{extract_params, is_result_type};
use crate::tool::types::param::ParamToolAttrs;
use crate::tool::types::tool_method::ToolMethodInfo;

use super::parser::OpenApiSpec;
use super::spec_static::generate_spec_static;

/// Top-level entry point: take the parsed `#[openapi]` arguments,
/// the parsed spec, the original `ItemImpl`, and the spec path, and
/// return the full TokenStream to splice back into the source.
pub fn expand_impl(
    impl_item: &ItemImpl,
    spec: &OpenApiSpec,
    spec_path: &str,
) -> TokenStream2 {
    // 1. Find every method that carries `#[openapi_op(...)]` and
    //    construct a `ToolMethodInfo` for it.
    // Upper bound: at most one `#[openapi_op]` per fn item, so the
    // total number of `tool_methods` is bounded by `impl_item.items.len()`.
    let mut tool_methods: Vec<ToolMethodInfo> = Vec::with_capacity(impl_item.items.len());

    for item in &impl_item.items {
        if let ImplItem::Fn(fn_item) = item {
            if let Some(op_args) = super::extract::extract_op_attr(&fn_item.attrs) {
                let Some(op_id) = op_args.operation_id else {
                    let err = syn::Error::new_spanned(
                        &fn_item.sig.ident,
                        "`#[openapi_op]` is missing `operation_id = \"...\"`",
                    )
                    .to_compile_error();
                    return err;
                };

                let Some((path, method, op)) = spec.lookup(&op_id) else {
                    let err = syn::Error::new_spanned(
                        &fn_item.sig.ident,
                        format!(
                            "operation_id `{}` not found in OpenAPI spec `{}`",
                            op_id, spec_path
                        ),
                    )
                    .to_compile_error();
                    return err;
                };

                let info = build_tool_method_info(fn_item, &op_id, op, path, method);
                tool_methods.push(info);
            }
        }
    }

    // 2. Generate the standard `#[tool]`-style boilerplate via the
    //    existing codegen helpers. These functions live in
    //    `tokitai-macros/src/tool/codegen/*` which is out of our edit
    //    scope, but the public surface (the free functions we call
    //    here) is stable.
    let tool_def_consts = definitions::generate_tool_def_consts(&tool_methods);
    let all_tool_defs = definitions::generate_all_tool_defs_array(&tool_methods, &impl_item.self_ty);
    let call_tool_methods = dispatcher::generate_call_tool_method(&tool_methods);
    let helper_methods = wrappers::generate_helper_methods(&tool_methods);
    let tool_count_const = definitions::generate_tool_count_const(&tool_methods);

    // 3. Splice every generated item into the impl block.
    let mut new_items: Vec<ImplItem> = impl_item.items.clone();
    for item_ts in tool_def_consts {
        if let Ok(it) = syn::parse2::<ImplItem>(item_ts) {
            new_items.push(it);
        }
    }
    if let Ok(it) = syn::parse2::<ImplItem>(tool_count_const) {
        new_items.push(it);
    }

    let all_tool_defs_tokens = &all_tool_defs;
    let impl_type = &impl_item.self_ty;

    let get_tool_definitions_fn: ImplItem = syn::parse_quote! {
        /// All tool definitions (compile-time generated, no runtime config).
        fn __get_tool_definitions() -> &'static [::tokitai_core::ToolDefinition] {
            static TOOLS: ::std::sync::LazyLock<::std::vec::Vec<::tokitai_core::ToolDefinition>> = ::std::sync::LazyLock::new(|| {
                ::std::vec::Vec::from([#(#all_tool_defs_tokens.clone()),*])
            });
            &TOOLS
        }
    };
    new_items.push(get_tool_definitions_fn);

    for method in call_tool_methods {
        if let Ok(it) = syn::parse2::<ImplItem>(method) {
            new_items.push(it);
        }
    }
    for helper in helper_methods {
        if let Ok(it) = syn::parse2::<ImplItem>(helper) {
            new_items.push(it);
        }
    }

    // No-op `configure_tool` for parity with `#[tool]`-generated
    // blocks; OpenAPI metadata is fixed at compile time, so runtime
    // configuration is intentionally a no-op.
    new_items.push(syn::parse_quote! {
        /// Compile-time-only stub. OpenAPI-derived metadata cannot be
        /// overridden at runtime; this method exists for trait-shape
        /// parity with `#[tool]`.
        pub fn configure_tool(_tool_name: &str, _configs: &[::tokitai_core::ToolConfig]) {}
    });

    let mut new_impl = impl_item.clone();
    new_impl.items = new_items;

    // 4. Outside the impl block: a `ToolProvider` impl, a
    //    `ToolCaller` impl, and the spec static. The static goes in
    //    its own consts module so the names don't leak.
    let tool_caller_impl = quote! {
        impl ::tokitai_core::ToolCaller for #impl_type {
            fn call_tool(&self, name: &str, args: &::tokitai_core::serde_types::Value) -> Result<::tokitai_core::serde_types::Value, ::tokitai_core::ToolError> {
                self.call_tool(name, args)
            }
        }
    };

    let tool_provider_impl = quote! {
        impl ::tokitai_core::ToolProvider for #impl_type {
            fn tool_definitions() -> &'static [::tokitai_core::ToolDefinition] {
                Self::__get_tool_definitions()
            }

            fn tool_count() -> usize {
                Self::__TOOL_COUNT
            }
        }
    };

    // 5. Spec static. Names are scoped per-impl to avoid clashes.
    let impl_ident = match impl_type_as_ident(impl_type) {
        Some(id) => id,
        None => format_ident!("OpenApiClient"),
    };
    let spec_static = generate_spec_static(&impl_ident, spec, spec_path);

    quote! {
        #new_impl

        #tool_provider_impl
        #tool_caller_impl

        #spec_static
    }
}

/// Build a `ToolMethodInfo` for a method. We do not run
/// `extract_tool_info` from the regular `#[tool]` pipeline because
/// our method lacks the `#[tool]` attribute; instead we mirror the
/// fields that the codegen functions actually consume.
fn build_tool_method_info(
    fn_item: &syn::ImplItemFn,
    op_id: &str,
    op: &super::parser::Operation,
    path: &str,
    method: &str,
) -> ToolMethodInfo {
    let method_name = fn_item.sig.ident.to_string();

    // Parameter extraction reuses the same helper the `#[tool]`
    // pipeline uses, so the JSON-schema generation, default
    // handling, and option detection all behave identically.
    let params = extract_params(&fn_item.sig.inputs, &fn_item.attrs, &[], &[]);
    let _ = path;
    let _ = method;

    ToolMethodInfo {
        name: method_name.clone(),
        tool_name: op_id.to_string(),
        description: op.description_or_summary(),
        params,
        is_async: fn_item.sig.asyncness.is_some(),
        is_result: is_result_type(&fn_item.sig.output),
        is_generic: !fn_item.sig.generics.params.is_empty(),
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
        return_type: normalise_return_type(&fn_item.sig.output),
        doc: Some(op.description_or_summary()),
        alias: Vec::new(),
        allow: Vec::new(),
        cache: None,
        rate_limit: None,
        param_validations: Vec::<(String, ParamToolAttrs)>::new(),
    }
}

/// If the method returns a `Result<T, _>` we keep the `Result` shape
/// in the generated wrapper, so we just pass through the original
/// return type.
fn normalise_return_type(rt: &ReturnType) -> ReturnType {
    rt.clone()
}

/// Best-effort: if the impl's `self_ty` is a plain `Ident`, return
/// it; otherwise return `None` and let the caller fall back to a
/// generic name. The static name needs to be a valid Rust
/// identifier, and only `Ident`-shaped types satisfy that without a
/// quote-roundtrip.
fn impl_type_as_ident(ty: &Type) -> Option<syn::Ident> {
    if let Type::Path(type_path) = ty {
        if let Some(last) = type_path.path.segments.last() {
            return Some(last.ident.clone());
        }
    }
    None
}
