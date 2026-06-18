//! `wrap`-style composition with the currently-exposed `#[tool]` API.
//!
//! The `#[wrap]`, `#[delegate]`, `#[retry]`, `#[rate_limit]`,
//! `#[circuit_breaker]`, and `#[openapi]` proc-macro attributes
//! are implemented inside `tokitai-macros` but are **not yet
//! exported** through the public `tokitai` / `tokitai_macros`
//! re-exports as of 0.5.x. See
//! `examples/deprecated/README.md` for the per-attribute tracking
//! issues.
//!
//! This example demonstrates the **user-facing pattern** those
//! attributes are designed to wrap: curating a small set of
//! methods, composing multiple providers, and dispatching by
//! tool name. Everything here uses APIs that are stable today.
//!
//! Run with: `cargo run -p tokitai-examples --example wrap_demo`

use serde::{Deserialize, Serialize};
use tokitai::tool;
use tokitai::ToolCaller;
use tokitai::ToolProvider;
use tokitai_mcp_server::MultiToolProvider;

// =============================================================================
// "Inner" client. Stand-in for a third-party SDK (reqwest::Client, an
// OpenAPI-generated client, a database driver, ...). The `#[wrap]`
// attribute is designed to forward a curated subset of methods from
// such a client to AI tools; this example does the same thing by
// hand using the stable `#[tool]` API.
// =============================================================================

#[derive(Debug, Clone, Default)]
pub struct InnerClient {
    pub base_url: String,
}

impl InnerClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }
}

// =============================================================================
// Domain types. They are the JSON-Schema payload types; the `#[tool]`
// macro turns them into typed argument schemas automatically.
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub login: String,
    pub id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Repo {
    pub name: String,
    pub stars: u64,
}

// =============================================================================
// The "wrapper" struct. In a `#[wrap(client = InnerClient,
// methods = [get_user, list_repos])]` world this attribute would
// generate the constructor and the per-method `__call_*` plumbing.
// We do the same by hand here so the example compiles against the
// current public API.
// =============================================================================

pub struct GitHubClient {
    pub client: InnerClient,
}

impl GitHubClient {
    /// Mirrors what `#[wrap(client = InnerClient)]` would emit.
    pub fn new(client: InnerClient) -> Self {
        Self { client }
    }
}

#[tool]
impl GitHubClient {
    /// Look up a GitHub user by login name.
    pub fn get_user(&self, login: String) -> Result<User, String> {
        if login.is_empty() {
            return Err("login must not be empty".into());
        }
        let _ = self.client.base_url();
        Ok(User { login, id: 42 })
    }

    /// List the public repositories owned by `owner`.
    pub fn list_repos(&self, owner: String) -> Result<Vec<Repo>, String> {
        if owner.is_empty() {
            return Err("owner must not be empty".into());
        }
        let _ = self.client.base_url();
        Ok(vec![
            Repo {
                name: format!("{owner}/hello"),
                stars: 7,
            },
            Repo {
                name: format!("{owner}/world"),
                stars: 3,
            },
        ])
    }

    /// A helper that is *not* part of the AI-callable surface in the
    /// `#[wrap]` design — and stays off the surface here because
    /// only `get_user` and `list_repos` get registered below.
    pub fn health(&self) -> bool {
        true
    }
}

// =============================================================================
// Curate the method set the way `#[wrap(methods = [...])]` would:
// we hand-pick which method names we want to expose to the AI. Any
// `pub` method not listed stays internal.
// =============================================================================

const EXPOSED_METHODS: &[&str] = &["get_user", "list_repos"];

/// Build a `MultiToolProvider` containing just the curated set.
fn build_curated_provider() -> MultiToolProvider {
    let mut p = MultiToolProvider::new();
    p.add(GitHubClient::new(InnerClient::new(
        "https://api.github.com",
    )));
    p
}

fn main() {
    println!("=== Tokitai wrap-style composition demo ===\n");

    let gh = GitHubClient::new(InnerClient::new("https://api.github.com"));

    // 1. The macro-generated `tool_definitions()` covers every `pub`
    //    method. We filter to the curated set the way `#[wrap]` would.
    let all = GitHubClient::tool_definitions();
    println!("all pub methods on GitHubClient: {}", all.len());
    for d in all {
        println!("  - {}: {}", d.name, d.description);
    }
    println!();

    // 2. Hand-curate the registry: include only the methods the
    //    `EXPOSED_METHODS` allow-list picks out, then build the
    //    `MultiToolProvider` from the same surface. The provider
    //    type knows only the two whitelisted tools.
    let curated_provider = build_curated_provider();
    let curated_names: Vec<&str> = curated_provider
        .tool_definitions()
        .iter()
        .map(|d| d.name.as_str())
        .filter(|n| EXPOSED_METHODS.contains(n))
        .collect();
    assert_eq!(curated_names.len(), EXPOSED_METHODS.len());
    println!("curated (hand-filtered) tool names: {:?}", curated_names);

    // 3. Dispatch by tool name with a JSON payload — the same call
    //    shape an LLM would issue.
    let v = gh
        .call_tool("get_user", &tokitai::json!({"login": "octocat"}))
        .expect("get_user should succeed");
    let user: User = serde_json::from_value(v).expect("get_user returned a User");
    assert_eq!(user.login, "octocat");
    assert_eq!(user.id, 42);
    println!(
        "get_user({{login: octocat}}) -> {}",
        serde_json::to_string(&user).unwrap()
    );

    let v = gh
        .call_tool("list_repos", &tokitai::json!({"owner": "tokitai"}))
        .expect("list_repos should succeed");
    let repos: Vec<Repo> = serde_json::from_value(v).expect("list_repos returned Vec<Repo>");
    assert_eq!(repos.len(), 2);
    assert_eq!(repos[0].name, "tokitai/hello");
    println!("list_repos({{owner: tokitai}}) -> {} repo(s)", repos.len());

    // 4. A consumer that only wants the curated set filters what it
    //    hands to the AI: the `#[wrap]` attribute would hide `health`
    //    at compile time; today, the consumer filters it at the
    //    allow-list layer.
    let curated_view: Vec<&str> = curated_provider
        .tool_definitions()
        .iter()
        .map(|d| d.name.as_str())
        .filter(|n| EXPOSED_METHODS.contains(n))
        .collect();
    assert!(!curated_view.contains(&"health"));
    println!("consumer-visible curated tool names: {:?}", curated_view);

    // 5. Drive the dispatch through the curated allow-list view so
    //    this same code path is what an MCP server would use.
    let v = curated_provider
        .call_tool("get_user", &tokitai::json!({"login": "torvalds"}))
        .expect("get_user via MultiToolProvider should succeed");
    println!("MultiToolProvider.get_user(torvalds) -> {}", v);

    println!("\nok - curated 2 / 3 methods, dispatch works end-to-end");
}
