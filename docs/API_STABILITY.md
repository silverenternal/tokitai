# API 稳定承诺

本文档记录 Tokitai 项目的 API 稳定政策和版本兼容性承诺。

---

## 📋 版本政策

Tokitai 遵循 [语义化版本 2.0](https://semver.org/spec/v2.0.0.html)：

- **主版本号 (Major)**: 破坏性变更
- **次版本号 (Minor)**: 新功能，向后兼容
- **修订号 (Patch)**: Bug 修复，向后兼容

---

## 🎯 v0.5.x 系列 - 稳定 API

以下 API 在 v0.5.x 系列中保持稳定：

### ✅ 稳定 API

| API | 说明 | 稳定性 |
|-----|------|--------|
| `#[tool]` 宏 | 核心过程宏 | ✅ 稳定 |
| `ToolProvider::tool_definitions()` | 获取工具定义 | ✅ 稳定 |
| `ToolProvider::call_tool()` | 调用工具 | ✅ 稳定 |
| `ToolDefinition` | 工具定义结构体 | ✅ 稳定 |
| `ToolError` | 工具错误类型 | ✅ 稳定 |
| `SchemaGenConfig` | Schema 生成配置 | ✅ 稳定 |

### ⚠️ 实验性 API

以下 API 可能在 v0.5.x 系列中发生变化：

| API | 说明 | 稳定性 |
|-----|------|--------|
| `MultiToolProvider` | 多工具提供者 | ⚠️ 实验性 |
| `McpServerWithProvider<T>` | MCP 服务器包装 | ⚠️ 实验性 |
| `ToolProvider::clone_definitions()` | 克隆工具定义 | ⚠️ 实验性 |

### 📝 属性语法

以下属性语法在 v0.4.x / v0.5.x 系列中保持稳定：

```rust
// 方法级属性
#[tool(name = "custom_name", desc = "自定义描述")]
#[tool(skip)]
#[tool(deprecated = true, replaced_by = "new_method")]
#[tool(alias = ["alias1", "alias2"])]
#[tool(tags = ["tag1", "tag2"])]
#[tool(visible = false)]

// 参数级属性（在方法级别声明）
#[tool(min_length_param = 1, max_length_param = 100)]
#[tool(min_param = 0, max_param = 150)]
#[tool(example_param = "example_value")]
#[tool(default_param = null)]
#[tool(validate_param = "value > 0")]

// 文档注释语法
/// @param name 参数描述
/// @validate name !value.is_empty()
/// @required name
```

---

## 🚀 v1.0.0 计划

### 发布条件

v1.0.0 发布前需满足以下条件：

- [ ] 所有稳定 API 经过 3 个月以上生产环境验证
- [ ] 无未解决的 P0/P1 级别 bug
- [ ] 完整的文档和迁移指南
- [ ] 社区反馈收集完成

### 承诺

v1.0.0 发布后：

- **所有公开 API 在 v1.x 系列中保持向后兼容**
- **破坏性变更将等到 v2.0.0**
- **提供至少 6 个月的 v0.5.x 维护支持**

---

## 📅 版本兼容性矩阵

| Tokitai 版本 | Rust 最低版本 | 兼容性说明 |
|-------------|--------------|------------|
| v0.5.x | 1.80+ | 当前稳定版 |
| v0.4.x | 1.80+ | 维护中，6 个月内继续接收安全修复 |
| v0.3.x | 1.80+ | 已废弃，建议升级 |
| v1.0.0 (计划) | 1.80+ | 长期支持版 |

---

## 🔄 破坏性变更政策

当必须进行破坏性变更时：

1. **提前通知**: 在 CHANGELOG.md 和 GitHub Release 中提前声明
2. **迁移指南**: 提供详细的迁移步骤和代码示例
3. **过渡期**: 提供至少一个次版本号的过渡期
4. **自动化迁移**: 尽可能提供 codemod 工具自动迁移代码

### 示例：v0.3 → v0.4 迁移

**变更**: `TOOL_DEFINITIONS` 常量 → `tool_definitions()` 方法

**迁移步骤**:

```rust
// ❌ 旧代码 (v0.3)
let tools = Calculator::TOOL_DEFINITIONS;

// ✅ 新代码 (v0.4)
let tools = Calculator::tool_definitions();
```

---

## 📞 反馈渠道

如有 API 相关问题或建议：

- **GitHub Issues**: https://github.com/silverenternal/tokitai/issues
- **讨论区**: https://github.com/silverenternal/tokitai/discussions

---

*最后更新：2026-03-10*
