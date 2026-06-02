//! Example: `#[openapi]`-driven `ToolProvider`
//!
//! **Deprecated / design sketch.** This example depends on the
//! `#[openapi]` and `#[openapi_op]` proc-macro attributes, which
//! are implemented in `tokitai-macros/src/tool/wrap_openapi/` but
//! are not yet exposed by `tokitai` / `tokitai_macros` in v0.5.0.
//! The plain `.rs` source is preserved here as a design sketch;
//! it will not compile against the current proc-macro crate. See
//! `examples/deprecated/README.md` for the full list of pending
//! attributes.
//!
//! The original intent of the example is documented below.
//!
//! ---
//!
//! This example shows how to drop an OpenAPI 3 spec next to your
//! `Cargo.toml` and turn a plain struct into a `ToolProvider` with
//! one attribute. The macro reads the spec at *proc-macro compile
//! time* — there is no spec parsing cost at runtime.
//!
//! ```text
//! .
//! ├── Cargo.toml
//! ├── openai_chat.json     # <-- include_str!()'d by the macro
//! └── src
//!     └── main.rs
//! ```
//!
//! In `src/main.rs`:
//!
//! ```rust,ignore
//! use tokitai::{openapi, openapi_op, ToolProvider};
//! use serde::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct ChatRequest { /* … fields … */ }
//!
//! #[derive(Serialize, Deserialize)]
//! struct ChatResponse { /* … fields … */ }
//!
//! #[derive(Default)]
//! struct OpenAIClient {
//!     http: reqwest::Client,
//!     api_key: String,
//! }
//!
//! #[openapi(
//!     spec = "openai_chat.json",
//!     base_url = "https://api.openai.com/v1",
//! )]
//! impl OpenAIClient {
//!     #[openapi_op(operation_id = "createChatCompletion")]
//!     pub async fn create_chat_completion(
//!         &self,
//!         body: ChatRequest,
//!     ) -> Result<ChatResponse, reqwest::Error> {
//!         self.http
//!             .post(format!("{}/chat/completions", self.base_url()))
//!             .bearer_auth(&self.api_key)
//!             .json(&body)
//!             .send()
//!             .await?
//!             .json()
//!             .await
//!     }
//!
//!     #[openapi_op(operation_id = "listModels")]
//!     pub async fn list_models(&self) -> Result<Vec<String>, reqwest::Error> {
//!         Ok(self
//!             .http
//!             .get(format!("{}/models", self.base_url()))
//!             .bearer_auth(&self.api_key)
//!             .send()
//!             .await?
//!             .json::<serde_json::Value>()
//!             .await?["data"]
//!             .as_array()
//!             .map(|arr| {
//!                 arr.iter()
//!                     .filter_map(|v| v.get("id").and_then(|i| i.as_str()).map(String::from))
//!                     .collect()
//!             })
//!             .unwrap_or_default())
//!     }
//! }
//!
//! impl OpenAIClient {
//!     fn base_url(&self) -> &str { "https://api.openai.com/v1" }
//! }
//!
//! fn main() {
//!     // The macro synthesised `ToolProvider` for us. No spec parsing
//!     // happens here — the lookup table is a `phf::Map` baked at
//!     // compile time.
//!     let defs = OpenAIClient::tool_definitions();
//!     assert!(!defs.is_empty());
//!     println!("registered {} tools:", defs.len());
//!     for d in defs {
//!         println!("  - {}: {}", d.name, d.description);
//!     }
//! }
//! ```
//!
//! What the macro generates, in addition to the per-method
//! `__TOOL_DEF_*` / `__call_*` plumbing:
//!
//! - `pub static __OPENAPI_OPS_OpenAIClient: phf::Map<&str, __OpenApiOp_OpenAIClient>`
//!   — keyed by `operationId`; use it to introspect the spec at runtime.
//! - `pub static __OPENAPI_SPEC_RAW: &str` — the raw spec text,
//!   available for `$ref` resolution or pretty-printing.
