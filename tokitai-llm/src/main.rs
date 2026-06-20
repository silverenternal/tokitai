//! T-034: binary entry point. All real work lives in the subcommand
//! handlers under `cli::*`; this file is intentionally thin so a
//! future `tokitai-llm` v0.2 can swap `clap` for `argh` without
//! touching the business logic.

use std::time::Duration;

use clap::Parser;
use tokitai_llm::cache::ToolCache;
use tokitai_llm::cli::{Cli, Command};
use tokitai_llm::infer;
use tokitai_llm::Result;

#[tokio::main]
async fn main() -> Result<()> {
    // tracing-subscriber picks up `RUST_LOG=info,tokitai_llm=debug`
    // (or any other env-filter expression). Failures here are not
    // fatal: a missing subscriber just means logs go to stderr in
    // the default format.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .try_init();

    let cli = Cli::parse();
    match cli.command {
        Command::Verify(args) => tokitai_llm::verify::run(args).await,
        Command::Infer(args) => {
            // T-047: build the tool-result cache from CLI args.
            // `--tool-cache-size 0` disables the cache entirely
            // so `--no-tool-cache` style behaviour is reachable
            // through the same path.
            let tool_cache = if args.tool_cache_size == 0 {
                None
            } else {
                Some(ToolCache::with_capacity_and_ttl(
                    args.tool_cache_size,
                    Duration::from_secs(args.tool_cache_ttl),
                ))
            };
            infer::run(args, tool_cache).await
        }
        Command::Examples(args) => tokitai_llm::examples::run(args).await,
        Command::InferCapabilities(args) => tokitai_llm::infer_capabilities::run(args).await,
    }
}
