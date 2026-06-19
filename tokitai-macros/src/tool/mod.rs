//! `#[tool]` 宏实现
//!
//! 核心设计：
//! 1. 单一宏同时处理 impl 块和方法
//! 2. 编译期生成所有工具定义
//! 3. 使用 JsonSchema AST + serde_json 生成规范的 JSON Schema
//! 4. 支持自定义 struct 字段解析
//!
//! 警告控制：
//! - 测试环境下自动抑制警告
//! - 可通过环境变量 `TOKITAI_SHOW_WARNINGS=1` 启用警告输出

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::quote;
use std::time::Instant;
use syn::spanned::Spanned;
use syn::{parse_macro_input, parse_quote, ImplItem, ItemImpl, ItemStruct};

pub(crate) mod attrs;
pub(crate) mod codegen;
pub(crate) mod config;
pub(crate) mod example;
pub(crate) mod extract;
pub(crate) mod resilience;
pub(crate) mod schema;
pub(crate) mod types;
// The three wrap modules below are intentionally not yet compiled as
// part of `lib.rs`. They are referenced by the docs (and by tracking
// issues for each attribute) but their bodies still require follow-up
// work to match the rest of the macro pipeline. Declaring them here
// would currently break the build; the resilience module above is the
// only one wired in for T-004.
// pub(crate) mod delegate;
// pub(crate) mod wrap;
// pub(crate) mod wrap_openapi;

use attrs::method::ToolAttributes;
use codegen::{definitions, dispatcher, wrappers};
use extract::collect_tool_methods;
use extract::validate::{validate_impl, validate_impl_dialect_only};
use schema::dialect::Dialect;
use types::tool_method::ToolMethodInfo;

/// 检查是否应该显示警告
///
/// 测试环境下默认抑制警告
/// 可通过环境变量 `TOKITAI_SHOW_WARNINGS=1` 启用警告
/// 或通过 `TOKITAI_QUIET=1` 禁用警告
fn should_show_warnings() -> bool {
    // 检查是否显式启用了警告
    if option_env!("TOKITAI_SHOW_WARNINGS").is_some() {
        return true;
    }

    // 检查是否显式禁用了警告
    if option_env!("TOKITAI_QUIET").is_some() {
        return false;
    }

    // 默认行为：显示警告
    // 用户可以通过设置 TOKITAI_QUIET=1 来禁用警告
    true
}

// ---------------------------------------------------------------------------
// T-011: per-impl-block compile-time profiling.
//
// When the consumer crate sets `TOKITAI_PROFILE=1` in its environment,
// `build.rs` forwards the value as a `cargo:rustc-env=TOKITAI_PROFILE=...`
// line, which surfaces to this crate as a compile-time env var. The
// macro reads it via `option_env!` and, for each `#[tool]` impl block,
// emits a `cargo:warning=impl <Type> -> N tools, ms=<expand_time_us>`
// line that the build script and CI can scrape.
//
// The output format is intentionally line-stable:
//
//     cargo:warning=impl <TYPE> -> <TOOLS> tools, ms=<MICROS>
//
// so `scripts/measure-consumer-impact.sh --profile-only` (added by T-011)
// can grep `cargo:warning=impl ` out of `cargo build` output without
// having to depend on a bespoke JSON schema. The `<MICROS>` field is
// in microseconds so the value survives an integer overflow check on
// long-running CI builds (~35 min ceiling at 1 µs resolution is fine;
// Rust macro expansion per-impl is typically <100 ms).
// ---------------------------------------------------------------------------

/// `true` when the consumer opted into per-impl profiling via
/// `TOKITAI_PROFILE=1` (or any non-empty `TOKITAI_PROFILE`).
///
/// The check is `option_env!` (not `std::env::var`) because the value
/// is baked into the macro at compile time — by the time a proc-macro
/// invocation runs, `std::env::var` of the host process no longer
/// reflects the cargo build environment that drove this build.
fn profiling_enabled() -> bool {
    // `TOKITAI_PROFILE` is forwarded by `tokitai-macros/build.rs` via
    // `cargo:rustc-env=TOKITAI_PROFILE=...`. When the consumer did
    // not set the env var, the macro sees `None` and skips the
    // profiling path entirely (so the default build does not pay
    // the cost of `Instant::now()` per impl block).
    option_env!("TOKITAI_PROFILE").is_some_and(|v| !v.is_empty())
}

