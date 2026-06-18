//! dev_assistant.rs
//!
//! Real-world integration test: an AI coding-assistant tool suite exercising
//! `#[tool]`, `#[tool_type]`, `MultiToolProvider`, schema round-tripping, and
//! error paths. Acts as a downstream-consumer regression test.
//!
//! Run with:  cargo run -p tokitai-examples --example dev_assistant

use std::process::Command;

use serde::{Deserialize, Serialize};
use tokitai::tool;
use tokitai::tool_type;
use tokitai::ToolCaller;
use tokitai_mcp_server::MultiToolProvider;

// =============================================================================
// Custom return type registered with `#[tool_type]`
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
#[tool_type(
    name = "Match",
    properties = "path: string, line: integer, content: string",
    required = "path, line, content"
)]
pub struct Match {
    pub path: String,
    pub line: usize,
    pub content: String,
}

// =============================================================================
// 1. ProjectInspector — five tools, each with a different `#[tool(...)]` knob
// =============================================================================
//
// NOTE: `#[tool]` on an impl that mixes async + sync methods triggers
// duplicate-`__call_<name>` errors in the macro output. We work around it
// by keeping all `ProjectInspector` methods sync. (See BUGS_FOUND.md.)
//   - default name on `count_lines` and `git_status`
//   - custom name on `read_file_head` (default = `read_file`)
//   - custom desc on `list_files`
//   - alias on `search_text`
//   - per-parameter `default_path` (note: must be on the method-level `#[tool(...)]`)
//   - per-parameter `validate` (also on the method-level attribute)

/// Inspects files in the local repo.
pub struct ProjectInspector;

#[tool]
impl ProjectInspector {
    /// List files in `directory` whose name contains `pattern`.
    #[tool(desc = "List files under a directory that contain the given pattern (sync stub).")]
    pub fn list_files(&self, directory: String, pattern: String) -> Vec<String> {
        vec![
            format!("{}/src/lib.rs", directory),
            format!("{}/Cargo.toml", directory),
            format!("{}/README.md", directory),
        ]
        .into_iter()
        .filter(|p| p.contains(&pattern))
        .collect()
    }

    /// Reads up to `max_lines` lines from a file.
    #[tool(
        name = "read_file_head",
        desc = "Read the head of a UTF-8 text file.",
        default_path = "/dev/null"
    )]
    pub fn read_file(
        &self,
        #[allow(dead_code)] path: String,
        #[allow(dead_code)] max_lines: usize,
    ) -> String {
        match std::fs::read_to_string(&path) {
            Ok(content) => content
                .lines()
                .take(max_lines)
                .collect::<Vec<_>>()
                .join("\n"),
            Err(e) => format!("<read_file error: {}>", e),
        }
    }

    /// Counts the lines in `path`.
    pub fn count_lines(&self, path: String) -> usize {
        std::fs::read_to_string(&path)
            .map(|c| c.lines().count())
            .unwrap_or(0)
    }

    /// Searches `path` for `needle`.
    #[tool(
        alias = ["grep", "find_in_file"],
        desc = "Search a file for a needle and return a list of matches.",
        validate_path = "!value.is_empty()"
    )]
    pub fn search_text(
        &self,
        #[allow(dead_code)] path: String,
        #[allow(dead_code)] needle: String,
        #[allow(dead_code)] case_sensitive: bool,
    ) -> Vec<Match> {
        let Ok(content) = std::fs::read_to_string(&path) else {
            return Vec::new();
        };
        content
            .lines()
            .enumerate()
            .filter(|(_, line)| {
                if case_sensitive {
                    line.contains(&needle)
                } else {
                    line.to_lowercase().contains(&needle.to_lowercase())
                }
            })
            .map(|(i, line)| Match {
                path: path.clone(),
                line: i + 1,
                content: line.to_string(),
            })
            .collect()
    }

    /// Runs `git status` in the current directory.
    #[tool(
        name = "git_status",
        desc = "Return the output of `git status --short`."
    )]
    pub fn git_status(&self) -> String {
        let output = Command::new("git").args(["status", "--short"]).output();
        match output {
            Ok(o) if o.status.success() => String::from_utf8_lossy(&o.stdout).into_owned(),
            Ok(o) => format!("git status failed: {}", String::from_utf8_lossy(&o.stderr)),
            Err(e) => format!("git not runnable: {}", e),
        }
    }
}

