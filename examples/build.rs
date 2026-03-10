//! Build script to set environment variables for examples
//!
//! This suppresses macro warnings during example builds

fn main() {
    // Suppress tokitai macro warnings in example builds
    println!("cargo:rustc-env=TOKITAI_QUIET=1");
}