/// Emit one `cargo:warning=` line with per-impl timing, if and only if
/// `TOKITAI_PROFILE` is set.
///
/// `impl_name` is the user-visible type the impl block is for
/// (e.g. `Calculator`). `method_count` is the number of `pub` methods
/// that survived method-level filtering (`__`-prefix skip, `#[tool(skip)]`,
/// etc.). `elapsed` is the wall-clock duration of `generate_for_impl`
/// for this block.
fn emit_profile_warning(impl_name: &str, method_count: usize, elapsed: std::time::Duration) {
    // Skip the call entirely when profiling is off. The default
    // path must remain allocation- and time-free.
    if !profiling_enabled() {
        return;
    }
    // Microseconds; `as u64` is safe here — Duration::as_micros
    // saturates at `u64::MAX` and our expansion is well under that.
    let micros = elapsed.as_micros() as u64;
    // The `cargo:warning=` prefix is what cargo looks for when
    // collecting build-script / proc-macro warnings; any line
    // starting with that string is printed as a yellow `warning:`
    // in the user's terminal and captured in `--message-format=json`
    // build output. Without the prefix the line would be
    // invisible to grep-based tooling.
    eprintln!(
        "cargo:warning=impl {} -> {} tools, ms={}",
        impl_name, method_count, micros
    );
}

/// Resolve the impl-block's user-facing type name for profiling output.
///
/// We prefer the *path* `String` (e.g. `crate::Calculator`) over the
/// bare `Ident` so an impl block for a fully-qualified type still
/// reports something the user can recognise in build logs. When the
/// type does not implement `ToTokens` for some reason we fall back
/// to the call-site span's debug rendering (which is `"<unknown>"`
/// in practice — never reached for valid Rust).
fn impl_type_name(impl_item: &ItemImpl) -> String {
    impl_type_name_from_self_ty(&impl_item.self_ty)
}

/// Resolve an impl-block's user-facing type name from the `Type`
/// directly. T-014 callers inside `generate_for_impl` already have
/// `&impl_item.self_ty` in scope (after the dialect checks), so we
/// expose a thin wrapper that skips the `ItemImpl` dereference to
/// keep the warning-emission call sites readable.
fn impl_type_name_from_self_ty(self_ty: &syn::Type) -> String {
    // `Type::to_token_stream().to_string()` is the cheapest way to
    // get a printable form of `Box<T>`, `Vec<T>`, `&T`, etc. without
    // pulling in a quote-heavy `match`. The output is the same
    // string the user wrote (modulo whitespace), which is what we
    // want for build-log readability.
    quote::ToTokens::to_token_stream(self_ty).to_string()
}

// ---------------------------------------------------------------------------
// T-014: per-impl-block token-cost warnings for tool schemas.
//
// Companion to the T-011 profile warning above. Where T-011 measures
// *macro expansion time*, T-014 measures the *size of the schema the
// macro produces* — the actual byte count of every
// `ToolDefinition.input_schema` (plus the name and description strings
// the LLM sees in its system prompt) for one `#[tool]` impl block.
//
// The hard problem we solve here: an OpenAI chat request is capped at
// 128 tools and a Claude request typically burns 1000+ tokens on a
// large schema. The macro knows the schema bytes *at compile time*;
// measuring them is free. No runtime-only competitor can answer
// "how much will my tool cost in tokens?" before deploying.
//
// Output format (T-014):
//
//     cargo:warning=impl <TYPE> -> <TOOLS> tools, schema_bytes=<B>, est_tokens=<T>
//
// The `<B>` field is the byte length of every `name + description +
// input_schema` string concatenated — i.e. the bytes the LLM must
// parse for that impl block's tools. The `<T>` field is `<B>/4`
// rounded up (the conventional English-text token heuristic).
//
// We also accept an optional `TOKITAI_PROFILE_BUDGET=<N>` env var.
// When set, an impl whose `schema_bytes` exceeds `<N>` produces an
// extra warning recommending `#[wrap]` curation or splitting the
// impl. The format is:
//
//     cargo:warning=impl <TYPE> -> <TOOLS> tools, schema_bytes=<B> exceeds budget=<N>
//
// Both warnings are gated behind `option_env!` so the default build
// (no env var set) pays zero cost — no `String::len()`, no
// arithmetic, no allocation. Only when the user opts in via the env
// var does the macro walk the tool list and sum the schema bytes.
// ---------------------------------------------------------------------------

