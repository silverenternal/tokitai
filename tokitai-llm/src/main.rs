//! T-034: binary entry point. All real work lives in the subcommand
//! handlers under `cli::*`; this file is intentionally thin so a
//! future `tokitai-llm` v0.2 can swap `clap` for `argh` without
//! touching the business logic.

use clap::Parser;
use tokitai_llm::cli::{Cli, Command};
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
        Command::Infer(args) => tokitai_llm::infer::run(args).await,
        Command::Examples(args) => tokitai_llm::examples::run(args).await,
        Command::InferCapabilities(args) => tokitai_llm::infer_capabilities::run(args).await,
    }
}
