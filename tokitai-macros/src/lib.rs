//! # Tokitai Macros
//!
//! **Procedural macros for Tokitai - Zero-dependency macro for AI tool integration**
//!
//! This crate provides the `#[tool]` procedural macro that enables compile-time tool definitions
//! for AI/LLM tool calling systems. It generates all the boilerplate code needed to expose
//! your Rust functions as AI-callable tools.
//!
//! ## Key Features
//!
//! - **Zero Runtime Dependencies** - The macro itself has no runtime overhead
//! - **Compile-time Generation** - Tool definitions are generated at compile time
//! - **Type Safety** - Parameter validation happens at compile time
//! - **Automatic Discovery** - Mark an `impl` block and all `pub` methods become tools
//! - **Customizable** - Override tool names and descriptions via attributes
//! - **Vendor Neutral** - Works with any AI/LLM provider (Ollama, OpenAI, Anthropic, etc.)
//!
//! ## Quick Start
//!
//! Add to your `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! tokitai = "0.5"
//! ```
//!
//! Then use the `#[tool]` macro:
//!
//! ```rust,ignore
//! use tokitai::tool;
//!
//! pub struct Calculator;
//!
//! #[tool]
//! impl Calculator {
//!     /// Add two numbers together
//!     pub async fn add(&self, a: i32, b: i32) -> i32 {
//!         a + b
//!     }
//!
//!     /// Multiply two numbers
//!     pub async fn multiply(&self, a: i32, b: i32) -> i32 {
//!         a * b
//!     }
//! }
//!
//! // Usage
//! let calc = Calculator;
//!
//! // Get tool definitions (compile-time generated)
//! let tools = Calculator::tool_definitions();
//! println!("Number of tools: {}", tools.len());
//!
//! // Call a tool
//! let result = calc.call_tool("add", &serde_json::json!({"a": 10, "b": 20})).unwrap();
//! println!("Result: {}", result);  // 30
//! ```
//!
//! ## How It Works
//!
//! The `#[tool]` macro automatically:
//!
//! 1. Extracts doc comments as tool descriptions
//! 2. Generates JSON Schema for parameters from Rust types
//! 3. Generates a `__get_tool_definitions()` helper and implements
//!    `tokitai_core::ToolProvider::tool_definitions()`
//! 4. Implements `call_tool` dispatcher for runtime invocation
//! 5. Generates parameter parsing and validation code
//!
//! ## Customization
//!
//! You can customize tool names and descriptions using the attribute syntax:
//!
//! ```rust,ignore
//! #[tool]
//! impl MyTools {
//!     #[tool(name = "fetch_url", desc = "Fetch content from a URL")]
//!     pub async fn fetch(&self, url: String) -> String {
//!         // implementation
//!     }
//! }
//! ```
//!
//! ## Generated Code
//!
//! For each `#[tool]` impl block, the macro generates:
//!
//! ```rust,ignore
//! // 1. Tool definitions helper + `ToolProvider` trait impl
//! impl Calculator {
//!     fn __get_tool_definitions() -> &'static [ToolDefinition] {
//!         static TOOLS: ::std::sync::LazyLock<::std::vec::Vec<ToolDefinition>> =
//!             ::std::sync::LazyLock::new(|| vec![
//!                 ToolDefinition {
//!                     name: "add",
//!                     description: "Add two numbers together",
//!                     input_schema: "{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"integer\"},\"b\":{\"type\":\"integer\"}},\"required\":[\"a\",\"b\"]}",
//!                 },
//!                 // ... more tools
//!             ]);
//!         &TOOLS
//!     }
//! }
//! impl ToolProvider for Calculator {
//!     fn tool_definitions() -> &'static [ToolDefinition] {
//!         Self::__get_tool_definitions()
//!     }
//! }
//!
//! // 2. call_tool dispatcher
//! impl Calculator {
//!     pub fn call_tool(&self, name: &str, args: &Value) -> Result<Value, ToolError> {
//!         match name {
//!             "add" => self.__call_add(args),
//!             "multiply" => self.__call_multiply(args),
//!             _ => Err(ToolError::not_found(format!("Unknown tool: {}", name))),
//!         }
//!     }
//! }
//! ```
//!
//! ## Type Mapping
//!
//! Rust types are automatically mapped to JSON Schema types with full recursive support:
//!
//! ### Basic Types
//!
//! | Rust Type | JSON Schema Type |
//! |-----------|------------------|
//! | `String`, `str` | `string` |
//! | `i8`, `i16`, `i32`, `i64`, `i128`, `u8`, `u16`, `u32`, `u64`, `u128`, `usize`, `isize` | `integer` |
//! | `f32`, `f64` | `number` |
//! | `bool` | `boolean` |
//!
//! ### Compound Types
//!
//! | Rust Type | JSON Schema Type |
//! |-----------|------------------|
//! | `Vec<T>` | `array` with `items` schema |
//! | `[T; N]` | `array` with `items` schema |
//! | `&[T]` | `array` with `items` schema |
//! | `HashMap<K, V>` | `object` with `additionalProperties` |
//! | `Option<T>` | `anyOf` with inner type and `null` |
//! | `(T, U, ...)` | `array` (tuple representation) |
//!
//! ### Third-party Types
//!
//! | Rust Type | JSON Schema Type |
//! |-----------|------------------|
//! | `chrono::DateTime<Utc>` | `string` with `format: date-time` |
//! | `chrono::NaiveDateTime` | `string` with `format: date-time` |
//! | `uuid::Uuid` | `string` with `format: uuid` |
//! | `url::Url` | `string` with `format: uri` |
//! | `PathBuf`, `Path` | `string` with `format: file-path` |
//!
//! ### Custom Types
//!
//! Custom structs are represented as `object` types. For full field-level schema generation,
//! ensure your struct derives `serde::Deserialize` and the macro will handle it at runtime.
//!
//! ## Requirements
//!
//! - **Rust Version**: 1.70+
//! - **Edition**: 2021
//!
//! ## License
//!
//! Licensed under either of:
//!
//! - Apache License, Version 2.0 ([LICENSE-APACHE](https://github.com/silverenternal/tokitai/blob/main/LICENSE))
//! - MIT License ([LICENSE-MIT](https://github.com/silverenternal/tokitai/blob/main/LICENSE))
//!
//! at your option.
//!
//! ## Contributing
//!
//! Unless you explicitly state otherwise, any contribution intentionally submitted
//! for inclusion in this crate by you, as defined in the Apache-2.0 license, shall be
//! dual licensed as above, without any additional terms or conditions.
//!
//! ## See Also
//!
//! - [`tokitai`](https://crates.io/crates/tokitai) - Main crate with runtime support
//! - [`tokitai-core`](https://crates.io/crates/tokitai-core) - Core types and traits

