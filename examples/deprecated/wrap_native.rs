//! Example: `#[wrap]` to expose a wrapped API client as AI tools.
//!
//! `#[wrap]` is a natural extension of `#[tool]`. Where `#[tool]`
//! registers every public method on the impl block, `#[wrap]` lets you
//! pre-select the methods you actually want to expose — ideal for
//! adapting a third-party API client (`reqwest::Client`,
//! `redis::Client`, an OpenAPI-generated client, etc.) into a
//! AI-callable surface.
//!
//! In this example the "inner client" is a tiny local type so the
//! example builds without external HTTP dependencies. The pattern
//! works identically for any other client type.
//!
//! Run with: `cargo run --example wrap_native` (once `wrap_native` is
//! registered in `examples/Cargo.toml`).

use serde::{Deserialize, Serialize};
use tokitai::wrap;
use tokitai::ToolProvider;

// ===== Inner "API client" =====================================================
//
// Stand-in for a real third-party client. Holds the bits of state a
// typical HTTP wrapper would (base URL, auth token, etc.). The
// `#[wrap]` macro does not require any specific shape — it just needs
// a type to thread through the generated constructor.

#[derive(Debug, Clone)]
pub struct InnerClient {
    base_url: String,
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

// ===== Domain types used in the tool schemas ==================================

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

// ===== The wrapped struct =====================================================

/// A minimal GitHub-style wrapper. The inner client is held in the
/// `client` field (the field name matches the `client = ...`
/// argument — pass `field = "..."` to override).
pub struct GitHubClient {
    pub client: InnerClient,
}

// ===== The `#[wrap]` impl =====================================================
//
// `methods = [get_user, list_repos]` pre-selects exactly two methods
// to expose as tools. The other public method, `health`, is NOT listed
// and therefore won't show up in `GitHubClient::tool_definitions()`.

#[wrap(client = InnerClient, methods = [get_user, list_repos])]
impl GitHubClient {
    /// Look up a GitHub user by login name.
    pub fn get_user(&self, login: String) -> Result<User, String> {
        if login.is_empty() {
            return Err("login must not be empty".into());
        }
        // Real implementation would do:
        //     self.client.get(&format!("/users/{login}"))
        // We just exercise the wrapper pattern.
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

    /// Not listed in `methods = [...]`, so it will NOT be a tool.
    pub fn health(&self) -> bool {
        true
    }
}

fn main() {
    // The macro generates `GitHubClient::new(client: InnerClient) -> Self`.
    let gh = GitHubClient::new(InnerClient::new("https://api.github.com"));
    assert_eq!(gh.client.base_url(), "https://api.github.com");

    // Only the two listed methods show up in the tool definitions.
    assert_eq!(GitHubClient::tool_count(), 2);
    let defs = GitHubClient::tool_definitions();
    let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
    assert!(names.contains(&"get_user"));
    assert!(names.contains(&"list_repos"));
    assert!(!names.contains(&"health"));

    // The macro also auto-implements `ToolCaller`, so we can dispatch
    // by tool name from a JSON payload — exactly as `#[tool]` allows.
    let v = gh
        .call_tool("get_user", &serde_json::json!({"login": "octocat"}))
        .unwrap();
    let user: User = serde_json::from_value(v).unwrap();
    assert_eq!(user.login, "octocat");
    assert_eq!(user.id, 42);

    let v = gh
        .call_tool("list_repos", &serde_json::json!({"owner": "tokitai"}))
        .unwrap();
    let repos: Vec<Repo> = serde_json::from_value(v).unwrap();
    assert_eq!(repos.len(), 2);
    assert_eq!(repos[0].name, "tokitai/hello");

    println!("ok - {} tools registered", GitHubClient::tool_count());
}