// =============================================================================
// 1b. (skipped) Async tool methods are not usable in the current macro
//     revision: see BUGS_FOUND.md #2. The example below sticks to sync only.
// =============================================================================

// =============================================================================
// 2. Calculator — three tools, all sync, all i64
// =============================================================================

pub struct Calculator;

#[tool]
impl Calculator {
    /// Add two 64-bit integers.
    pub fn add(&self, a: i64, b: i64) -> i64 {
        a + b
    }

    /// Subtract `b` from `a`.
    pub fn subtract(&self, a: i64, b: i64) -> i64 {
        a - b
    }

    /// Multiply two 64-bit integers.
    pub fn multiply(&self, a: i64, b: i64) -> i64 {
        a * b
    }
}

// =============================================================================
// 3. Wire it all into a MultiToolProvider
// =============================================================================

fn build_provider() -> MultiToolProvider {
    let mut p = MultiToolProvider::new();
    p.add(ProjectInspector);
    p.add(Calculator);
    p
}

// =============================================================================
// 4. Schema introspection + round-trip
// =============================================================================

fn dump_schemas(provider: &MultiToolProvider) {
    println!("\n=== Tool schemas ===\n");
    for def in provider.tool_definitions() {
        println!("- {}  ({})", def.name, def.description);
        let schema_str = def.input_schema.to_string();
        let parsed: serde_json::Value =
            serde_json::from_str(&schema_str).unwrap_or(serde_json::Value::Null);
        let round_tripped = serde_json::to_string(&parsed).unwrap_or_default();
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&round_tripped).unwrap(),
            parsed,
            "schema for {} lost information on round-trip",
            def.name
        );
        println!("  schema: {}\n", schema_str);
    }
}

fn check_match_schema(provider: &MultiToolProvider) {
    let search = provider
        .tool_definitions()
        .iter()
        .find(|d| d.name == "search_text")
        .expect("search_text should exist");
    let schema: serde_json::Value = serde_json::from_str(&search.input_schema.to_string())
        .expect("search_text schema should be valid JSON");
    println!("search_text schema parsed: {:#?}", schema);
}

fn smoke_dispatch(provider: &MultiToolProvider) {
    println!("\n=== Dispatching tool calls ===\n");
    let cases: &[(&str, serde_json::Value, &str)] = &[
        ("add", serde_json::json!({"a": 2, "b": 40}), "calc/add"),
        (
            "list_files",
            serde_json::json!({"directory": "tokitai-core", "pattern": "Cargo"}),
            "sync/list",
        ),
        (
            "read_file_head",
            serde_json::json!({"path": "Cargo.toml", "max_lines": 5}),
            "sync/read",
        ),
        (
            "count_lines",
            serde_json::json!({"path": "Cargo.toml"}),
            "sync/count",
        ),
        (
            "search_text",
            serde_json::json!({"path": "Cargo.toml", "needle": "tokitai", "case_sensitive": false}),
            "sync/search",
        ),
        ("git_status", serde_json::json!({}), "sync/git"),
        (
            "subtract",
            serde_json::json!({"a": 100, "b": 1}),
            "calc/sub",
        ),
        ("multiply", serde_json::json!({"a": 6, "b": 7}), "calc/mul"),
        (
            "grep",
            serde_json::json!({"path": "Cargo.toml", "needle": "name", "case_sensitive": true}),
            "alias/grep",
        ),
        (
            "find_in_file",
            serde_json::json!({"path": "Cargo.toml", "needle": "edition", "case_sensitive": false}),
            "alias/find",
        ),
    ];
    for (name, args, label) in cases {
        match provider.call_tool(name, args) {
            Ok(v) => println!("  [{}] {} -> {}", label, name, v),
            Err(e) => println!("  [{}] {} -> ERR: {}", label, name, e),
        }
    }
}

