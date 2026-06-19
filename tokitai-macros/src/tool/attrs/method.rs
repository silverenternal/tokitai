//! 方法级工具属性解析

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{
    parse::{Parse, ParseStream},
    token, Expr, Ident, Lit, LitStr,
};

use super::param::{
    parse_json_value, parse_lit_to_f64, parse_lit_to_string, parse_lit_to_usize, parse_value_string,
};
use crate::tool::example::{parse_example_singular, parse_examples_array, BakedExample};
use crate::tool::types::param::ParamToolAttrs;

/// impl-block-level tool attributes.
///
/// The `name` and `description` fields are reserved for a future
/// tool-registry feature (see `silverenternal/tokitai#42`) that
/// would let users override per-impl tool metadata from a single
/// place. The current release only uses the per-method `@tool(name=…)`
/// and `@tool(desc=…)` doc-comment attributes; the impl-level fields
/// are accepted by the parser for forward compatibility and stored
/// as dead-code so downstream registry code can adopt them without
/// another breaking change.
///
/// T-012 added the `dialect` field. When set, the macro audits every
/// generated `ToolDefinition.input_schema` against the chosen
/// provider's known quirks. Unknown dialect names are reported as
/// `E0030`. The default is `mcp` (loosest), so users who don't opt
/// in see no behavior change.
#[derive(Default)]
pub struct ToolAttributes {
    /// Reserved for the tool-registry feature.
    #[allow(dead_code)]
    pub name: Option<String>,
    /// Reserved for the tool-registry feature.
    #[allow(dead_code)]
    pub description: Option<String>,
    /// T-012: schema dialect for compile-time correctness
    /// audits (`mcp` / `openai-strict` / `anthropic`).
    pub dialect: Option<String>,
    /// T-018: per-impl minimum description score. Methods whose
    /// `desc = "..."` literal scores below this threshold are
    /// rejected at compile time. Defaults to 60 when `None`.
    pub min_desc_score: Option<u8>,
    /// T-018: when `true`, every `desc = "..."` literal on this
    /// impl block bypasses the score lint. Used for one-word
    /// verbs and other intentionally terse descriptions.
    pub allow_short_desc: bool,
    /// T-022: when `true`, every `desc = "..."` literal on this
    /// impl block bypasses the adversarial-description safety
    /// lint. Used for audited security-test fixtures that need
    /// to ship a known-bad literal; production code paths should
    /// leave this off.
    pub allow_insecure_desc: bool,
    /// T-020: when `Some("semver")`, the macro parses every
    /// `since = "..."` / `until = "..."` literal as SemVer and
    /// refuses to compile when a literal fails to parse or
    /// produces an invalid interval (`since >= until`). When
    /// `None`, the macro accepts any string and uses
    /// lexicographic ordering at runtime.
    pub version_policy: Option<String>,
}

impl Parse for ToolAttributes {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut description = None;
        let mut dialect = None;
        let mut min_desc_score: Option<u8> = None;
        let mut allow_short_desc = false;
        let mut allow_insecure_desc = false;
        let mut version_policy: Option<String> = None;

        // 支持空输入（impl 块级别的 #[tool] 不需要参数）
        if input.is_empty() {
            return Ok(ToolAttributes {
                name,
                description,
                dialect,
                min_desc_score,
                allow_short_desc,
                allow_insecure_desc,
                version_policy,
            });
        }