mod compose;
mod description;
mod error;
mod tool;

use proc_macro::TokenStream;

// ---------------------------------------------------------------------------
// T-004: thin proc-macro wrappers for the resilience decorator macros
// (`#[retry]`, `#[rate_limit]`, `#[circuit_breaker]`). The actual
// codegen lives in `crate::tool::resilience::*::expand`; this file
// only exposes them as `#[proc_macro_attribute]` so they are reachable
// via `tokitai_macros::retry` / `::rate_limit` / `::circuit_breaker`.
// ---------------------------------------------------------------------------

/// # `#[tool]` Attribute Macro
///
/// Marks an `impl` block or individual methods as AI-callable tools.
///
/// ## Usage
///
/// ### 1. Mark an impl block (Recommended)
///
/// When placed on an `impl` block, all `pub` methods are automatically registered as tools:
///
/// ```rust,ignore
/// pub struct Calculator;
///
/// #[tool]
/// impl Calculator {
///     /// Add two numbers together
///     pub async fn add(&self, a: i32, b: i32) -> i32 {
///         a + b
///     }
/// }
/// ```
///
/// ### 2. Mark individual methods
///
/// Use `#[tool(...)]` on specific methods to customize tool properties:
///
/// ```rust,ignore
/// #[tool]
/// impl Calculator {
///     #[tool(name = "add_numbers", desc = "Add two numbers together")]
///     pub async fn add(&self, a: i32, b: i32) -> i32 {
///         a + b
///     }
///
///     /// This method won't be registered as a tool
///     fn helper(&self) {}
/// }
/// ```
///
/// ### 3. Mark individual parameters
///
/// Use `#[tool_attr(...)]` on specific parameters to customize parameter properties:
///
/// ```rust,ignore
/// #[tool]
/// impl Calculator {
///     pub async fn add(
///         &self,
///         #[tool_attr(required)] a: Option<i32>,  // Option but required
///         #[tool_attr(default = "0")] b: Option<i32>,  // Has default value
///     ) -> i32 {
///         a.unwrap_or(0) + b.unwrap_or(0)
///     }
/// }
/// ```
///
/// ## Generated Code
///
/// The macro generates:
///
/// 1. `fn __get_tool_definitions() -> &'static [ToolDefinition]` - lazy-initialized tool definitions
/// 2. `impl ToolProvider` - exposes `tool_definitions()` to the runtime
/// 3. `fn call_tool(&self, name: &str, args: &Value) -> Result<Value, ToolError>` - Tool dispatcher
/// 4. Wrapper functions for each tool with JSON parameter parsing
///
/// ## Features
///
/// -Zero runtime dependencies (macro itself)
/// -Compile-time tool definition generation
/// -Automatic description extraction from doc comments
/// -Custom tool names and descriptions support
/// -Type-safe parameter parsing
/// -Recursive type resolution for complex types
/// -JSON Schema generation with proper formatting
#[proc_macro_attribute]
pub fn tool(attr: TokenStream, item: TokenStream) -> TokenStream {
    tool::tool(attr, item)
}

