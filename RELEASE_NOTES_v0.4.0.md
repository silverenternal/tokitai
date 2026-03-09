# Release Notes: v0.4.0

**Release Date**: 2026-03-10  
**Rating**: 9.0/10 ⬆️ (+0.5 from 8.5)

---

## 🎯 Release Highlights

v0.4.0 是一个**破坏性变更版本**，主要修复 P11 审查报告中的致命问题，简化 API 设计，提升开发者体验。

### 核心改进

1. **API 简化**: `TOOL_DEFINITIONS` 常量 → `tool_definitions()` 方法
2. **参数属性修正**: 统一属性语法，消除编译错误
3. **文档清理**: 36+ 处旧 API 引用更新
4. **依赖完善**: examples 完整依赖配置

---

## ⚠️ Breaking Changes (迁移指南)

### 1. TOOL_DEFINITIONS → tool_definitions()

**旧代码:**
```rust
pub const TOOL_DEFINITIONS: &str = include_str!("../tools.json");
```

**新代码:**
```rust
pub fn tool_definitions() -> &'static [&'static str] {
    // 动态生成工具定义
}
```

**影响范围:**
- `tokitai-core/docs/*.md` (12 处)
- `tokitai/docs/*.md` (12 处)
- `tokitai-macros/docs/*.md` (12 处)

### 2. 参数属性语法修正

**❌ 旧语法 (编译错误):**
```rust
pub fn format_date(&self, date: String, 
                   #[tool_attr(example_format = "%Y/%m/%d")] format: String) {}
```

**✅ 新语法:**
```rust
#[tool(example_format = "%Y/%m/%d")]
pub fn format_date(&self, date: String, format: String) {}
```

**原理:** 参数级属性（如 `xxx_param`）必须在方法级别声明，由宏内部处理。

---

## 🐛 Bug Fixes

| 优先级 | 问题 | 修复 |
|--------|------|------|
| P0 | `examples/mcp_server_demo.rs` 参数属性冲突 | 移动属性到方法级别 |
| P0 | `examples/Cargo.toml` 依赖缺失 | 添加 axum, tower-http, tracing-subscriber |
| P1 | 文档旧 API 引用 (36+ 处) | 批量替换为 `tool_definitions()` |
| P2 | 文档块语法错误 | ` ```rust,ignore` → ` ```text` |

---

## 📦 Updated Crates

所有 crate 版本同步到 `0.4.0`:

```toml
tokitai-core = "0.4.0"
tokitai-macros = "0.4.0"
tokitai = "0.4.0"
tokitai-mcp-server = "0.4.0"
```

---

## ✅ Verification

| 检查项 | 结果 |
|--------|------|
| 测试 | ✅ 85/85 通过 |
| Clippy | ✅ 无警告 |
| 文档生成 | ✅ 无警告 |
| 编译 | ✅ 无错误 |

---

## 🚀 Publishing Order

按依赖顺序发布到 crates.io:

```bash
# 1. 核心类型 (零依赖)
cargo publish -p tokitai-core

# 2. 宏 (依赖 tokitai-core)
cargo publish -p tokitai-macros

# 3. 主 crate (依赖 tokitai-core, tokitai-macros)
cargo publish -p tokitai

# 4. MCP 服务器 (依赖 tokitai)
cargo publish -p tokitai-mcp-server
```

---

## 📋 GitHub Release Checklist

- [ ] 创建 Git tag: `git tag -a v0.4.0 -m "Release v0.4.0"`
- [ ] 推送 tag: `git push origin v0.4.0`
- [ ] 创建 GitHub Release (使用此笔记)
- [ ] 发布到 crates.io (按顺序)
- [ ] 更新 README.md 版本号
- [ ] 通知用户破坏性变更

---

## 📖 Related Documentation

- [CHANGELOG.md](CHANGELOG.md) - 完整变更日志
- [P11_REVIEW_FIX_REPORT_ROUND2.md](P11_REVIEW_FIX_REPORT_ROUND2.md) - P11 审查修复报告
- [CONTRIBUTING.md](CONTRIBUTING.md) - 贡献指南

---

## 💡 Recommended Practices

为 `Option<T>` 参数添加 default 或 example:

```rust
// ✅ 推荐做法
#[tool(default_email = "null", example_subject = "咨询")]
pub fn send_email(&self, to: String, email: Option<String>, subject: Option<String>) {}
```

**原因:** AI 需要知道可选参数是否可以省略，避免调用失败。

---

**Full Changelog**: https://github.com/silverenternal/tokitai/compare/v0.3.4...v0.4.0
