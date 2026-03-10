//! Build script to set environment variables for macro tests
//!
//! This suppresses macro warnings during test builds to keep test output clean

fn main() {
    // Suppress tokitai macro warnings in test builds
    println!("cargo:rustc-env=TOKITAI_QUIET=1");
}
