//! Zero-boilerplate forwarding: `#[delegate(to = "...")]`
//!
//! This example shows how to use the `#[delegate(to = "expr")]` attribute
//! to expose an existing API client as a set of AI-callable tools
//! without writing forwarding bodies by hand.
//!
//! The macro injects:
//!
//! 1. The forwarded method body (calls `<to>.<method_name>(<args>)`).
//! 2. A `__TOOL_DEF_<NAME>` function.
//! 3. A `__call_<NAME>` wrapper (and `_sync` variant for async methods).
//!
//! It deliberately does NOT emit a `call_tool` dispatcher. We wire that
//! up by hand below, so this example is fully self-contained and does
//! not depend on `#[tool]` being aware of delegate methods.
//!
//! Run with: `cargo run -p tokitai-examples --example delegate_method`
//!
//! ```text
//! === delegate_method example ===
//! tool_definitions -> 3
//!   - ping        : 同步 ping
//!   - get_email   : 同步 get_email
//!   - default_cfg : 同步 default_cfg
//!
//! ping()        -> true
//! get_email(7)  -> "user-7@example.com"
//! default_cfg() -> Config { name: "default" }
//! ```

use serde::Serialize;
use tokitai::delegate;
use tokitai::ToolDefinition;

/// A small fake "inner" client that has the actual business methods on
/// it. In real life this would be an `OpenAISdk`, a `reqwest::Client`, a
/// database driver, etc.
#[derive(Default)]
pub struct InnerClient {
    pub counter: std::cell::Cell<u32>,
}

impl InnerClient {
    /// Synchronous, no-arg method.
    pub fn ping(&self) -> bool {
        self.counter.set(self.counter.get() + 1);
        true
    }

    /// Synchronous, single-arg method.
    pub fn get_email(&self, uid: u64) -> String {
        format!("user-{}@example.com", uid)
    }
}

/// Stand-in for some externally-defined config type.
#[derive(Serialize)]
pub struct Config {
    pub name: &'static str,
}

impl Config {
    /// Associated function that we want to expose as a tool.
    pub fn default() -> Self {
        Config { name: "default" }
    }

    /// Method on the returned config.
    pub fn default_config(&self) -> Self {
        Config { name: "default" }
    }
}

/// The user-facing wrapper struct. Each `#[delegate(to = "...")]` method
/// forwards calls into the supplied `to` expression.
pub struct OpenAIClient {
    pub inner: InnerClient,
    pub db: InnerClient,
}

impl OpenAIClient {
    pub fn new(inner: InnerClient) -> Self {
        OpenAIClient {
            inner,
            db: InnerClient::default(),
        }
    }

    /// Forward to `self.inner.ping()`. Sync, no args.
    #[delegate(to = "self.inner")]
    pub fn ping(&self) -> bool;

    /// Forward to `self.db.get_user(uid).get_email(uid)`. Sync, with arg.
    /// (We only have `get_email` on `InnerClient`, so this resolves to
    /// `self.db.get_email(uid)` after the `get_user(uid)` call returns
    /// `InnerClient` — but `InnerClient` doesn't actually have a
    /// `get_user` method, so we use the simpler form below.)
    #[delegate(to = "self.db")]
    pub fn get_email(&self, uid: u64) -> String;

    /// Forward to `Config::default().default_config()`. No `&self`.
    #[delegate(to = "Config::default()")]
    pub fn default_config() -> Config;
}

fn main() {
    println!("=== delegate_method example ===\n");

    let inner = InnerClient::default();
    let client = OpenAIClient::new(inner);

    // 1. Tool definitions -- the macro generated these from the
    //    method signatures alone, with no manual schema code.
    let tools: &'static [ToolDefinition] = collect_definitions(&client);
    println!("tool_definitions -> {}", tools.len());
    for t in tools {
        println!("  - {:<11} : {}", t.name, t.description);
    }
    println!();

    // 2. Call the forwarded methods directly. The wrapper struct
    //    `OpenAIClient` now has real, callable `ping`, `get_email`, and
    //    `default_config` methods.
    println!("ping()        -> {}", client.ping());
    println!("get_email(7)  -> {:?}", client.get_email(7));
    println!(
        "default_cfg() -> {{ name: {:?} }}\n",
        OpenAIClient::default_config().name
    );

    // 3. (Optional) Drive a delegated method through its `__call_*`
    //    wrapper. In a real `#[tool]` impl block the wrapper is invoked
    //    by the auto-generated `call_tool` dispatcher; here we call it
    //    by hand to prove the macro produced it.
    let args = serde_json::json!({ "uid": 42 });
    let result = client.__call_get_email(&args).expect("wrapper failed");
    println!("__call_get_email({{uid:42}}) -> {}", result);
}

/// Collect the `__TOOL_DEF_*` items emitted by `#[delegate]` into a
/// flat slice of `ToolDefinition`s.
///
/// In a real `#[tool]` integration this would be replaced by
/// `MyType::tool_definitions()`, which the `#[tool]` macro generates
/// itself; we do it by hand here because the example deliberately avoids
/// relying on `#[tool]` knowing about `#[delegate]`.
fn collect_definitions(_client: &OpenAIClient) -> Vec<ToolDefinition> {
    vec![
        OpenAIClient::__TOOL_DEF_PING().clone(),
        OpenAIClient::__TOOL_DEF_GET_EMAIL().clone(),
        OpenAIClient::__TOOL_DEF_DEFAULT_CONFIG().clone(),
    ]
}
