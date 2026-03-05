# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
