# 贡献指南

感谢你考虑为 Tokitai 做出贡献！

## 快速导航

- [报告 Bug](#报告-bug)
- [功能建议](#功能建议)
- [开发环境设置](#开发环境设置)
- [提交代码](#提交代码)
- [代码风格](#代码风格)
- [测试](#测试)
- [文档](#文档)

---

## 报告 Bug

我们使用 GitHub Issues 来追踪 Bug。

### 提交 Bug 报告前请检查

1. 搜索现有 Issues，确认没有重复报告
2. 确认 Bug 在最新版本中仍然存在
3. 准备最小可复现代码示例

### Bug 报告模板

```markdown
**描述问题**
简明扼要地描述问题是什么。

**复现步骤**
1. 创建项目...
2. 添加代码...
3. 运行 `cargo run`...
4. 看到错误...

**期望行为**
清晰描述你期望发生什么。

**实际行为**
描述实际发生了什么。

**环境信息**
- Rust 版本：`rustc --version`
- Tokitai 版本：`cargo tree | grep tokitai`
- 操作系统：Windows/Linux/macOS

**代码示例**
```rust
// 最小可复现代码
```

**附加信息**
任何截图、日志或其他相关信息。
```

---

## 功能建议

我们欢迎功能建议！请创建一个 Issue 并包含：

1. **功能描述**：简明扼要地描述你的建议
2. **使用场景**：描述这个功能的使用场景
3. **替代方案**：描述你考虑过的其他解决方案
4. **额外信息**：任何相关的代码示例或参考资料

---

## 开发环境设置

### 1. 克隆仓库

```bash
git clone https://github.com/silverenternal/tokitai.git
cd tokitai
```

### 2. 安装 Rust 工具链

```bash
# 安装 rustup（如果尚未安装）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 安装最新稳定版 Rust
rustup update stable

# 验证安装
rustc --version
cargo --version
```

### 3. 安装开发工具（推荐）

```bash
# 代码格式化
rustup component add rustfmt

# Clippy linting
rustup component add clippy

# 查看宏展开
cargo install cargo-expand

# 文档生成
cargo install cargo-doc
```

### 4. 构建项目

```bash
# 构建整个 workspace
cargo build --workspace

# 运行测试
cargo test --workspace

# 运行 Clippy
cargo clippy --workspace --all-targets -- -D warnings
```

---

## 提交代码

### 1. 创建分支

```bash
git checkout -b feature/your-feature-name
# 或
git checkout -b fix/issue-123
```

### 2. 编写代码

遵循 [代码风格](#代码风格) 指南。

### 3. 运行测试

```bash
# 确保所有测试通过
cargo test --workspace

# 确保 Clippy 无警告
cargo clippy --workspace --all-targets -- -D warnings

# 确保代码格式化
cargo fmt --all -- --check
```

### 4. 提交更改

```bash
git add .
git commit -m "feat: 添加新功能

详细描述新功能的作用和使用方法。

Fixes #123"
```

### 5. 提交 Pull Request

1. Push 到你的分支：`git push origin feature/your-feature-name`
2. 在 GitHub 上创建 Pull Request
3. 填写 PR 描述，关联相关 Issue

---

## 代码风格

### Rust 代码风格

我们遵循 [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)：

1. **格式化**：使用 `cargo fmt` 自动格式化
2. **命名**：
   - 类型、Trait：`PascalCase`
   - 函数、变量：`snake_case`
   - 常量、宏：`SCREAMING_SNAKE_CASE`
3. **文档**：所有公共 API 必须有文档注释
4. **错误处理**：使用 `Result` 类型，避免 `unwrap()`

### 提交信息风格

我们使用 [Conventional Commits](https://www.conventionalcommits.org/)：

```
<type>(<scope>): <description>

[optional body]

[optional footer]
```

**Type 类型：**

| 类型 | 描述 |
|------|------|
| `feat` | 新功能 |
| `fix` | Bug 修复 |
| `docs` | 文档更新 |
| `style` | 代码格式化（不影响功能） |
| `refactor` | 重构（不添加功能或修复 Bug） |
| `perf` | 性能优化 |
| `test` | 测试相关 |
| `chore` | 构建、配置、工具等 |

**示例：**

```
feat: 添加异步工具调用支持

- 支持 async fn 作为工具方法
- 自动生成异步 call_tool 方法
- 添加异步示例代码

Fixes #45
```

---

## 测试

### 运行测试

```bash
# 运行所有测试
cargo test --workspace

# 运行特定测试
cargo test --package tokitai-macros test_basic_tool

# 运行 UI 测试
cargo test --test ui_tests

# 生成测试覆盖率报告（需要 cargo-tarpaulin）
cargo tarpaulin --workspace --out Html
```

### 编写测试

#### 单元测试

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

#### 集成测试

```rust
// tests/integration_test.rs
use tokitai::tool;

#[test]
fn test_tool_macro() {
    // 测试宏生成代码
}
```

#### UI 测试

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

## 文档

### 代码文档

所有公共 API 必须有文档注释：

```rust
/// 工具定义 - 描述一个 AI 可调用的工具
///
/// # 字段
///
/// * `name` - 工具名称
/// * `description` - 工具描述
/// * `input_schema` - 输入参数 JSON Schema
///
/// # 示例
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

### 文档测试

确保文档中的代码示例可以编译和运行：

```bash
# 运行文档测试
cargo test --doc
```

### 更新文档

修改代码时，请同步更新：

1. 代码文档注释
2. README.md
3. docs/ 目录下的详细文档
4. 示例代码

---

## 发布流程

### 发布新版本

1. 更新 `CHANGELOG.md`
2. 更新 `Cargo.toml` 版本号
3. 运行完整测试：`cargo test --workspace`
4. 运行 Clippy：`cargo clippy --workspace --all-targets -- -D warnings`
5. 构建文档：`cargo doc --workspace --no-deps`
6. 提交并打 Tag：`git tag -a v0.3.0 -m "Release v0.3.0"`
7. 发布到 crates.io：`cargo publish`

---

## 问题？

如有任何问题，欢迎：

- 创建 [Discussion](https://github.com/silverenternal/tokitai/discussions)
- 发送邮件至 3147264070@qq.com
- 在 Issue 中提问

---

## 行为准则

我们遵循 [Rust 行为准则](https://www.rust-lang.org/policies/code-of-conduct)。请：

- 保持开放和包容
- 尊重不同观点
- 优雅地接受建设性批评
- 关注对社区最有利的事情
- 对其他社区成员表示同理心

---

感谢你为 Tokitai 做出贡献！🎉
