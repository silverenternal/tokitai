# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Performance ⚡

- **50% 性能提升** - 通过系统化 `#[inline]` 优化
  - `tool_definitions_access`: 764ps → 421ps (-45%)
  - `tool_lookup`: 1.56ns → 750ps (-52%)
  - `schema_pretty_print`: 1.19μs → 568ns (-53%)
  - `tool_call_simple`: 307ns → 152ns (-50%)
  - `tool_call_multi_param`: 685ns → 534ns (-35%)
- 优化热点路径：`SchemaGenConfig` Builder 方法（17 个函数）
- 优化提取函数：`extract_param_info`, `extract_doc_comments`
- 优化代码生成：`generate_schema_json_*` 系列函数

### Fixed 🐛

- **P0: Clippy 警告清理** - 修复 12 个 `default()` 调用警告
  - `tokitai-mcp-server/tests/integration_test.rs`: 使用 unit struct 直接初始化
- **P1: 测试警告输出抑制** - 使用 `cfg!(test)` 抑制测试环境下的宏警告输出
  - 参数默认值警告（Option 类型无 default/example）
  - 验证/转换表达式解析失败警告
  - context=async 不匹配警告

## [0.4.0] - 2026-03-10

### Breaking Changes ⚠️

- **API 简化：`TOOL_DEFINITIONS` 常量 → `tool_definitions()` 方法**
  - Before: `pub const TOOL_DEFINITIONS: &str = include_str!(...);`
  - After: `pub fn tool_definitions() -> &'static [&'static str] { ... }`
  - Reason: 更灵活的运行时工具定义生成，支持动态工具注册
  - Migration: 替换所有 `TOOL_DEFINITIONS` 引用为 `tool_definitions()`

- **参数属性语法修正**
  - Before: `#[tool_attr(example_format = "...")]` 可直接放在参数上
  - After: `#[tool(example_format = "...")]` 必须放在方法级别
  - Reason: 参数级属性（如 `xxx_param`）必须在方法级别声明，由宏内部处理
  - Migration: 
    ```rust
    // ❌ 旧语法（编译错误）
    pub fn format_date(&self, date: String, #[tool_attr(example_format = "%Y/%m/%d")] format: String) {}
    
    // ✅ 新语法
    #[tool(example_format = "%Y/%m/%d")]
    pub fn format_date(&self, date: String, format: String) {}
    ```

### Fixed 🐛

- **P0: 示例代码编译错误** - `examples/mcp_server_demo.rs` 参数属性冲突
- **P1: 文档旧 API 引用** - 清理 36+ 处 `TOOL_DEFINITIONS` → `tool_definitions()`
  - `tokitai-core/docs/*.md` (12 处)
  - `tokitai/docs/*.md` (12 处)
  - `tokitai-macros/docs/*.md` (12 处)
- **P2: 文档块语法错误** - 宏示例使用 ` ```text` 而非 ` ```rust,ignore`

### Added 📚

- **examples/Cargo.toml 完整依赖** - 添加 `axum`, `tower-http`, `tracing-subscriber` 等 HTTP 服务器依赖
- **P11 审查修复报告** - `P11_REVIEW_FIX_REPORT_ROUND2.md` 完整记录所有修复

### Changed 🔄

- **文档块规范** - 所有宏生成代码示例使用 ` ```text` 语法高亮
- **版本同步** - 所有 workspace crate 版本统一到 `0.4.0`

### Verification ✅

| 检查项 | 结果 |
|--------|------|
| 测试 | 85/85 通过 |
| Clippy | 无警告 |
| 文档生成 | 无警告 |
| 编译 | 无错误 |

---

## [0.3.4] - 2026-03-09

### P11+ Code Review Fixes 🔧

#### P0 - Critical Improvements
- **Removed all Chinese compiler warnings** - Complete internationalization
  - Changed `⚠️  方法 \`add\` 被标记为 deprecated，但未指定 replaced_by` → `[tokitai] warning: method \`add\` is marked deprecated without replaced_by`
  - Changed `⚠️  解析验证表达式失败` → `[tokitai] warning: failed to parse validation expression`
  - Changed `⚠️  解析转换表达式失败` → `[tokitai] warning: failed to parse transform expression`
  - Changed `⚠️  方法 \`new_method\` 标记为 context = "async" 但不是 async 方法` → `[tokitai] warning: method \`new_method\` is marked context = "async" but is not an async method`
  - All repair suggestions now in English