/// # `#[tool_type]` Attribute Macro
///
/// Registers a custom type with a manually defined JSON schema.
///
/// Use this macro when you have custom struct types that the `#[tool]` macro cannot automatically parse.
/// This allows you to provide explicit schema information for AI tool calling.
///
/// ## Usage
///
/// ```rust,ignore
/// use tokitai::tool_type;
///
/// #[tool_type(
///     name = "Location",
///     properties = "latitude: number, longitude: number",
///     required = "latitude, longitude"
/// )]
/// pub struct Location {
///     pub latitude: f64,
///     pub longitude: f64,
/// }
/// ```
///
/// ## Attributes
///
/// - `name`: The type name (required)
/// - `properties`: Comma-separated list of `field_name: type` pairs
///   - Supported types: `string`, `integer`, `number`, `boolean`, `array`, `object`
/// - `required`: Comma-separated list of required field names
///
/// ## Generated Schema
///
/// The above example generates:
///
/// ```json
/// {
///   "type": "object",
///   "properties": {
///     "latitude": { "type": "number" },
///     "longitude": { "type": "number" }
///   },
///   "required": ["latitude", "longitude"]
/// }
/// ```
#[proc_macro_attribute]
pub fn tool_type(attr: TokenStream, item: TokenStream) -> TokenStream {
    tool::tool_type(attr, item)
}

/// Parameter validation attribute (used internally by #[tool] macro)
///
/// This attribute is automatically processed by the `#[tool]` macro.
/// It should not be used directly by users - instead use the `#[tool(...)]` syntax on parameters.
///
/// ## Example (internal usage)
///
/// ```text
/// #[tool]
/// impl MyTools {
///     pub fn create_user(
///         &self,
///         #[tool_validate = "!value.is_empty()")]
///         name: String,
///     ) -> Result<String, Error> {
///         // ...
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn tool_validate(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // This is a no-op attribute, processed by #[tool] macro
    item
}

