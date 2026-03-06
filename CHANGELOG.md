# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1] - 2026-03-06

### Added
- `Display` trait implementation for `ToolDefinition` for easier debugging
- Comprehensive `starter_project` example template with complete project structure
- `SKILL_TEMPLATE.md` documentation with ready-to-use templates
- Enhanced error messages for generic method violations with actionable suggestions
- Documentation warnings for `call_tool_sync` blocking behavior

### Fixed
- User custom error messages now preserved instead of being replaced with generic "方法执行失败"
- Removed unnecessary `#[allow(dead_code)]` attributes in example code
- Doc comment formatting in starter project examples

### Changed
- README.md now includes "5 分钟快速上手" quick start guide
- README.md now mentions `#[tool(skip)]` feature in features section
- Improved panic error message when calling async tools without runtime
- Starter project now properly organized with modular tool definitions

## [0.3.0] - 2026-03-06

### Added
- `#[tool(skip)]` attribute to exclude public methods from tool registration
- Generic method detection with helpful compile-time error messages
- `call_tool_sync()` now safely handles async methods by checking for tokio runtime
- `log` crate dependency for proper logging (replaces `eprintln!`)
- UI test for `#[tool(skip)]` functionality

### Changed
- **Breaking**: `ToolError::message` changed from `&'static str` to `String` (fixes memory leak)
- **Breaking**: All crate versions bumped to 0.3.0
- Improved `MethodToolAttrs` parsing to support both `#[tool(skip)]` and `#[tool(name = "...", desc = "...")]`
- `get_json_type()` now correctly handles nested `Option<T>` types
- Generated wrapper functions include `#[allow(clippy::all)]` to reduce noise
- MCP schema parsing now logs warnings instead of silently failing
- Example code now follows best practices (Default implementations, etc.)

### Fixed
- Memory leak in `ToolError::message` (removed `Box::leak`)
- `call_tool_sync()` panic when calling async methods without tokio runtime (now returns error)
- Clippy `collapsible_match` warnings in macro code
- Clippy warnings in example code (`new_without_default`, `redundant_closure`, `dead_code`)
- `eprintln!` in library code (replaced with `log::warn!`)

### Removed
- `Box::leak` usage for string allocation (memory-safe implementation)

## [0.2.0] - 2026-03-05

### Changed
- **Breaking**: Renamed macros from `#[ai_skill]`/`#[ai_tool]` to single `#[tool]` macro
- **Breaking**: Simplified API - only one attribute needed
- **Breaking**: Removed runtime dependencies (tokio, reqwest, uuid) from core
- Refactored into three crates: `tokitai-core`, `tokitai-macros`, `tokitai`
- Updated repository to https://github.com/silverenternal/tokitai

### Removed
- **Breaking**: Removed `AnthropicAdapter` and `McpServer` - users can implement their own AI adapters
- Removed business-specific error types for better generality

### Added
- Zero-dependency core types with `&'static str` for compile-time tool definitions
- trybuild UI tests for macro validation
- Optional `mcp` feature for MCP protocol support

## [0.1.0] - 2026-03-05

### Added
- Initial release with `#[ai_skill]` and `#[ai_tool]` macros
- Support for Anthropic (Claude) API adapter
- MCP (Model Context Protocol) server implementation
- Skill registry for managing multiple AI skills