        while !input.is_empty() {
            let key: Ident = input.parse()?;

            // T-018 / T-022: `allow_short_desc` and
            // `allow_insecure_desc` are bare flags (no `= "..."`).
            // The other keys are key=value pairs.
            if key == "allow_short_desc" {
                allow_short_desc = true;
                if input.peek(token::Comma) {
                    input.parse::<token::Comma>()?;
                }
                continue;
            }
            if key == "allow_insecure_desc" {
                allow_insecure_desc = true;
                if input.peek(token::Comma) {
                    input.parse::<token::Comma>()?;
                }
                continue;
            }

            input.parse::<token::Eq>()?;

            // T-018: `min_desc_score` accepts an integer literal
            // (`= 60`) instead of a string literal. We peek at the
            // first token: if it is a `LitInt`, we consume it and
            // parse as a `u8`; otherwise we fall through to the
            // string-literal path so every other key continues to
            // work.
            if key == "min_desc_score" {
                if let Ok(lit_int) = input.parse::<syn::LitInt>() {
                    min_desc_score = lit_int.base10_parse::<u8>().ok();
                } else if let Ok(lit_str) = input.parse::<LitStr>() {
                    min_desc_score = lit_str.value().parse().ok();
                }
                if input.peek(token::Comma) {
                    input.parse::<token::Comma>()?;
                }
                continue;
            }

            let value: LitStr = input.parse()?;
            match key.to_string().as_str() {
                "name" => name = Some(value.value()),
                "desc" | "description" => description = Some(value.value()),
                // T-012: `#[tool(dialect = "openai-strict")]` (or
                // `"mcp"` / `"anthropic"`). Unknown names are
                // surfaced by `validate_impl` as `E0030` so the
                // user sees the diagnostic at the impl-block
                // span, not at the first method.
                "dialect" => dialect = Some(value.value()),
                // T-020: opt into SemVer-ordered comparison for
                // `since` / `until` literals. Accepts `"semver"`
                // (the only value today); unknown values are
                // surfaced by `validate_impl` as `E0030`.
                "version_policy" => version_policy = Some(value.value()),
                _ => {}
            }

            if input.peek(token::Comma) {
                input.parse::<token::Comma>()?;
            }
        }

        Ok(ToolAttributes {
            name,
            description,
            dialect,
            min_desc_score,
            allow_short_desc,
            allow_insecure_desc,
            version_policy,
        })
    }
}

/// 方法级别的工具属性
#[derive(Default)]
pub struct MethodToolAttrs {
    pub name: Option<String>,
    pub desc: Option<String>,
    pub skip: bool,
    pub deprecated: bool,
    pub replaced_by: Option<String>,
    pub deprecated_note: Option<String>,
    pub deprecated_since: Option<String>,
    pub remove_in: Option<String>,
    pub version: Option<String>,
    /// T-020: lower bound (inclusive) of the schema-evolution
    /// interval. When set, the dispatcher only serves the tool
    /// when `tokitai_core::current_version() >= since`. Compared
    /// via SemVer when `version_policy = "semver"` is set on the
    /// impl block; otherwise lexicographic.
    pub since: Option<String>,
    /// T-020: upper bound (exclusive) of the schema-evolution
    /// interval. When set, the dispatcher hides the tool when
    /// `tokitai_core::current_version() >= until`.
    pub until: Option<String>,
    pub visible: bool,
    pub tags: Vec<String>,
    pub group: Option<String>,
    pub return_description: Option<String>,
    pub context: Option<String>,
    pub example_input: Option<serde_json::Value>,
    pub param_order: Option<Vec<String>>,
    pub hidden_params: Vec<String>,
    pub example_output: Option<String>,
    pub alias: Vec<String>,
    pub allow: Vec<String>,
    pub cache: Option<String>,
    pub rate_limit: Option<String>,
    pub param_validations: Vec<(String, ParamToolAttrs)>,
    /// T-016: baked few-shot examples as Rust expressions. Each
    /// entry is the parsed `(args_tokens, result_tokens)` pair
    /// from a `call!(self.method(args) => result)` literal. The
    /// macro inlines them at two sites: a compile-time type check
    /// (so stale examples cannot ship) and the schema's
    /// `examples` field (so the LLM sees `{ "input": ..., "output": ... }`).
    pub baked_examples: Vec<BakedExample>,
    /// T-018: per-method override of the score threshold. Used
    /// when one method in an impl legitimately needs a lower
    /// bar (e.g. a getter whose name carries all the meaning).
    pub min_desc_score: Option<u8>,
    /// T-018: when `true`, this method's `desc = "..."` literal
    /// bypasses the score lint. Used for one-word verbs and
    /// other intentionally terse descriptions.
    pub allow_short_desc: bool,
    /// T-022: when `true`, this method's `desc = "..."` literal
    /// bypasses the adversarial-description safety lint. Used
    /// for audited security-test fixtures that need to ship a
    /// known-bad literal; production code paths should leave
    /// this off.
    pub allow_insecure_desc: bool,
    /// T-022: per-method extension of the bad-pattern set. Each
    /// entry is a case-insensitive substring; a hit raises the
    /// safety lint for this method only. Useful when an org has
    /// a per-tool policy (e.g. "never describe a `send_email`
    /// tool as `urgent`") that the in-source default does not
    /// cover.
    pub desc_blocklist: Vec<String>,
    /// T-019: per-method byte budget for the serialized result.
    /// When set, the macro compiles a runtime guard into the
    /// `__call_*` wrapper that truncates the result or returns
    /// `ToolError::Truncated` once the budget is exceeded. The
    /// value of `0` is a compile error (the sentinel would
    /// consume the whole output).
    pub result_truncate_bytes: Option<usize>,
}