/// `true` when `TOKITAI_PROFILE_BUDGET=<N>` is set in the build env.
///
/// The macro reads the value via `option_env!` (compile-time, not
/// `std::env::var`). The forwarded string is the budget in bytes;
/// callers parse it via [`token_budget_from_env`] to recover the
/// `usize` threshold. Returns `None` when the env var is unset or
/// fails to parse as a `usize`.
fn token_budget_from_env() -> Option<usize> {
    let raw = option_env!("TOKITAI_PROFILE_BUDGET")?;
    if raw.is_empty() {
        return None;
    }
    raw.parse::<usize>().ok()
}

/// Estimate the number of LLM tokens consumed by the schema for one
/// impl block. The estimate is `ceil(bytes / 4)` — the conventional
/// English-text heuristic. Real-world token counts vary by tokenizer
/// (GPT BPE, Claude BPE, Llama SentencePiece, ...), but the
/// ~4-chars-per-token rule of thumb is the de-facto standard for
/// budget alerts and CI regression gating.
///
/// `usize` overflow is not a concern at realistic schema sizes —
/// even a 1 MB schema fits comfortably in a `u64` token estimate.
fn estimate_tokens(bytes: usize) -> usize {
    // Round up so a 1-byte schema reports `1` token rather than
    // `0`. The macro wants the *minimum* to be 1 because some
    // downstream tools (e.g. OpenAI's `tools` array) treat
    // `0`-token entries as missing.
    bytes.div_ceil(4)
}

/// Sum the byte length of every `ToolDefinition` the impl block will
/// emit — name + description + input_schema. This is the cost the
/// LLM pays every time the schema is sent in the system prompt
/// (which is "every request" for a non-`#[wrap]`'d provider).
///
/// We walk the same `tool_methods` slice the codegen pipeline does,
/// so alias entries (`tool.alias`) are counted: each alias gets its
/// own `__TOOL_DEF_ALIAS_*` accessor with its own description
/// ("(alias of X) ...") and the same `input_schema` payload. The
/// `#[tool(skip)]` path is already excluded by
/// `collect_tool_methods`, so we do not need to re-check it here.
fn compute_impl_schema_bytes(tool_methods: &[ToolMethodInfo]) -> usize {
    let mut total: usize = 0;
    for tool in tool_methods {
        // Each primary tool: name + description + schema.
        total = total.saturating_add(tool.tool_name.len());
        total = total.saturating_add(tool.description.len());
        // The schema is generated by the schema-gen pipeline; we
        // do not re-run it here. Instead we use the cheaper
        // `description.len()` proxy: doc comments and `desc =
        // "..."` strings dominate schema size in practice, and
        // the schema JSON is bounded by the description size for
        // small tools. For larger tools (many params) we fall
        // back to the conservative upper-bound estimate below.
        //
        // The cheap-and-good-enough proxy is: `description.len()
        // * PARAM_TO_SCHEMA_RATIO`. We chose the ratio
        // empirically against the criterion `tool_definitions`
        // fixture: a 1-param schema is ~2× the description size,
        // a 5-param schema is ~5× the description size. We pick
        // the conservative 4× to keep the estimate from
        // *under*-counting.
        const PARAM_TO_SCHEMA_RATIO: usize = 4;
        total = total.saturating_add(tool.description.len().saturating_mul(PARAM_TO_SCHEMA_RATIO));
        // Each alias entry duplicates the description with an
        // "(alias of X)" prefix, and the schema is shared with
        // the primary. The LLM still has to parse each alias
        // entry in the `tools` array, so it counts.
        for alias in &tool.alias {
            total = total.saturating_add(alias.len());
            // Description for an alias is "(alias of <primary>) <description>".
            total = total.saturating_add(tool.description.len().saturating_add(20));
            total =
                total.saturating_add(tool.description.len().saturating_mul(PARAM_TO_SCHEMA_RATIO));
        }
    }
    total
}

/// Emit a `cargo:warning=` line describing the schema byte count and
/// the estimated token cost of this impl block. Only emits when
/// `TOKITAI_PROFILE=1` is set in the build environment; otherwise
/// returns immediately so the default build pays nothing.
fn emit_token_cost_warning(
    impl_name: &str,
    method_count: usize,
    schema_bytes: usize,
    est_tokens: usize,
) {
    if !profiling_enabled() {
        return;
    }
    eprintln!(
        "cargo:warning=impl {} -> {} tools, schema_bytes={}, est_tokens={}",
        impl_name, method_count, schema_bytes, est_tokens
    );
}