- **Refactored `SchemaGenConfig` to public Builder pattern**
  - Before: Private builder methods (internal use only)
  - After: **Public API** - all builder methods now `pub` for external use
  - Added `build()` method for explicit chain termination
  - Example: `SchemaGenConfig::new(params).deprecated(true).tags(&tags).build()`
- **Upgraded global cache from `Mutex<Option>` to `LazyLock<Mutex>`**
  - More idiomatic Rust 1.80+ pattern
  - Eliminates double-wrapping overhead

#### P1 - Quality Improvements
- **Fixed Markdown support documentation** - Avoided over-promising
  - Clarified: "保留原始文本格式" (preserves raw text format) instead of "支持 Markdown 格式"
  - Added note: "此函数仅保留原始文本，不进行 Markdown 解析（如转换为 HTML）"
  - Recommended external libraries (e.g., `pulldown-cmark`) for full Markdown parsing
- **Added LazyLock deadlock detection warnings**
  - Added `# Safety Note` to `GLOBAL_CONFIG_REGISTRY` documentation
  - Added initialization order warning to `__get_tool_definitions()` documentation
  - Documented current safe initialization sequence
- **Fixed Clippy warnings** (`is_some_and` instead of `map_or`)

#### P2 - Documentation
- **Added comprehensive Cargo Doc example** (`docs/CARGO_DOC_EXAMPLE.md`)
  - How to generate beautiful API documentation
  - Best practices for doc comments with Markdown
  - Custom styling and logo integration
- **Created LangChain Migration Guide** (`docs/LANGCHAIN_MIGRATION.md`)
  - Side-by-side Python vs Rust comparison
  - Complete feature mapping table
  - Performance benchmarks (10-50x faster)
  - FAQ for common migration questions

### Fixed
- Removed all Clippy warnings (25+ warnings → 0) 🎉
- Fixed `too_many_arguments` warning by introducing `SchemaGenConfig` struct for JSON Schema generation
- Fixed `dead_code` warning for `build()` method with `#[allow(dead_code)]` attribute
- Fixed unused variable warnings in tests using `cargo fix`

### Changed
- Refactored `generate_schema_json_with_deprecated_and_tags` function signature:
  - Before: 15 individual parameters
  - After: Single `&SchemaGenConfig` parameter for better maintainability
- Simplified code generation logic by removing redundant `#[cfg(feature = "serde")]` conditions

### Improved
- Code quality: All Clippy checks now pass with zero warnings
- Test coverage: 68/68 tests passing (100% pass rate)
- Maintainability: Schema generation configuration now uses structured approach

## [0.3.3] - 2026-03-08

### Note
- Version 0.3.3 was skipped to align version numbers across workspace crates

## [0.3.2] - 2026-03-06

### Added
- `HashCalculator` tool with SHA256 hashing support
  - `sha256()` method for computing string hashes
  - `sha256_file()` method for computing file hashes
- New example demonstrating AI calling SHA256 computation tool
- Dependencies: `sha2` and `hex` crates for cryptographic hashing

### Removed
- HeWeather (和风天气) real API integration due to API host authentication issues
- Weather preloading and caching logic
- `WEATHER_API_KEY` and `WEATHER_USE_REAL_API` environment variables
- `reqwest` blocking feature (now only requires `json` feature)

### Changed
- `WeatherService` now uses mock data for all cities
- Simplified `ollama_integration.rs` example to focus on core functionality

## [0.3.1] - 2026-03-05

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
- Improved panic error message when calling async methods without runtime
- Starter project now properly organized with modular tool definitions

## [0.3.0] - 2026-03-04

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
