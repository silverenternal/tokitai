# Contributing Guide

Thanks for considering contributing to Tokitai!

## Quick Navigation

- [Reporting Bugs](#reporting-bugs)
- [Feature Suggestions](#feature-suggestions)
- [Development Environment Setup](#development-environment-setup)
- [Submitting Code](#submitting-code)
- [Code Style](#code-style)
- [Testing](#testing)
- [Documentation](#documentation)

---

## Reporting Bugs

We use GitHub Issues to track bugs.

### Before Submitting a Bug Report

1. Search existing Issues to make sure it hasn't already been reported
2. Confirm the bug still exists in the latest version
3. Prepare a minimal reproducible code example

### Bug Report Template

```markdown
**Describe the problem**
Briefly describe what the problem is.

**Steps to reproduce**
1. Create a project...
2. Add the code...
3. Run `cargo run`...
4. See the error...

**Expected behavior**
Clearly describe what you expected to happen.

**Actual behavior**
Describe what actually happened.

**Environment information**
- Rust version: `rustc --version`
- Tokitai version: `cargo tree | grep tokitai`
- Operating system: Windows/Linux/macOS

**Code example**
```rust
// Minimal reproducible code
```

**Additional information**
Any screenshots, logs, or other relevant information.
```

---

## Feature Suggestions

We welcome feature suggestions! Please open an Issue that includes:

1. **Feature description**: Briefly describe your suggestion
2. **Use case**: Describe the scenario in which this feature would be used
3. **Alternatives considered**: Describe any other solutions you have considered
4. **Additional information**: Any relevant code examples or references

---

## Development Environment Setup

### 1. Clone the Repository

```bash
git clone https://github.com/silverenternal/tokitai.git
cd tokitai
```

### 2. Install the Rust Toolchain

```bash
# Install rustup (if you haven't already)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install the latest stable Rust
rustup update stable

# Verify the installation
rustc --version
cargo --version
```

### 3. Install Development Tools (Recommended)

```bash
# Code formatter
rustup component add rustfmt

# Clippy lints
rustup component add clippy

# View macro expansions
cargo install cargo-expand

# Documentation generator
cargo install cargo-doc
```

### 4. Build the Project

```bash
# Build the entire workspace
cargo build --workspace

# Run the tests
cargo test --workspace

# Run Clippy
cargo clippy --workspace --all-targets -- -D warnings
```

---

## Submitting Code

### 1. Create a Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/issue-123
```

### 2. Write the Code

Follow the [Code Style](#code-style) guide.

### 3. Run the Tests

```bash
# Make sure all tests pass
cargo test --workspace

# Make sure Clippy reports no warnings
cargo clippy --workspace --all-targets -- -D warnings

# Make sure the code is formatted
cargo fmt --all -- --check
```

### 4. Commit Your Changes

```bash
git add .
git commit -m "feat: add new feature

Describe in detail what the new feature does and how to use it.

Fixes #123"
```

### 5. Open a Pull Request

1. Push your branch: `git push origin feature/your-feature-name`
2. Open a Pull Request on GitHub
3. Fill in the PR description and link any related Issues

---

## Code Style

### Rust Code Style

We follow the [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/):

1. **Formatting**: Use `cargo fmt` to auto-format
2. **Naming**:
   - Types, traits: `PascalCase`
   - Functions, variables: `snake_case`
   - Constants, macros: `SCREAMING_SNAKE_CASE`
3. **Documentation**: All public APIs must have doc comments
4. **Error handling**: Use the `Result` type and avoid `unwrap()`

### Commit Message Style

We follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Types:**

| Type | Description |
|------|-------------|
| `feat` | A new feature |
| `fix` | A bug fix |
| `docs` | Documentation only changes |
| `style` | Formatting changes (no functional impact) |
| `refactor` | A code change that neither fixes a bug nor adds a feature |
| `perf` | A code change that improves performance |
| `test` | Adding or correcting tests |
| `chore` | Build, configuration, tooling, etc. |

**Example:**

```
feat: add async tool call support

- Support async fn as tool methods
- Auto-generate async call_tool methods
- Add async example code

Fixes #45
```

---

## Testing

### Running Tests

```bash
# Run all tests
cargo test --workspace

# Run a specific test
cargo test --package tokitai-macros test_basic_tool

# Run UI tests
cargo test --test ui_tests

# Generate a test coverage report (requires cargo-tarpaulin)
cargo tarpaulin --workspace --out Html
```

### Writing Tests

#### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add() {
        let calc = Calculator;
        assert_eq!(calc.add(2, 3), 5);
    }

    #[test]
    fn test_error_handling() {
        let result = divide(10, 0);
        assert!(result.is_err());
    }
}
```

#### Integration Tests

```rust
// tests/integration_test.rs
use tokitai::tool;

#[test]
fn test_tool_macro() {
    // Test macro-generated code
}
```

#### UI Tests

```rust
// tests/ui/01_basic_tool.rs
use tokitai::tool;

#[tool]
impl MyTools {
    pub fn my_tool(&self) {}
}

fn main() {}
```

---

## Documentation

### Code Documentation

All public APIs must have doc comments:

```rust
/// Tool definition - describes an AI-callable tool
///
/// # Fields
///
/// * `name` - The tool name
/// * `description` - The tool description
/// * `input_schema` - The JSON schema for the input parameters
///
/// # Examples
///
/// ```
/// use tokitai::ToolDefinition;
///
/// let tool = ToolDefinition::new("add", "Add two numbers", "{\"type\":\"object\"}");
/// assert_eq!(tool.name, "add");
/// ```
pub struct ToolDefinition {
    pub name: &'static str,
    // ...
}
```

### Documentation Tests

Make sure the code examples in the documentation compile and run:

```bash
# Run the doc tests
cargo test --doc
```

### Updating Documentation

When you change code, please update the documentation at the same time:

1. The code doc comments
2. README.md
3. Detailed documentation under the docs/ directory
4. Example code

---

## Release Process

### Publishing a New Version

1. Update `CHANGELOG.md`
2. Update the version in `Cargo.toml`
3. Run the full test suite: `cargo test --workspace`
4. Run Clippy: `cargo clippy --workspace --all-targets -- -D warnings`
5. Build the documentation: `cargo doc --workspace --no-deps`
6. Commit and tag: `git tag -a v0.3.0 -m "Release v0.3.0"`
7. Publish to crates.io: `cargo publish`

---

## Questions?

If you have any questions, feel free to:

- Open a [Discussion](https://github.com/silverenternal/tokitai/discussions)
- Open an Issue

---

## Code of Conduct

We follow the [Rust Code of Conduct](https://www.rust-lang.org/policies/code-of-conduct). Please:

- Be open and inclusive
- Respect differing viewpoints
- Gracefully accept constructive criticism
- Focus on what is best for the community
- Show empathy toward other community members

---

Thanks for contributing to Tokitai!