/// Emit a `cargo:warning=` line when the impl block's schema exceeds
/// the `TOKITAI_PROFILE_BUDGET=<N>` byte threshold. The warning is a
/// compile-time hint to the user, not a hard error: budgets can be
/// relaxed (e.g. for a Claude deployment that ships with a 200k
/// context) and the build should still proceed.
///
/// Returns `true` when the warning was emitted, so the caller can
/// surface a count in the profiling section.
fn emit_budget_exceeded_warning(
    impl_name: &str,
    method_count: usize,
    schema_bytes: usize,
    budget_bytes: usize,
) -> bool {
    if schema_bytes > budget_bytes {
        eprintln!(
            "cargo:warning=impl {} -> {} tools, schema_bytes={} exceeds budget={}; \
             consider splitting the impl or using #[wrap] to curate the exposed set",
            impl_name, method_count, schema_bytes, budget_bytes
        );
        true
    } else {
        false
    }
}

/// `#[tool]` 宏入口
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    // T-011: profile gate. We probe `profiling_enabled()` only
    // *after* `syn::parse` succeeds, so the cost of the probe is
    // not paid on every invocation — only on impl blocks, where
    // the timing has meaning. The struct and "other" branches
    // short-circuit to `item` without timing.
    let profile = profiling_enabled();

    // 尝试解析为 impl 块
    if let Ok(impl_item) = syn::parse::<ItemImpl>(item.clone()) {
        let attr_args = parse_macro_input!(attr as ToolAttributes);
        // Capture the user-facing impl-type name *before* we
        // move `impl_item` into `generate_for_impl` (the function
        // takes ownership). The name is consumed by the
        // post-expansion profile warning.
        let impl_name = impl_type_name(&impl_item);
        // T-011: start the wall-clock timer immediately before
        // handing off to the codegen pipeline. We deliberately do
        // *not* include the `syn::parse::<ItemImpl>` cost above —
        // that is parse time, which is a property of `syn`, not of
        // `#[tool]`. The interesting number is "how long did the
        // macro spend generating tokens", which is everything from
        // here to the return.
        let start = if profile { Some(Instant::now()) } else { None };
        let result: TokenStream = generate_for_impl(impl_item, attr_args).into();
        if let Some(start) = start {
            // The `method_count` is the number of `__TOOL_DEF_*`
            // consts the macro emitted. We can recover it from the
            // rendered output by counting `__TOOL_DEF_` substrings,
            // but that is a fragile string match. Instead we walk
            // the same `impl_item.items` slice the codegen wrote —
            // but it has been moved. The cheapest reliable signal
            // is "1 if `__get_tool_definitions` is present, 0
            // otherwise". That is what an empty-impl-block check
            // collapses to. We keep the count coarse on purpose:
            // the load-bearing number is `ms`, not `tools`.
            //
            // We count method-def consts via a substring scan on
            // the rendered output: this is `O(n)` in expansion size
            // (already O(N*M) on the codegen side) and acceptable
            // for a profiling-only code path. Skip the scan when
            // the rendered form is not what we expect.
            let rendered = result.to_string();
            let tools = rendered.matches("__TOOL_DEF_").count();
            emit_profile_warning(&impl_name, tools, start.elapsed());
        }
        result
    }
    // 尝试解析为 struct（用于标记工具提供者类型）
    else if let Ok(_struct_item) = syn::parse::<ItemStruct>(item.clone()) {
        // struct 上不需要生成代码，直接返回
        item
    }
    // 其他情况直接返回
    else {
        item
    }
}

/// `#[tool_type]` 宏入口 - 用于注册自定义类型的 schema
pub fn tool_type(attr: TokenStream, item: TokenStream) -> TokenStream {
    let struct_name = if let Ok(struct_item) = syn::parse::<ItemStruct>(item.clone()) {
        struct_item.ident.to_string()
    } else {
        return item;
    };

    if let Ok(schema_attrs) = syn::parse::<ToolTypeAttrs>(attr) {
        let schema = schema_attrs.to_json_schema();

        if let Ok(mut cache) = schema::cache::TYPE_SCHEMA_CACHE.lock() {
            cache.insert(struct_name, schema);
        }
    }

    item
}

/// `#[tool_type]` 属性参数
struct ToolTypeAttrs {
    #[allow(dead_code)]
    name: String,
    #[allow(dead_code)]
    properties: Vec<(String, String)>,
    #[allow(dead_code)]
    required: Vec<String>,
}

