//! T-024: emit `OUT_DIR/tokitai_manifest.rs` carrying the resolved
//! `tokitai-core` version the binary was compiled against. The
//! runtime `infer` path reads this constant when validating
//! `--require-tokitai=<prefix>` (mirrors the MCP server's
//! build.rs).
//!
//! The path lookup walks upward from `CARGO_MANIFEST_DIR` looking
//! for a `Cargo.toml` with a `[workspace]` table, instead of
//! hardcoding `../Cargo.lock`. See M-3 in CHANGELOG.md for
//! context.

use std::fs;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Cargo.toml");

    let manifest_dir = PathBuf::from(
        std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR is set by cargo"),
    );
    let workspace_root = find_workspace_root(&manifest_dir);
    let version = read_tokitai_core_version(&workspace_root);

    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo"));
    let dest = out_dir.join("tokitai_manifest.rs");
    let body = format!(
        "/// T-024: resolved at compile time from the workspace Cargo.lock.\n\
         /// The runtime `infer` path uses this to enforce\n\
         /// `--require-tokitai=<prefix>`.\n\
         pub const TOKITAI_CORE_VERSION: &str = \"{version}\";\n"
    );
    fs::write(&dest, body).expect("write tokitai_manifest.rs");
}

/// Walk upward from `start` looking for a `Cargo.toml` with a
/// `[workspace]` table. Returns the directory containing that file.
/// Falls back to `start` when nothing is found.
fn find_workspace_root(start: &std::path::Path) -> PathBuf {
    let mut current: Option<&std::path::Path> = Some(start);
    while let Some(dir) = current {
        let candidate = dir.join("Cargo.toml");
        if candidate.is_file() {
            if let Ok(contents) = fs::read_to_string(&candidate) {
                if contents.contains("[workspace]") {
                    return dir.to_path_buf();
                }
            }
        }
        current = dir.parent();
    }
    start.to_path_buf()
}

/// Read the resolved version of the `tokitai-core` package from
/// the workspace `Cargo.lock`. Returns the sentinel
/// `"0.0.0-unresolved"` when the lock file is missing or the
/// package is not listed.
fn read_tokitai_core_version(workspace_root: &std::path::Path) -> String {
    let lock = workspace_root.join("Cargo.lock");
    let Ok(contents) = fs::read_to_string(&lock) else {
        eprintln!(
            "cargo:warning=Tokitai build.rs: Cargo.lock not found at {}",
            lock.display()
        );
        return "0.0.0-unresolved".to_string();
    };

    let mut in_tokitai_core = false;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            in_tokitai_core = false;
            continue;
        }
        if trimmed.starts_with("name = ") {
            let name = trimmed.trim_start_matches("name = ").trim_matches('"');
            in_tokitai_core = name == "tokitai-core";
            continue;
        }
        if in_tokitai_core && trimmed.starts_with("version = ") {
            return trimmed
                .trim_start_matches("version = ")
                .trim_matches('"')
                .to_string();
        }
    }
    eprintln!(
        "cargo:warning=Tokitai build.rs: `tokitai-core` package not found in {}",
        lock.display()
    );
    "0.0.0-unresolved".to_string()
}