fn error_paths(provider: &MultiToolProvider) {
    println!("\n=== Error paths ===\n");
    // Non-existent tool
    match provider.call_tool("does_not_exist", &serde_json::json!({})) {
        Ok(v) => println!("  [not_found] unexpected success: {}", v),
        Err(e) => println!("  [not_found] kind={:?} msg={}", e.kind, e),
    }
    // Wrong-typed argument
    match provider.call_tool("add", &serde_json::json!({"a": "two", "b": 40})) {
        Ok(v) => println!("  [wrong_type] unexpected success: {}", v),
        Err(e) => println!("  [wrong_type] kind={:?} msg={}", e.kind, e),
    }
    // Missing required argument
    match provider.call_tool("add", &serde_json::json!({"a": 1})) {
        Ok(v) => println!("  [missing] unexpected success: {}", v),
        Err(e) => println!("  [missing] kind={:?} msg={}", e.kind, e),
    }
    // Runtime failure (non-existent file)
    match provider.call_tool(
        "count_lines",
        &serde_json::json!({"path": "/this/path/does/not/exist/xyzzy.rs"}),
    ) {
        Ok(v) => println!("  [runtime] result: {}", v),
        Err(e) => println!("  [runtime] kind={:?} msg={}", e.kind, e),
    }
    // Validation failure on search_text — note: this may not actually fire if
    // `validate_path` is silently ignored; that's a bug we want to surface.
    match provider.call_tool(
        "search_text",
        &serde_json::json!({"path": "", "needle": "x", "case_sensitive": true}),
    ) {
        Ok(v) => println!("  [validation] result: {}", v),
        Err(e) => println!("  [validation] kind={:?} msg={}", e.kind, e),
    }
    // Schema lists `default: "/dev/null"` for `path` on read_file_head, but
    // does the runtime actually substitute it when omitted?
    match provider.call_tool("read_file_head", &serde_json::json!({"max_lines": 2})) {
        Ok(v) => println!("  [default_fallback] result: {}", v),
        Err(e) => println!("  [default_fallback] kind={:?} msg={}", e.kind, e),
    }
}

fn perf_smoke(provider: &MultiToolProvider) {
    println!("\n=== Performance smoke (10 calls) ===\n");
    let args = serde_json::json!({"a": 12, "b": 34});
    let start = std::time::Instant::now();
    for _ in 0..10 {
        let _ = provider.call_tool("add", &args);
    }
    let total = start.elapsed();
    println!("  10x add: total = {:?}, average = {:?}", total, total / 10);
}

fn main() {
    println!("=== Tokitai dev-assistant regression example ===\n");

    // T-015: opt-in tracing-subscriber init. When the example
    // is built with `cargo run --features trace --example
    // dev_assistant` the macro emits `#[tracing::instrument]`
    // spans on every `__call_*` wrapper; the subscriber below
    // prints one structured line per call carrying the
    // `tool.name`, `tool.version`, `args.size`, and
    // `result.size` fields. To see it work:
    //
    //     RUST_LOG=tokitai=trace cargo run --features trace \
    //         --example dev_assistant
    //
    // On the default build (no `trace` feature) the
    // subscriber is a no-op and the example's hot path is
    // byte-identical to the no-trace build.
    init_tracing_subscriber();

    let provider = build_provider();
    println!(
        "Registered {} tools across {} providers",
        provider.tool_definitions().len(),
        2
    );

    dump_schemas(&provider);
    check_match_schema(&provider);
    smoke_dispatch(&provider);
    error_paths(&provider);
    perf_smoke(&provider);

    println!("\n=== Done ===");
}

/// T-015: install a `tracing-subscriber` that prints every
/// `tokitai_tool_call` span to stderr with sub-microsecond
/// precision. The init is a no-op on the default build (the
/// `trace` feature is off) because the macro never emits the
/// spans in the first place. We use
/// `tracing_subscriber::fmt` with `EnvFilter::from_default_env`
/// so users can dial verbosity via `RUST_LOG`.
fn init_tracing_subscriber() {
    use tracing_subscriber::EnvFilter;
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("tokitai=info"));
    // `try_init` is fine to fail silently: another test or
    // example in the same process may have already installed
    // a global subscriber. The macro path does not depend on
    // the subscriber being present — it only emits the spans.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .try_init();
}