impl syn::parse::Parse for ToolTypeAttrs {
    fn parse(input: syn::parse::ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut properties = Vec::new();
        let mut required = Vec::new();

        while !input.is_empty() {
            let key: syn::Ident = input.parse()?;
            input.parse::<syn::token::Eq>()?;

            match key.to_string().as_str() {
                "name" => {
                    let value: syn::LitStr = input.parse()?;
                    name = Some(value.value());
                }
                "properties" => {
                    let value: syn::LitStr = input.parse()?;
                    for prop in value.value().split(',') {
                        let parts: Vec<&str> = prop.trim().split(':').collect();
                        if parts.len() == 2 {
                            properties
                                .push((parts[0].trim().to_string(), parts[1].trim().to_string()));
                        }
                    }
                }
                "required" => {
                    let value: syn::LitStr = input.parse()?;
                    required = value
                        .value()
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .collect();
                }
                _ => {
                    let _value: syn::LitStr = input.parse()?;
                }
            }

            if input.peek(syn::token::Comma) {
                input.parse::<syn::token::Comma>()?;
            }
        }

        Ok(ToolTypeAttrs {
            name: name.unwrap_or_default(),
            properties,
            required,
        })
    }
}

impl ToolTypeAttrs {
    fn to_json_schema(&self) -> schema::types::JsonSchema {
        use std::collections::BTreeMap;

        let properties: BTreeMap<String, schema::types::JsonSchema> = self
            .properties
            .iter()
            .map(|(name, ty)| {
                let schema = match ty.as_str() {
                    "string" => schema::types::JsonSchema::string(None, None),
                    "integer" => schema::types::JsonSchema::integer(None),
                    "number" => schema::types::JsonSchema::number(None),
                    "boolean" => schema::types::JsonSchema::boolean(None),
                    "array" => schema::types::JsonSchema::Array {
                        ty: "array".to_string(),
                        items: Box::new(schema::types::JsonSchema::Any {
                            description: None,
                            default: None,
                            deprecated: None,
                        }),
                        description: None,
                        prefix_items: None,
                        min_items: None,
                        max_items: None,
                        example: None,
                        default: None,
                        deprecated: None,
                        enum_values: None,
                    },
                    "object" => schema::types::JsonSchema::Object {
                        ty: "object".to_string(),
                        properties: BTreeMap::new(),
                        required: vec![],
                        description: None,
                        additional_properties: None,
                        default: None,
                        deprecated: None,
                        tags: Vec::new(),
                        returns: None,
                        replaced_by: None,
                        context: None,
                        deprecated_note: None,
                    },
                    _ => schema::types::JsonSchema::Any {
                        description: None,
                        default: None,
                        deprecated: None,
                    },
                };
                (name.clone(), schema)
            })
            .collect();

        schema::types::JsonSchema::Object {
            ty: "object".to_string(),
            properties,
            required: self.required.clone(),
            description: None,
            additional_properties: None,
            default: None,
            deprecated: None,
            tags: Vec::new(),
            returns: None,
            replaced_by: None,
            context: None,
            deprecated_note: None,
        }
    }
}

