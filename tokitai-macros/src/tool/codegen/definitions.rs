//! 工具定义生成
//!
//! 包含 generate_tool_def_consts、generate_all_tool_defs_array 等函数

use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote, quote_spanned};
use syn::Type;

use crate::error::MacroError;
use crate::tool::example::bake_examples_to_schema_json;
use crate::tool::schema::dialect::{audit, Dialect};
use crate::tool::schema::gen::SchemaGenConfig;
use crate::tool::types::tool_method::ToolMethodInfo;

/// 生成编译期工具定义函数
///
/// `dialect` (T-012) is the active schema-dialect rule set. When
/// set to anything other than `Mcp`, every rendered
/// `ToolDefinition.input_schema` is audited against the
/// dialect's rule set; violations are emitted as `compile_error!`
/// invocations anchored at the user-written method span.
pub fn generate_tool_def_consts(tools: &[ToolMethodInfo], dialect: Dialect) -> Vec<TokenStream2> {
    // Each tool emits one primary def + one def per alias. Pre-allocate
    // an exact-size buffer so the outer `Vec` never reallocates while
    // we are emitting tokens.
    let total: usize = tools.len() + tools.iter().map(|t| t.alias.len()).sum::<usize>();
    let mut consts = Vec::with_capacity(total);

    for tool in tools {
        if tool.is_generic {
            let name = &tool.name;
            // T-001: anchor the `compile_error!` at the user's
            // method name, not the macro call site. Without this
            // the editor highlights `#[tool]` (line N) instead of
            // the offending `fn generic_method<T>(...)` (line M),
            // and the user has to read the macro output to figure
            // out which method tripped the rule.
            let span = tool.ident_span;
            consts.push(quote_spanned! {span=>
                compile_error!(concat!(
                    "[tokitai] Tool method `",
                    #name,
                    "` uses generic parameters, which are not supported.\n",
                    "Solutions:\n",
                    "   1. Use a concrete type: fn ",
                    #name,
                    "(data: MyType) -> String\n",
                    "   2. Use serde_json::Value: fn ",
                    #name,
                    "(data: Value) -> String\n",
                    "   3. Manually deserialize inside the method"
                ));
            });
            continue;
        }

        let const_name = format_ident!("__TOOL_DEF_{}", tool.name.to_uppercase());
        let tool_name = &tool.tool_name;
        let description = &tool.description;

        let config = SchemaGenConfig::new(&tool.params)
            .deprecated(tool.deprecated)
            .replaced_by(tool.replaced_by.as_deref())
            .context(tool.context.as_deref())
            .tags(&tool.tags)
            .return_description(tool.return_description.as_deref())
            .example_input(tool.example_input.as_ref())
            .param_order(tool.param_order.as_deref())
            .example_output(tool.example_output.as_deref())
            .deprecated_note(tool.deprecated_note.as_deref())
            .deprecated_since(tool.deprecated_since.as_deref())
            .remove_in(tool.remove_in.as_deref())
            .group(tool.group.as_deref())
            .cache(tool.cache.as_deref())
            .rate_limit(tool.rate_limit.as_deref())
            .baked_examples(if tool.baked_examples.is_empty() {
                None
            } else {
                Some(&tool.baked_examples)
            });

        let (schema_ast, schema_json) =
            crate::tool::schema::gen::generate_schema_ast_and_json_with_deprecated_and_tags(
                &config,
            );

        // T-012: audit the rendered schema against the chosen
        // dialect. Violations become `compile_error!` calls
        // anchored at the user-written method name, so editors
        // jump straight to the offending code.
        let span = tool.ident_span;
        for v in audit(dialect, &schema_ast) {
            let err: MacroError = v.into_macro_error(span);
            consts.push(err.to_compile_error());
        }

        let version_tokens = tool
            .version
            .as_ref()
            .map(|v| {
                quote! { .with_version(#v) }
            })
            .unwrap_or_else(|| quote! {});

        let deprecated_tokens = if tool.deprecated_since.is_some()
            || tool.remove_in.is_some()
            || tool.replaced_by.is_some()
        {
            let dep_since = tool.deprecated_since.as_deref().unwrap_or("");
            let rem_in = tool.remove_in.as_deref().unwrap_or("");
            let repl_by = tool.replaced_by.as_deref().unwrap_or("");
            quote! { .with_deprecated(#dep_since, #rem_in, #repl_by) }
        } else {
            quote! {}
        };

        // T-020: bake the `since` / `until` interval bounds into
        // the `ToolDefinition`. We emit a `.with_since(...)` /
        // `.with_until(...)` chain when either is set, so the
        // lazy-initialised `ToolDefinition` carries the metadata
        // the runtime dispatch filter reads via
        // `ToolDefinition::is_in_interval`.
        let since_tokens = tool
            .since
            .as_deref()
            .map(|s| quote! { .with_since(#s) })
            .unwrap_or_else(|| quote! {});
        let until_tokens = tool
            .until
            .as_deref()
            .map(|s| quote! { .with_until(#s) })
            .unwrap_or_else(|| quote! {});

        // T-002: when the user wrote `#[tool(desc = "...")]`, mark the
        // `ToolDefinition` so the runtime `tokitai!` config respects the
        // priority table (`CONFIG_PRIORITY_ORDER`). Doc comments and
        // synthesized defaults stay open to runtime override.
        let explicit_desc_tokens = if tool.description_explicit {
            quote! { .with_description_explicit() }
        } else {
            quote! {}
        };

        // T-016: when baked examples are present, evaluate them
        // at LazyLock initialization and append their JSON shape
        // to the schema's `examples` field. The base schema is
        // still emitted as a `'static str` literal (no runtime
        // cost when no examples are supplied). The merge is done
        // inside the LazyLock initializer (see below) so the
        // final `input_schema` string already carries the
        // examples array.
        let baked_examples_value_expr = bake_examples_to_schema_json(&tool.baked_examples);
        let baked_examples_tokens = if tool.baked_examples.is_empty() {
            quote! {}
        } else {
            quote! {
                .with_baked_examples(#baked_examples_value_expr)
            }
        };

        consts.push(quote! {
            fn #const_name() -> &'static ::tokitai::ToolDefinition {
                static DEF: ::std::sync::LazyLock<::tokitai::ToolDefinition> = ::std::sync::LazyLock::new(|| {
                    // T-016: when baked examples are present, merge
                    // them into the schema's `examples` field at
                    // initialization time so the rendered
                    // `input_schema` already carries the envelope.
                    // When no examples are present this is a plain
                    // `ToolDefinition::new(...)` call with no
                    // runtime cost beyond the LazyLock arm.
                    let __schema_json: ::std::string::String = {
                        let __base: &str = #schema_json;
                        let __examples: ::tokitai::Value = #baked_examples_value_expr;
                        if let ::tokitai::Value::Array(ref arr) = __examples {
                            if !arr.is_empty() {
                                if let ::std::result::Result::Ok(::tokitai::Value::Object(mut __map)) =
                                    ::serde_json::from_str::<::tokitai::Value>(__base)
                                {
                                    __map.insert(
                                        "examples".to_string(),
                                        ::tokitai::Value::Array(arr.clone()),
                                    );
                                    ::serde_json::to_string(&__map)
                                        .unwrap_or_else(|_| __base.to_string())
                                } else {
                                    __base.to_string()
                                }
                            } else {
                                __base.to_string()
                            }
                        } else {
                            __base.to_string()
                        }
                    };
                    ::tokitai::ToolDefinition::new(#tool_name, #description, __schema_json) #version_tokens #deprecated_tokens #explicit_desc_tokens #baked_examples_tokens #since_tokens #until_tokens
                });
                &*DEF
            }
        });

        for (i, alias_name) in tool.alias.iter().enumerate() {
            let alias_const_name =
                format_ident!("__TOOL_DEF_ALIAS_{}_{}", tool.name.to_uppercase(), i);
            let alias_desc = format!("(alias of {}) {}", tool.tool_name, tool.description);

            consts.push(quote! {
                fn #alias_const_name() -> &'static ::tokitai::ToolDefinition {
                    static DEF: ::std::sync::LazyLock<::tokitai::ToolDefinition> = ::std::sync::LazyLock::new(|| {
                        ::tokitai::ToolDefinition::new(#alias_name, #alias_desc, #schema_json)
                    });
                    &*DEF
                }
            });
        }
    }

    consts
}

/// 生成所有工具定义的数组
pub fn generate_all_tool_defs_array(
    tools: &[ToolMethodInfo],
    impl_type: &Type,
) -> Vec<TokenStream2> {
    // Same upper bound as `generate_tool_def_consts`: one entry per
    // primary tool + one per alias.
    let total: usize = tools.len() + tools.iter().map(|t| t.alias.len()).sum::<usize>();
    let mut defs = Vec::with_capacity(total);

    for tool in tools {
        let const_name = format_ident!("__TOOL_DEF_{}", tool.name.to_uppercase());
        defs.push(quote! { #impl_type::#const_name() });

        for (i, _alias_name) in tool.alias.iter().enumerate() {
            let alias_const_name =
                format_ident!("__TOOL_DEF_ALIAS_{}_{}", tool.name.to_uppercase(), i);
            defs.push(quote! { #impl_type::#alias_const_name() });
        }
    }

    defs
}

/// 【P3 优化】生成编译期工具计数常量
pub fn generate_tool_count_const(tools: &[ToolMethodInfo]) -> TokenStream2 {
    let tool_count = tools.len() + tools.iter().flat_map(|t| t.alias.iter()).count();
    quote! {
        #[allow(dead_code)]
        const __TOOL_COUNT: usize = #tool_count;
    }
}