/// Parameter transformation attribute (used internally by #[tool] macro)
///
/// This attribute is automatically processed by the `#[tool]` macro.
/// It should not be used directly by users - instead use the `#[tool(...)]` syntax on parameters.
///
/// ## Example (internal usage)
///
/// ```text
/// #[tool]
/// impl MyTools {
///     pub fn create_user(
///         &self,
///         #[tool_transform = "value.to_lowercase()")]
///         email: String,
///     ) -> Result<String, Error> {
///         // ...
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn tool_transform(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // This is a no-op attribute, processed by #[tool] macro
    item
}

/// Parameter description attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_desc(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter example attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_example(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter default attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_default(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter required attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_required(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter min attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_min(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter max attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_max(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter min_length attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_min_length(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter max_length attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_max_length(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter pattern attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_pattern(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter min_items attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_min_items(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter max_items attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_max_items(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter multiple_of attribute (used internally by #[tool] macro)
#[proc_macro_attribute]
pub fn tool_multiple_of(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

/// Parameter-level tool attributes (helper macro for #[tool])
///
/// This attribute is used to add validation, transformation, and other metadata
/// to individual parameters. It is automatically processed by the `#[tool]` macro.
///
/// ## Usage
///
/// ```rust,ignore
/// #[tool]
/// impl MyTools {
///     pub fn create_user(
///         &self,
///         #[param_tool(validate = "!value.is_empty()", desc = "Name cannot be empty")]
///         name: String,
///         #[param_tool(transform = "value.to_lowercase()")]
///         email: String,
///         #[param_tool(default = 10)]
///         count: i32,
///     ) -> Result<String, Error> {
///         // ...
///     }
/// }
/// ```
///
/// ## Supported Attributes
///
/// - `validate = "expression"` - Validation expression (use `value` to refer to the parameter)
/// - `transform = "expression"` - Transformation expression (use `value` to refer to the parameter)
/// - `desc = "description"` - Parameter description
/// - `default = value` - Default value (supports literals: integers, floats, booleans, strings, arrays, objects)
/// - `example = value` - Example value
/// - `required` - Mark parameter as required (even if Option type)
#[proc_macro_attribute]
pub fn param_tool(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // This is a no-op attribute, processed by #[tool] macro
    item
}

// (T-016: `call!` macro lives in `tokitai`, not here, because
// `proc-macro` crates cannot export `macro_rules!`. The `#[tool]`
// attribute parser sees the pre-expansion tokens so it does not
// depend on `call!` being defined for its parsing logic.)

/// # `#[compose]` Attribute Macro
///
/// T-017: declarative multi-step tool composition. Collapses a
/// sequence of named `pub` methods into a SINGLE tool the LLM
/// calls once. The macro generates a synthetic public method on
/// the impl block whose body threads the LLM's arguments through
/// the named sub-methods in order.
///
/// ## Usage
///
/// ```rust,ignore
/// use tokitai::tool;
///
/// pub struct TripPlanner;
///
/// #[tool]
/// #[compose(name = "book_trip", steps = [search_flights, filter_by_price, book_flight, send_email])]
/// impl TripPlanner {
///     pub fn search_flights(&self, origin: String, dest: String) -> Vec<Flight> { ... }
///     pub fn filter_by_price(&self, flights: Vec<Flight>, max: f64) -> Vec<Flight> { ... }
///     pub fn book_flight(&self, flights: Vec<Flight>) -> BookingConfirmation { ... }
///     pub fn send_email(&self, confirmation: BookingConfirmation) -> String { ... }
/// }
/// ```
///
/// The LLM sees ONE tool (`book_trip`) with input schema
/// `{ origin, dest, max }` (the first step's parameters) and
/// output schema matching `send_email`'s return type. The
/// runtime threads `origin, dest` through `search_flights`,
/// feeds its output to `filter_by_price` along with `max`,
/// and so on.
///
/// ## Compile-time checks
///
/// - Every named step method exists on the same `impl` block.
/// - The chain of types connects: step N's return type matches
///   step N+1's first non-`self` argument type.
/// - The composition is acyclic (no step appears twice).
///
/// Diagnostics are anchored at the offending step's span
/// (T-001) so editors jump to the right place.
#[proc_macro_attribute]
pub fn compose(attr: TokenStream, item: TokenStream) -> TokenStream {
    crate::compose::expand(attr, item)
}

/// # `tokitai!` Configuration Macro
///
/// Used to centrally configure tool properties without modifying original code.
///
/// ## Usage
///
/// ```rust,ignore
/// tokitai::config! {
///     MyService {
///         get_user: {
///             desc: "Fetch user info",
///             tags: ["user", "read"],
///             params: {
///                 id: {
///                     desc: "User unique identifier",
///                     example: "1001"
///                 }
///             }
///         }
///     }
/// }
/// ```
///
/// ## Features
///
/// - Override method descriptions
/// - Add tags to methods
/// - Configure parameter-level descriptions and examples
/// - Works with existing `#[tool]` annotated code
///
/// ## Priority
///
/// The priority table is **frozen** as of T-002 and lives in
/// `tokitai_core::config::CONFIG_PRIORITY_ORDER` so the macro and the
/// user-facing docs cannot drift.
///
/// 1. `#[tool(desc = "...")]` (compile-time, attribute-supplied) — wins on conflict
/// 2. doc comment (compile-time, `///` lines above the method)
/// 3. `tokitai!` config block (runtime; does NOT override an explicit `#[tool(desc)]`)
/// 4. synthesized default (e.g. `"调用 <method> 方法"`) — last-resort fallback
///
/// Parameter-level: `#[param_tool(desc = "...")]` (compile-time) >
/// runtime `tokitai!` per-parameter `desc:`.
#[proc_macro]
pub fn config(item: TokenStream) -> TokenStream {
    tool::config(item)
}

// ---------------------------------------------------------------------------
// T-004: resilience decorator proc-macro attributes.
//
// These three attributes wrap the body of a (sync or `async`) function
// in retry, rate-limit, or circuit-breaker logic. The codegen lives in
// `crate::tool::resilience::*::expand` and is wired through these thin
// `#[proc_macro_attribute]` wrappers so they are reachable from
// downstream code as `tokitai_macros::retry`, etc.
//
// On `async fn`, the generated sleeps are driven by the registered
// `AsyncExecutor` (T-003) via `tokitai_core::async_sleep(...)`, so the
// runtime worker thread is never blocked between attempts. On sync
// functions the sleeps fall back to `std::thread::sleep` (acceptable
// because the caller is already a sync caller).
//
// See `docs/wrap-architecture.md` §4.4 and §6 for the design overview
// and `docs/reference/{retry,rate-limit,circuit-breaker}.md` for the
// per-attribute reference.
// ---------------------------------------------------------------------------

/// `#[retry(max = N, backoff = "...", jitter = bool, on = "...")]`
/// decorator. Wraps a `Result`-returning function body in a retry loop
/// with backoff between attempts. See
/// [`tokitai_macros`](crate) module docs and `docs/reference/retry.md`.
#[proc_macro_attribute]
pub fn retry(attr: TokenStream, item: TokenStream) -> TokenStream {
    crate::tool::resilience::retry::expand(attr.into(), item.into()).into()
}

/// `#[rate_limit(rps = N, burst = N)]` decorator. Wraps a function
/// body in a token-bucket rate limiter. See
/// `docs/reference/rate-limit.md`.
#[proc_macro_attribute]
pub fn rate_limit(attr: TokenStream, item: TokenStream) -> TokenStream {
    crate::tool::resilience::rate_limit::expand(attr.into(), item.into()).into()
}

/// `#[circuit_breaker(failure_threshold = N, reset_timeout = "30s")]`
/// decorator. Wraps a function body in a 3-state (closed / open /
/// half-open) circuit breaker. See
/// `docs/reference/circuit-breaker.md`.
#[proc_macro_attribute]
pub fn circuit_breaker(attr: TokenStream, item: TokenStream) -> TokenStream {
    crate::tool::resilience::circuit_breaker::expand(attr.into(), item.into()).into()
}

// ---------------------------------------------------------------------------
// Hidden compile-time hooks used by `tokitai-macros/tests/property_based_test.rs`.
//
// These are NOT part of the public API and are not re-exported by `tokitai`.
// They exist solely to bridge proptest (a runtime testing framework) with
// the `#[tool]` macro (which only runs at compile time):
//
// * `__property_expand` runs the `tool` proc-macro on its `item`
//   argument and emits a `&'static str` literal containing the
//   rendered expansion. The runtime test compares that string
//   against a hand-curated snapshot.
//
// * `__property_would_error` runs the same pipeline and emits a
//   `bool` literal — `true` if the expansion contains a
//   `compile_error!` invocation, `false` otherwise.
//
// The tests file is at
// `tokitai-macros/tests/property_based_test.rs`; see the
// "Property-based testing" doc under `docs/internal/` for
// the rationale and design.
// ---------------------------------------------------------------------------
#[doc(hidden)]
#[proc_macro]
pub fn __property_expand(item: TokenStream) -> TokenStream {
    use proc_macro2::Literal;
    use proc_macro2::TokenStream as TokenStream2;

    let result = tool::tool(TokenStream::new(), item);
    let result_ts2: TokenStream2 = result.into();
    let rendered = result_ts2.to_string();
    // Use a `Literal::string` so the macro expansion is a
    // syntactically-valid Rust string literal no matter what
    // characters (quotes, backslashes, ...) appear in the
    // rendered expansion.
    let lit = Literal::string(&rendered);
    quote::quote! { #lit }.into()
}

#[doc(hidden)]
#[proc_macro]
pub fn __property_would_error(item: TokenStream) -> TokenStream {
    use proc_macro2::TokenStream as TokenStream2;

    let result = tool::tool(TokenStream::new(), item);
    let result_ts2: TokenStream2 = result.into();
    let rendered = result_ts2.to_string();
    let would_error = rendered.contains("compile_error !") || rendered.contains("compile_error!");
    let b = if would_error { "true" } else { "false" };
    let lit: TokenStream2 = b.parse().expect("bool literal parses");
    quote::quote! { #lit }.into()
}

// ---------------------------------------------------------------------------
// T-008: string-literal variants of the property-test hooks. The original
// `__property_expand!` / `__property_would_error!` accept a token stream,
// which makes it impossible to pass a proptest-generated `String` (a
// token stream is a fixed-size compile-time artifact). These two
// string-literal variants accept a `&'static str` containing source
// code, parse it as a `syn::File`, and run the same pipeline. They are
// used by `tests/property_based_test.rs` to exercise randomly-synthesized
// impl blocks and to perform AST-level structural assertions on the
// rendered expansion.
// ---------------------------------------------------------------------------
#[doc(hidden)]
#[proc_macro]
pub fn __property_expand_str(item: TokenStream) -> TokenStream {
    use proc_macro2::Literal;
    use proc_macro2::TokenStream as TokenStream2;
    use quote::ToTokens;

    let item2: TokenStream2 = item.into();
    let lit_str: syn::Result<syn::LitStr> = syn::parse2(item2);
    let src = match lit_str {
        Ok(ls) => ls.value(),
        Err(_) => return make_lit("NOT_A_STRING_LITERAL"),
    };

    // Parse the source as a `syn::File` (the same shape the
    // proc-macro driver sees). The `tool` proc-macro expects
    // a token stream that is itself a single `Item::Impl` or
    // that contains an `Item::Impl`; `syn::parse_str` on the
    // `ItemImpl` is the most direct path. If parsing fails we
    // return a sentinel that the structural tests can detect.
    let tokens: TokenStream2 = match syn::parse_str::<syn::ItemImpl>(&src) {
        Ok(item_impl) => item_impl.to_token_stream(),
        Err(_) => return make_lit("PARSE_ERROR"),
    };
    let result = tool::tool(TokenStream::new(), tokens.into());
    let result_ts2: TokenStream2 = result.into();
    let rendered = result_ts2.to_string();
    let lit = Literal::string(&rendered);
    quote::quote! { #lit }.into()
}

#[doc(hidden)]
#[proc_macro]
pub fn __property_would_error_str(item: TokenStream) -> TokenStream {
    use proc_macro2::TokenStream as TokenStream2;
    use quote::ToTokens;

    let item2: TokenStream2 = item.into();
    let lit_str: syn::Result<syn::LitStr> = syn::parse2(item2);
    let src = match lit_str {
        Ok(ls) => ls.value(),
        Err(_) => return make_lit("NOT_A_STRING_LITERAL"),
    };

    let tokens: TokenStream2 = match syn::parse_str::<syn::ItemImpl>(&src) {
        Ok(item_impl) => item_impl.to_token_stream(),
        Err(_) => {
            // Unparseable source counts as "would error" so the
            // structural test does not silently pass on garbage.
            let b: TokenStream2 = "true".parse().expect("bool literal parses");
            return quote::quote! { #b }.into();
        }
    };
    let result = tool::tool(TokenStream::new(), tokens.into());
    let result_ts2: TokenStream2 = result.into();
    let rendered = result_ts2.to_string();
    let would_error = rendered.contains("compile_error !") || rendered.contains("compile_error!");
    let b = if would_error { "true" } else { "false" };
    let lit: TokenStream2 = b.parse().expect("bool literal parses");
    quote::quote! { #lit }.into()
}

// ---------------------------------------------------------------------------
// T-001 span probe — hidden proc-macro used by
// `tokitai-macros/tests/error_span_test.rs`.
//
// Accepts a *string literal* of Rust source code (a struct +
// impl block pair) and runs the validation pipeline. Returns a
// `&'static str` containing one line per `MacroError`:
//
//     <code> | <rendered-compile_error>
//
// We accept a string literal (rather than a token stream) so
// the input parses deterministically as a `syn::File`; the
// proc-macro2 token stream path is ambiguous in test contexts
// and parses as a macro invocation instead of a top-level
// item.
// ---------------------------------------------------------------------------
#[doc(hidden)]
#[proc_macro]
pub fn __error_spans(item: TokenStream) -> TokenStream {
    use proc_macro2::TokenStream as TokenStream2;
    use quote::ToTokens;

    // Convert to proc_macro2 and parse the input as a single
    // `syn::LitStr`. `LitStr::value()` knows how to unescape the
    // literal's content (handles `\n`, `\\`, `\"`, `\u{...}`),
    // which the proc_macro::Literal::to_string() source form
    // does not.
    let item2: TokenStream2 = item.into();
    let lit_str: syn::Result<syn::LitStr> = syn::parse2(item2);
    let src = match lit_str {
        Ok(ls) => ls.value(),
        Err(_) => return make_lit("NOT_A_STRING_LITERAL"),
    };

    let parsed: syn::Result<syn::File> = syn::parse_str(&src);
    let report = match parsed {
        Ok(file) => {
            let mut s = String::new();
            let mut found = false;
            for item in &file.items {
                if let syn::Item::Impl(impl_item) = item {
                    found = true;
                    let errs = crate::tool::extract::validate::validate_impl(impl_item);
                    for err in &errs {
                        let mut ts2 = TokenStream2::new();
                        err.to_compile_error().to_tokens(&mut ts2);
                        let rendered_ts = ts2.to_string();
                        s.push_str(&format!("{} | {}\n", err.code(), rendered_ts));
                    }
                }
            }
            if !found {
                "NO_IMPL_FOUND".to_string()
            } else {
                s
            }
        }
        Err(e) => format!("PARSE_ERROR {}\n", e),
    };
    make_lit(&report)
}

#[doc(hidden)]
fn make_lit(s: &str) -> TokenStream {
    let lit = proc_macro2::Literal::string(s);
    quote::quote! { #lit }.into()
}

// Note: proc-macro crates may only export items tagged with `#[proc_macro]`,
// `#[proc_macro_derive]`, or `#[proc_macro_attribute]`. Runtime helpers like
// `tool_type_schema` / `tool_output_schema` belong in `tokitai-core`, not here.