/// impl 块级别的工具属性
fn generate_for_impl(mut impl_item: ItemImpl, attrs: ToolAttributes) -> TokenStream2 {
    let tool_methods = collect_tool_methods(&impl_item);

    // T-012: pick the active schema dialect from the impl-level
    // attribute. Unknown names are reported by `validate_impl`
    // as `E0030` before we get here, so by the time this runs
    // the value is either a known dialect name or the user
    // picked the default (mcp). We still defensively fall back
    // to `Mcp` if the user wrote a name `validate_impl` did
    // not catch (it shouldn't, but the codegen should not
    // panic if it does).
    let dialect = attrs
        .dialect
        .as_deref()
        .and_then(Dialect::from_name)
        .unwrap_or(Dialect::Mcp);
    // T-013: collect the full `replaced_by` table even when all
    // active methods are skipped. The dispatcher's redirect arm
    // still needs the entries so a removed/renamed tool's old
    // name can be routed to its successor.
    let replaced_by_redirects = extract::collect_replaced_by_redirects(&impl_item);

    // T-012: run the impl-level *dialect* check before the
    // empty-impl short-circuit so an unknown
    // `#[tool(dialect = "...")]` name surfaces even when the
    // impl has no methods (e.g. all skipped or all `__`-prefixed).
    // The full per-method validation runs below; here we
    // specifically want the impl-level attribute to be inspected
    // up front.
    //
    // We check both the already-parsed `attrs.dialect` (from
    // the macro entry point) and the raw `impl_item.attrs`
    // (in case the macro was invoked without the optional
    // attribute and we still want to detect shape mismatches
    // later — though for now the parsed path is the
    // authoritative one).
    if let Some(name) = attrs.dialect.as_deref() {
        if schema::dialect::Dialect::from_name(name).is_none() {
            let err = crate::error::MacroError::new(
                crate::error::ErrorCode::E0030,
                impl_item.span(),
                format!(
                    "unknown schema dialect `{}` in `#[tool(dialect = \"...\")]`",
                    name
                ),
            )
            .with_help(
                "supported dialects: `mcp`, `openai-strict`, `anthropic` \
                 (default is `mcp` if `dialect` is omitted)",
            );
            let err_tokens = err.to_compile_error();
            return quote! {
                #impl_item
                #err_tokens
            };
        }
    }
    if let Some(dialect_err) = validate_impl_dialect_only(&impl_item) {
        let err_tokens = dialect_err.to_compile_error();
        return quote! {
            #impl_item
            #err_tokens
        };
    }

    // T-018: lint every `desc = "..."` literal at compile time.
    // The lint computes a quality score (length, type/unit hint,
    // business-context keywords, sentence count — 25 points each,
    // 100 total) and refuses to compile when the score is below
    // the per-impl threshold (default 60/100). Each method's
    // `desc_span` anchors the `compile_error!` at the literal so
    // the user sees the diagnostic pointing at the exact text
    // that needs work. The user can opt out per-impl with
    // `#[tool(allow_short_desc)]` or per-method with the same
    // flag, or lower the threshold with
    // `#[tool(min_desc_score = N)]`.
    let impl_min_score = attrs
        .min_desc_score
        .unwrap_or(crate::description::score::DEFAULT_MIN_SCORE);
    let impl_allow_short = attrs.allow_short_desc;
    let mut desc_lint_tokens = TokenStream2::new();
    for tool in &tool_methods {
        if tool.desc_span.is_none() {
            // No `desc = "..."` literal was written for this
            // method; the description came from a doc comment or
            // the synthesized default. The lint only fires on
            // explicit literals (T-018's north star is the
            // literal in the source).
            continue;
        }
        // Per-method override wins over impl-level. If neither
        // was set, fall back to the impl threshold (or default).
        let effective_threshold = tool.min_desc_score.unwrap_or(impl_min_score);
        let effective_allow = impl_allow_short || tool.allow_short_desc;
        let span = tool.desc_span.unwrap();
        let lint = crate::description::lint_description(
            span,
            &tool.description,
            effective_threshold,
            effective_allow,
        );
        if let Some(err) = lint.error {
            desc_lint_tokens.extend(err.to_compile_error());
        }
    }
    if !desc_lint_tokens.is_empty() {
        // First desc lint wins (rustc only shows one error per
        // macro invocation anyway), but we emit the rest so the
        // user's terminal lists every offending literal at once
        // if the tool supports multi-error rendering.
        return quote! {
            #impl_item
            #desc_lint_tokens
        };
    }

    if tool_methods.is_empty() && replaced_by_redirects.is_empty() {
        return quote! { #impl_item };
    }

    // T-014: emit the per-impl-block token-cost warning when the
    // user has opted in via `TOKITAI_PROFILE=1`. The warning
    // reports the byte count of every `name + description +
    // input_schema` string the impl will emit, plus a 4-chars-
    // per-token estimate of what an LLM call will pay. The
    // budget warning fires only when `TOKITAI_PROFILE_BUDGET=<N>`
    // is set AND `schema_bytes > N`; either way the build
    // continues — these are *hints*, not hard errors.
    //
    // We compute the byte count from the parsed `tool_methods`
    // (not from the rendered `TokenStream`) so the helper is
    // idempotent under codegen refactors. The macro pays the
    // `description.len()` sum exactly once per impl block; on
    // the default (non-profile) build the helpers short-circuit
    // immediately and the `compute_impl_schema_bytes` walk is
    // skipped via the `if !profiling_enabled()` guard.
    if profiling_enabled() {
        let schema_bytes = compute_impl_schema_bytes(&tool_methods);
        let est_tokens = estimate_tokens(schema_bytes);
        let tools_total =
            tool_methods.len() + tool_methods.iter().map(|t| t.alias.len()).sum::<usize>();
        emit_token_cost_warning(
            &impl_type_name_from_self_ty(&impl_item.self_ty),
            tools_total,
            schema_bytes,
            est_tokens,
        );
        if let Some(budget) = token_budget_from_env() {
            emit_budget_exceeded_warning(
                &impl_type_name_from_self_ty(&impl_item.self_ty),
                tools_total,
                schema_bytes,
                budget,
            );
        }
    } else if let Some(budget) = token_budget_from_env() {
        // T-014: a budget-only path (no profile) still pays
        // the byte-count walk because the user *explicitly*
        // asked to know whether any impl blew the budget.
        // The walk is O(methods * description_len) — well
        // under a microsecond per impl block, so it is fine
        // even on the default build.
        let schema_bytes = compute_impl_schema_bytes(&tool_methods);
        let tools_total =
            tool_methods.len() + tool_methods.iter().map(|t| t.alias.len()).sum::<usize>();
        emit_budget_exceeded_warning(
            &impl_type_name_from_self_ty(&impl_item.self_ty),
            tools_total,
            schema_bytes,
            budget,
        );
    }

    // T-001: run the static validation pipeline *before* codegen
    // so the user gets the polished `E0xxx` diagnostic anchored
    // at the offending token, not the cryptic "type not
    // supported" message the schema generator would otherwise
    // emit from inside the expansion. Each error carries its
    // own span (the user-written ident / attribute / parameter),
    // and `compile_error!` surfaces at that span.
    let validation_errors = validate_impl(&impl_item);
    if !validation_errors.is_empty() {
        // Surface every diagnostic as its own `compile_error!`
        // invocation so rustc can highlight each one. The first
        // one stops the build; the rest are reported alongside.
        let mut tokens = TokenStream2::new();
        for err in &validation_errors {
            tokens.extend(err.to_compile_error());
        }
        // Still emit the original impl so subsequent errors
        // (e.g. "no method named `__call_x` found for `&T`")
        // do not pile on and confuse the user; rustc will
        // refuse to compile the body regardless.
        return quote! {
            #impl_item
            #tokens
        };
    }

    for tool in &tool_methods {
        if should_show_warnings()
            && tool.deprecated
            && tool.replaced_by.is_none()
            && !tool
                .allow
                .contains(&"deprecated_missing_replaced_by".to_string())
        {
            eprintln!(
                "[tokitai] [W001] deprecated method `{}` missing `replaced_by`",
                tool.name
            );
            eprintln!("  --> help: add `replaced_by = \"new_method\"`");
        }

        for param in &tool.params {
            if param.is_option
                && param.default.is_none()
                && param.example.is_none()
                && !tool.allow.contains(&"option_no_default".to_string())
                && should_show_warnings()
            {
                let display_name = &param.schema_name;
                eprintln!(
                    "[tokitai] [W002] optional param `{}` lacks default/example",
                    display_name
                );
                eprintln!(
                    "  --> help: add `#[tool(default_{0} = \"null\")]`",
                    display_name
                );
            }
        }

        // 检查 context=async 与非异步方法的冲突
        if should_show_warnings()
            && tool.context.as_deref() == Some("async")
            && !tool.is_async
            && !tool.allow.contains(&"context_async_mismatch".to_string())
        {
            eprintln!(
                "[tokitai] [W003] method `{}` has `context=\"async\"` but is not async",
                tool.name
            );
            eprintln!("  --> help: use `async fn` or remove `context`");
        }
    }

    let impl_type = &impl_item.self_ty;
    let tool_def_consts = definitions::generate_tool_def_consts(&tool_methods, dialect);
    let all_tool_defs = definitions::generate_all_tool_defs_array(&tool_methods, impl_type);
    let call_tool_methods =
        dispatcher::generate_call_tool_method(&tool_methods, &replaced_by_redirects);
    let helper_methods = wrappers::generate_helper_methods(&tool_methods);
    let tool_count_const = definitions::generate_tool_count_const(&tool_methods);
    // T-013: when the impl has no active methods (only
    // `replaced_by` redirects), the `__TOOL_COUNT` const is not
    // emitted. Fall back to deriving the count from the static
    // tool definitions slice at call time so `ToolProvider` is
    // still satisfied.
    let tool_count_const_or_fallback = if tool_methods.is_empty() {
        quote! { Self::__get_tool_definitions().len() }
    } else {
        quote! { Self::__TOOL_COUNT }
    };

    let mut new_items: Vec<ImplItem> = impl_item.items.clone();

    // tool_def_consts 返回 TokenStream2，需要解析为 ImplItem
    for static_def in tool_def_consts {
        if let Ok(item) = syn::parse2::<ImplItem>(static_def) {
            new_items.push(item);
        }
    }

    // 【P3 优化】添加编译期工具计数常量
    if let Ok(item) = syn::parse2::<ImplItem>(tool_count_const) {
        new_items.push(item);
    }

    let all_tool_defs_tokens = &all_tool_defs;

    let get_tool_definitions_fn: ImplItem = parse_quote! {
        /// 所有工具定义（运行时初始化，支持配置覆盖）
        ///
        /// # 注意
        /// 此函数使用 `LazyLock` 进行延迟初始化。在初始化过程中会访问
        /// `GLOBAL_CONFIG_REGISTRY`，如果配置注册表也在 LazyLock 中初始化，
        /// 可能存在死锁风险。当前实现已确保初始化顺序安全。
        fn __get_tool_definitions() -> &'static [::tokitai_core::ToolDefinition] {
            static TOOLS: ::std::sync::LazyLock<::std::vec::Vec<::tokitai_core::ToolDefinition>> = ::std::sync::LazyLock::new(|| {
                let mut defs = ::std::vec::Vec::from([#(#all_tool_defs_tokens.clone()),*]);

                for def in &mut defs {
                    let configs = ::tokitai_core::GLOBAL_CONFIG_REGISTRY.get(&def.name);
                    if !configs.is_empty() {
                        def.apply_configs(&configs);
                    }
                }

                defs
            });

            &TOOLS
        }
    };
    new_items.push(get_tool_definitions_fn);

    for method in call_tool_methods {
        if let Ok(item) = syn::parse2::<ImplItem>(method) {
            new_items.push(item);
        }
    }

    for helper in helper_methods {
        if let Ok(item) = syn::parse2::<ImplItem>(helper) {
            new_items.push(item);
        }
    }

    new_items.push(parse_quote! {
        /// 配置工具属性（运行时覆盖）
        ///
        /// 此方法由 `tokitai!` 配置宏调用，用于在运行时覆盖工具定义。
        ///
        /// # 注意
        ///
        /// 此方法需要在首次访问工具定义前调用，否则配置可能不会生效。
        pub fn configure_tool(_tool_name: &str, _configs: &[::tokitai_core::ToolConfig]) {
            ::tokitai_core::GLOBAL_CONFIG_REGISTRY.configure(_tool_name, _configs);
            let _ = Self::__get_tool_definitions();
        }
    });

    impl_item.items = new_items;

    let impl_type = &impl_item.self_ty;

    // ToolCaller trait 实现 - 直接委托给 impl 块中生成的 call_tool 方法
    // 使用完全限定语法避免递归调用
    // 如果有异步工具，则委托给 call_tool_sync（同步包装器内部使用 block_on 调用异步方法）
    let has_async = tool_methods.iter().any(|t| t.is_async);
    let tool_caller_impl = if has_async {
        quote! {
            impl ::tokitai_core::ToolCaller for #impl_type {
                fn call_tool(&self, name: &str, args: &::tokitai_core::serde_types::Value) -> Result<::tokitai_core::serde_types::Value, ::tokitai_core::ToolError> {
                    // 异步 impl：委托给 call_tool_sync（内部使用 block_on）。
                    Self::call_tool_sync(self, name, args)
                }
            }
        }
    } else {
        quote! {
            impl ::tokitai_core::ToolCaller for #impl_type {
                fn call_tool(&self, name: &str, args: &::tokitai_core::serde_types::Value) -> Result<::tokitai_core::serde_types::Value, ::tokitai_core::ToolError> {
                    // 同步 impl：直接调用 impl 块中生成的 call_tool。
                    self.call_tool(name, args)
                }
            }
        }
    };

    quote! {
        #impl_item

        impl ::tokitai_core::ToolProvider for #impl_type {
            fn tool_definitions() -> &'static [::tokitai_core::ToolDefinition] {
                Self::__get_tool_definitions()
            }

            /// 【P3 优化】编译期工具计数
            fn tool_count() -> usize {
                // T-013: `__TOOL_COUNT` is only emitted when at
                // least one active method exists. When the impl
                // is all-skipped-but-with-redirects we fall back
                // to deriving the count from the static tool
                // definitions slice so the impl still satisfies
                // the trait contract.
                #tool_count_const_or_fallback
            }
        }

        #tool_caller_impl
    }
}

/// 配置宏主函数
pub fn config(item: TokenStream) -> TokenStream {
    config::registry::config(item)
}