impl Parse for MethodToolAttrs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(Ident) {
            let key: Ident = input.fork().parse()?;
            // `skip` is special: it is the only attribute that
            // historically did not accept any companion keys
            // (`#[tool(skip)]`). T-013 added the requirement for
            // `#[tool(skip, replaced_by = "...", version = "...")]`
            // so a removed tool can keep its `replaced_by` link
            // even when its method is filtered out of the active
            // dispatcher. The skip fast-path now only fires when
            // there is exactly one token (`skip`) and nothing else.
            if key == "skip" && !input.peek2(token::Eq) {
                let is_just_skip = input.peek2(token::Comma) && !input.peek3(Ident);
                // T-013: only short-circuit when the attribute body
                // is exactly `skip` (optionally followed by a single
                // trailing comma). For any other shape, fall through
                // to the full key/value parser so companion
                // attributes (notably `replaced_by`) survive.
                if is_just_skip {
                    input.parse::<Ident>()?;
                    if input.peek(token::Comma) {
                        input.parse::<token::Comma>()?;
                    }
                    return Ok(MethodToolAttrs {
                        name: None,
                        desc: None,
                        skip: true,
                        deprecated: false,
                        replaced_by: None,
                        deprecated_note: None,
                        deprecated_since: None,
                        remove_in: None,
                        version: None,
                        since: None,
                        until: None,
                        visible: true,
                        tags: Vec::new(),
                        group: None,
                        return_description: None,
                        context: None,
                        example_input: None,
                        param_order: None,
                        hidden_params: Vec::new(),
                        example_output: None,
                        alias: Vec::new(),
                        allow: Vec::new(),
                        cache: None,
                        rate_limit: None,
                        param_validations: Vec::new(),
                        baked_examples: Vec::new(),
                        min_desc_score: None,
                        allow_short_desc: false,
                        allow_insecure_desc: false,
                        desc_blocklist: Vec::new(),
                        result_truncate_bytes: None,
                    });
                }
            }
        }

        let mut name = None;
        let mut desc = None;
        let mut should_skip = false;
        let mut deprecated = false;
        let mut replaced_by = None;
        let mut deprecated_note = None;
        let mut deprecated_since = None;
        let mut remove_in = None;
        let mut version = None;
        let mut since: Option<String> = None;
        let mut until: Option<String> = None;
        let mut visible = true;
        let mut tags = Vec::new();
        let mut group = None;
        let mut return_description = None;
        let mut context = None;
        let mut example_input: Option<serde_json::Value> = None;
        let mut param_order: Option<Vec<String>> = None;
        let mut hidden_params = Vec::new();
        let mut example_output = None;
        let mut category: Option<String> = None;
        let mut alias = Vec::new();
        let mut allow = Vec::new();
        let mut cache: Option<String> = None;
        let mut rate_limit: Option<String> = None;
        let mut param_validations: Vec<(String, ParamToolAttrs)> = Vec::new();
        let mut baked_examples: Vec<BakedExample> = Vec::new();
        let mut min_desc_score: Option<u8> = None;
        let mut allow_short_desc = false;
        let mut allow_insecure_desc = false;
        let mut desc_blocklist: Vec<String> = Vec::new();
        let mut result_truncate_bytes: Option<usize> = None;

        while !input.is_empty() {
            let key: Ident = input.parse()?;

            match key.to_string().as_str() {
                "skip" => {
                    // T-013: `#[tool(skip, replaced_by = "...", ...)]`
                    // now shares the rest of the attribute body with
                    // the full parser, so companion keys (notably
                    // `replaced_by` and `version`) survive.
                    should_skip = true;
                    if input.peek(token::Comma) {
                        let _ = input.parse::<token::Comma>()?;
                    }
                }
                "deprecated" => {
                    if input.peek(token::Eq) {
                        input.parse::<token::Eq>()?;
                        if let Ok(lit_bool) = input.parse::<syn::LitBool>() {
                            deprecated = lit_bool.value;
                        } else {
                            deprecated = true;
                        }
                    } else {
                        deprecated = true;
                    }
                    if input.peek(token::Comma) {
                        let _ = input.parse::<token::Comma>();
                    }
                }
                "replaced_by" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    replaced_by = Some(value.value());
                }
                "deprecated_note" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    deprecated_note = Some(value.value());
                }
                "deprecated_since" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    deprecated_since = Some(value.value());
                }
                "remove_in" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    remove_in = Some(value.value());
                }
                "version" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    version = Some(value.value());
                }
                // T-020: schema-evolution interval bounds. Both
                // accept any string by default so CalVer / commit
                // SHA work out of the box; the macro emits a
                // `compile_error!` recommending
                // `version_policy = "semver"` when the impl opted
                // into semver ordering and a literal fails to
                // parse as SemVer.
                "since" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    since = Some(value.value());
                }
                "until" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    until = Some(value.value());
                }
                "group" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    group = Some(value.value());
                }
                "visible" => {
                    input.parse::<token::Eq>()?;
                    if let Ok(ident) = input.parse::<Ident>() {
                        visible = ident != "false";
                    } else if input.peek(LitStr) {
                        let value: LitStr = input.parse()?;
                        visible = value.value().to_lowercase() != "false";
                    } else if let Ok(Lit::Bool(lit_bool)) = input.parse::<Lit>() {
                        visible = lit_bool.value;
                    }
                }
                "tags" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let tag: LitStr = content.parse()?;
                        tags.push(tag.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                }
                "return_description" | "returns" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    return_description = Some(value.value());
                }
                "context" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    context = Some(value.value());
                }
                "example_input" => {
                    input.parse::<token::Eq>()?;
                    example_input = parse_json_value(input)?;
                }
                // T-016: `example = call!(self.method(args) => result)` —
                // singular baked-example form. Reads a single token
                // tree (which will be a `call!(...)` macro invocation)
                // as raw tokens and stores it for downstream codegen.
                "baked_example" | "bake_example" | "example_call" => {
                    input.parse::<token::Eq>()?;
                    let value = consume_single_value(input)?;
                    baked_examples.push(parse_example_singular(value)?);
                }
                // T-016: `examples = [call!(...), call!(...)]` — plural
                // form. Reads a bracketed array of `call!(...)` literals.
                "baked_examples" | "bake_examples" | "examples_call" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    let mut elements: Vec<TokenStream> = Vec::new();
                    while !content.is_empty() {
                        elements.push(consume_single_value(&content)?);
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                    baked_examples.extend(parse_examples_array(elements)?);
                }
                // T-016: prefer `example = call!(...)` over the old
                // `example_input = <json>` shape when the value
                // starts with the `call` ident. The parser peeks
                // the first token; if it sees `call`, it routes to
                // the baked-example parser; otherwise it falls
                // back to the JSON-literal path so the existing
                // `example_input` shape is unaffected.
                "example" => {
                    input.parse::<token::Eq>()?;
                    let raw = consume_single_value(input)?;
                    if crate::tool::example::looks_like_call_macro(&raw) {
                        baked_examples.push(parse_example_singular(raw)?);
                    } else {
                        // Re-parse the captured tokens as a JSON
                        // value so existing `example_input = {...}`
                        // users continue to work.
                        let lit: LitStr = syn::parse2(raw).map_err(|_| {
                            syn::Error::new(
                                proc_macro2::Span::call_site(),
                                "tokitai `example = ...` must be either a JSON literal or a `call!(...)` invocation",
                            )
                        })?;
                        example_input = Some(serde_json::from_str(&lit.value()).map_err(|e| {
                            syn::Error::new(
                                proc_macro2::Span::call_site(),
                                format!("invalid JSON for `example`: {}", e),
                            )
                        })?);
                    }
                }
                "examples" => {
                    input.parse::<token::Eq>()?;
                    let raw = consume_single_value(input)?;
                    // Detect bracketed array of `call!(...)` literals
                    // vs. bare JSON. The bracketed form is the
                    // canonical `examples = [...]` for baked
                    // examples; the bare form is reserved for
                    // future use (none today).
                    let first_token_kind = raw.clone().into_iter().next().map(|t| t.to_string());
                    if first_token_kind.as_deref() == Some("[")
                        || raw.to_string().trim_start().starts_with('[')
                    {
                        // Unwrap the bracketed group so we can
                        // walk its inner tokens and split on
                        // top-level commas.
                        let mut elements: Vec<TokenStream> = Vec::new();
                        let inner_stream: TokenStream =
                            if let Some(proc_macro2::TokenTree::Group(g)) =
                                raw.clone().into_iter().next()
                            {
                                if g.delimiter() == proc_macro2::Delimiter::Bracket {
                                    g.stream()
                                } else {
                                    raw.clone()
                                }
                            } else {
                                raw.clone()
                            };
                        let inner = inner_stream.into_iter();
                        let mut current = TokenStream::new();
                        let mut depth = 0i32;
                        for tok in inner {
                            let s = tok.to_string();
                            if s == "]" && depth == 0 {
                                if !current.is_empty() {
                                    elements.push(current.clone());
                                    current = TokenStream::new();
                                }
                                break;
                            }
                            if s == "," && depth == 0 {
                                if !current.is_empty() {
                                    elements.push(current.clone());
                                    current = TokenStream::new();
                                }
                                continue;
                            }
                            if s == "[" {
                                depth += 1;
                            } else if s == "]" {
                                depth -= 1;
                            }
                            current.extend(std::iter::once(tok));
                        }
                        if !current.is_empty() {
                            elements.push(current);
                        }
                        baked_examples.extend(parse_examples_array(elements)?);
                    } else {
                        return Err(syn::Error::new_spanned(
                            raw,
                            "tokitai `examples = ...` must be a bracketed array of `call!(...)` invocations (e.g. `examples = [call!(self.f(1) => 2)]`)",
                        ));
                    }
                }
                "param_order" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    let mut order = Vec::new();
                    while !content.is_empty() {
                        let name: LitStr = content.parse()?;
                        order.push(name.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                    param_order = Some(order);
                }
                "hidden_params" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let name: LitStr = content.parse()?;
                        hidden_params.push(name.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                }
                "example_output" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    example_output = Some(value.value());
                }
                "category" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    category = Some(value.value());
                }
                "alias" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let alias_name: LitStr = content.parse()?;
                        alias.push(alias_name.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                }
                "allow" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let warning: LitStr = content.parse()?;
                        allow.push(warning.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                }
                "cache" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    cache = Some(value.value());
                }
                "rate_limit" => {
                    input.parse::<token::Eq>()?;
                    let value: LitStr = input.parse()?;
                    rate_limit = Some(value.value());
                }
                // T-018: per-method score threshold override and
                // bare opt-out flag. Mirrors the impl-level parser
                // in `ToolAttributes::parse` so the two shapes are
                // symmetrical.
                "min_desc_score" => {
                    input.parse::<token::Eq>()?;
                    if let Ok(lit_int) = input.parse::<syn::LitInt>() {
                        min_desc_score = lit_int.base10_parse::<u8>().ok();
                    } else if let Ok(lit_str) = input.parse::<LitStr>() {
                        min_desc_score = lit_str.value().parse().ok();
                    }
                }
                "allow_short_desc" => {
                    allow_short_desc = true;
                    if input.peek(token::Comma) {
                        let _ = input.parse::<token::Comma>();
                    }
                }
                // T-022: per-method opt-out from the adversarial
                // description lint. Bare flag, no `=` form.
                "allow_insecure_desc" => {
                    allow_insecure_desc = true;
                    if input.peek(token::Comma) {
                        let _ = input.parse::<token::Comma>();
                    }
                }
                // T-022: per-method extension of the bad-pattern
                // set. Each entry is a case-insensitive substring;
                // a hit raises the safety lint for this method
                // only. The shape mirrors `alias = [...]` /
                // `tags = [...]` for symmetry.
                "desc_blocklist" => {
                    input.parse::<token::Eq>()?;
                    let content;
                    syn::bracketed!(content in input);
                    while !content.is_empty() {
                        let phrase: LitStr = content.parse()?;
                        desc_blocklist.push(phrase.value());
                        if content.peek(token::Comma) {
                            content.parse::<token::Comma>()?;
                        }
                    }
                }
                // T-019: per-method byte budget for the
                // serialized result. Accepts an integer literal
                // (the canonical form) or a string literal
                // (mirrors `min_desc_score` above). `0` is a
                // compile error and is validated in
                // `generate_for_impl` because the parser path
                // cannot propagate `syn::Error` to the user
                // (the `attr.parse_args::<MethodToolAttrs>()`
                // call site in `extract_tool_info` silently
                // swallows parse errors and falls through to
                // the default attribute values, so a `0` value
                // would compile without a diagnostic). The
                // post-parse validation emits a `compile_error!`
                // at the offending literal's span. Negative
                // values are rejected by the unsigned `usize`
                // parse path.
                "result_truncate_bytes" => {
                    input.parse::<token::Eq>()?;
                    let parsed: Option<usize> = if let Ok(lit_int) = input.parse::<syn::LitInt>() {
                        lit_int.base10_parse::<usize>().ok()
                    } else if let Ok(lit_str) = input.parse::<LitStr>() {
                        lit_str.value().parse().ok()
                    } else {
                        None
                    };
                    match parsed {
                        Some(0) => result_truncate_bytes = Some(0),
                        Some(n) => result_truncate_bytes = Some(n),
                        None => {
                            return Err(syn::Error::new(
                                key.span(),
                                "tokitai `result_truncate_bytes` must be a positive integer literal \
                                 (e.g. `result_truncate_bytes = 4096`)",
                            ));
                        }
                    }
                }
                _ => {
                    let key_str = key.to_string();
                    let validation_prefixes = [
                        "enum_values_",
                        "min_length_",
                        "max_length_",
                        "min_items_",
                        "max_items_",
                        "multiple_of_",
                        "validate_",
                        "validate_msg_",
                        "default_",
                        "example_",
                        "one_of_",
                        "pattern_",
                        "min_",
                        "max_",
                    ];

                    let is_validation_attr = validation_prefixes
                        .iter()
                        .any(|prefix| key_str.starts_with(prefix));

                    if is_validation_attr {
                        for prefix in &validation_prefixes {
                            if key_str.starts_with(prefix) {
                                let param_name = key_str.strip_prefix(prefix).unwrap();
                                let existing_idx =
                                    param_validations.iter().position(|(n, _)| n == param_name);
                                let mut param_attrs = if let Some(idx) = existing_idx {
                                    param_validations.remove(idx).1
                                } else {
                                    ParamToolAttrs::default()
                                };

                                input.parse::<token::Eq>()?;

                                match *prefix {
                                    "one_of_" => {
                                        let content;
                                        syn::bracketed!(content in input);
                                        let mut values = Vec::new();
                                        while !content.is_empty() {
                                            let val: LitStr = content.parse()?;
                                            values.push(val.value());
                                            if content.peek(token::Comma) {
                                                content.parse::<token::Comma>()?;
                                            }
                                        }
                                        param_attrs.one_of = Some(values.clone());
                                    }
                                    "enum_values_" => {
                                        let content;
                                        syn::bracketed!(content in input);
                                        let mut values = Vec::new();
                                        while !content.is_empty() {
                                            let val_expr: Expr = content.parse()?;
                                            let val_str = val_expr.to_token_stream().to_string();
                                            values.push(parse_value_string(&val_str));
                                            if content.peek(token::Comma) {
                                                content.parse::<token::Comma>()?;
                                            }
                                        }
                                        param_attrs.enum_values = Some(values.clone());
                                    }
                                    "pattern_" => {
                                        param_attrs.pattern = parse_lit_to_string(input)?;
                                    }
                                    "min_" => {
                                        param_attrs.min = parse_lit_to_f64(input)?;
                                    }
                                    "max_" => {
                                        param_attrs.max = parse_lit_to_f64(input)?;
                                    }
                                    "min_length_" => {
                                        param_attrs.min_length = parse_lit_to_usize(input)?;
                                    }
                                    "max_length_" => {
                                        param_attrs.max_length = parse_lit_to_usize(input)?;
                                    }
                                    "min_items_" => {
                                        param_attrs.min_items = parse_lit_to_usize(input)?;
                                    }
                                    "max_items_" => {
                                        param_attrs.max_items = parse_lit_to_usize(input)?;
                                    }
                                    "multiple_of_" => {
                                        param_attrs.multiple_of = parse_lit_to_f64(input)?;
                                    }
                                    "validate_" => {
                                        param_attrs.validate = parse_lit_to_string(input)?;
                                    }
                                    "validate_msg_" => {
                                        param_attrs.validate_msg = parse_lit_to_string(input)?;
                                    }
                                    "validate_msg_zh_" => {
                                        param_attrs.validate_msg_zh = parse_lit_to_string(input)?;
                                    }
                                    "validate_msg_en_" => {
                                        param_attrs.validate_msg_en = parse_lit_to_string(input)?;
                                    }
                                    "default_" => {
                                        param_attrs.default = parse_json_value(input)?;
                                    }
                                    "example_" => {
                                        param_attrs.example = parse_json_value(input)?;
                                    }
                                    _ => {}
                                }

                                param_validations.push((param_name.to_string(), param_attrs));
                                break;
                            }
                        }
                    } else {
                        input.parse::<token::Eq>()?;
                        let value: LitStr = input.parse()?;
                        match key.to_string().as_str() {
                            "name" => name = Some(value.value()),
                            "desc" | "description" => desc = Some(value.value()),
                            _ => {}
                        }
                    }
                }
            }

            if input.peek(token::Comma) {
                input.parse::<token::Comma>()?;
            }
        }

        if let Some(cat) = category {
            tags.push(cat);
        }

        Ok(MethodToolAttrs {
            name,
            desc,
            skip: should_skip,
            deprecated,
            replaced_by,
            deprecated_note,
            deprecated_since,
            remove_in,
            version,
            since,
            until,
            visible,
            tags,
            group,
            return_description,
            context,
            example_input,
            param_order,
            hidden_params,
            example_output,
            alias,
            allow,
            cache,
            rate_limit,
            param_validations,
            baked_examples,
            min_desc_score,
            allow_short_desc,
            allow_insecure_desc,
            desc_blocklist,
            result_truncate_bytes,
        })
    }
}

/// T-016: consume a single attribute value (anything between an `=`
/// and the next top-level comma / end-of-input). The returned
/// token stream captures the full syntactic shape of the value —
/// in particular, a `call!(...)` invocation comes through as
/// `call ! ( ... )` because we don't expand the macro ourselves;
/// the downstream example parser detects the `call` ident and
/// parses the body.
///
/// Syn's `ParseStream` doesn't expose a "consume one value" method
/// out of the box, so we read tokens one at a time and stop at a
/// top-level comma. Parenthesis / bracket nesting is handled by
/// `proc_macro2::Group` — when we see an opening group we read
/// the whole group (including its balanced children) in one
/// step.
fn consume_single_value(input: ParseStream) -> syn::Result<TokenStream> {
    let mut out = TokenStream::new();
    loop {
        // Stop at end-of-input or a top-level comma.
        if input.is_empty() {
            break;
        }
        if input.peek(token::Comma) {
            break;
        }
        // Consume one token tree (ident, punct, literal, or a
        // balanced group — the `Group` variant carries its own
        // nested token stream, so commas inside a group are
        // opaque to us and don't terminate the value).
        let next: proc_macro2::TokenTree = input.parse()?;
        out.extend(std::iter::once(next));
    }
    Ok(out)
}
