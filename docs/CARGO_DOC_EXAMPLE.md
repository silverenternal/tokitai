# Cargo Doc 示例 - 生成美观的 API 文档

Tokitai 支持使用标准的 `cargo doc` 命令生成完整的 API 文档。本文档展示如何配置和自定义文档。

## 快速开始

### 生成文档

```bash
# 生成本地文档
cargo doc --open

# 生成包含私有项的文档（开发时 useful）
cargo doc --open --document-private-items

# 仅生成公共 API 文档
cargo doc --no-deps --open
```

### 文档位置

生成的文档位于：
- `target/doc/tokitai/index.html` - 主库文档
- `target/doc/tokitai_macros/index.html` - 宏文档
- `target/doc/tokitai_core/index.html` - 核心类型文档

## 文档注释最佳实践

### 1. 工具方法文档

```rust
use tokitai::tool;

#[tool]
impl WeatherTools {
    /// 获取指定城市的当前天气
    ///
    /// # Parameters
    /// - `city` - 城市名称（如 "Beijing", "Shanghai"）
    /// - `units` - 温度单位（"celsius" 或 "fahrenheit"，默认 "celsius"）
    ///
    /// # Returns
    /// 包含温度、湿度、天气状况的详细信息
    ///
    /// # Example
    /// ```
    /// let weather = WeatherTools;
    /// let result = weather.get_weather("Beijing", "celsius");
    /// // 返回：{ "temp": 25, "humidity": 60, "condition": "Sunny" }
    /// ```
    ///
    /// # Notes
    /// - 数据来源于 OpenWeatherMap API
    /// - 缓存时间为 10 分钟
    #[tool(desc = "获取城市当前天气")]
    pub fn get_weather(
        &self,
        city: String,
        units: Option<String>,
    ) -> WeatherResult {
        // 实现...
    }
}
```

### 2. 支持 Markdown 格式

Tokitai 的文档注释支持完整的 Markdown 格式：

```rust
#[tool]
impl DataTools {
    /// 数据**分析**工具 - 支持 *italic*, **bold**, `code`
    ///
    /// 这是一个多段落描述。
    /// 
    /// 第二段可以包含更多信息。
    ///
    /// ## 功能特性
    /// - ✅ 实时数据处理
    /// - ✅ 支持多种格式
    /// - ✅ 自动缓存
    ///
    /// ## 性能说明
    /// 平均响应时间 < 100ms，P99 < 500ms
    #[tool(desc = "数据分析工具")]
    pub fn analyze_data(&self, query: String) -> AnalysisResult {
        // 实现...
    }
}
```

### 3. 代码块示例

```rust
#[tool]
impl CodeTools {
    /// 执行代码片段
    ///
    /// ```python
    /// def fibonacci(n):
    ///     if n <= 1:
    ///         return n
    ///     return fibonacci(n-1) + fibonacci(n-2)
    /// ```
    ///
    /// ```javascript
    /// const add = (a, b) => a + b;
    /// console.log(add(2, 3)); // 5
    /// ```
    #[tool(desc = "执行代码片段")]
    pub fn execute_code(&self, language: String, code: String) -> ExecutionResult {
        // 实现...
    }
}
```

## 自定义文档配置

### Cargo.toml 配置

```toml
[package.metadata.docs.rs]
# 在 docs.rs 上生成文档时的配置
all-features = true
rustdoc-args = ["--cfg", "docsrs"]

[package.metadata.cargo-doc]
# 本地文档生成配置
document-private-items = false
```

### 添加 Logo 和样式

创建 `.cargo/config.toml`:

```toml
[build]
rustdocflags = ["--html-in-header", "docs/rustdoc-header.html"]
```

创建 `docs/rustdoc-header.html`:

```html
<style>
    .logo-container {
        text-align: center;
        padding: 20px;
        background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
        color: white;
    }
    .logo {
        font-size: 2em;
        font-weight: bold;
    }
</style>
<div class="logo-container">
    <div class="logo">🔮 Tokitai</div>
    <div>AI Tool Integration for Rust</div>
</div>
```

## 文档示例输出

### 工具定义文档

生成的文档会包含：

1. **方法签名** - 完整的 Rust 签名
2. **描述** - 从 doc comment 提取的描述
3. **参数** - 参数列表和类型
4. **返回值** - 返回值类型和描述
5. **示例** - 代码示例
6. **JSON Schema** - 自动生成的输入/输出 schema

### 示例截图

```
┌────────────────────────────────────────────────────────────┐
│ WeatherTools                                               │
├────────────────────────────────────────────────────────────┤
│                                                            │
│ get_weather                                                │
│ ───────────                                                │
│ pub fn get_weather(&self, city: String, units: Option<     │
│ String>) -> WeatherResult                                  │
│                                                            │
│ 获取指定城市的当前天气                                     │
│                                                            │
│ ## Parameters                                              │
│ - `city` - 城市名称（如 "Beijing", "Shanghai"）            │
│ - `units` - 温度单位（"celsius" 或 "fahrenheit"）          │
│                                                            │
│ ## Returns                                                 │
│ 包含温度、湿度、天气状况的详细信息                         │
│                                                            │
│ ## Example                                                 │
│ ```rust                                                    │
│ let result = weather.get_weather("Beijing", "celsius");    │
│ ```                                                        │
│                                                            │
│ ## JSON Schema                                             │
│ ```json                                                    │
│ {                                                          │
│   "type": "object",                                        │
│   "properties": {                                          │
│     "city": { "type": "string" },                          │
│     "units": { "type": "string" }                          │
│   },                                                       │
│   "required": ["city"]                                     │
│ }                                                          │
│ ```                                                        │
└────────────────────────────────────────────────────────────┘
```

## 在线文档

Tokitai 的文档自动发布到：
- **[docs.rs/tokitai](https://docs.rs/tokitai)** - 最新稳定版文档
- **[GitHub Pages](https://silverenternal.github.io/tokitai/)** - 开发版文档

## 故障排除

### 问题：文档中没有显示工具描述

**解决：** 确保方法有 doc comment（`///`），并且 `#[tool]` 属性在 doc comment 之后：

```rust
// ✅ 正确
/// 这是描述
#[tool]
pub fn my_method(&self) {}

// ❌ 错误
#[tool]
/// 这是描述
pub fn my_method(&self) {}
```

### 问题：Markdown 格式没有渲染

**解决：** 确保使用标准的 Markdown 语法：
- 使用 `**bold**` 而不是 `__bold__`
- 使用 `*italic*` 而不是 `_italic_`
- 代码块使用 ` ``` ` 包裹

### 问题：链接失效

**解决：** 使用 Rust 文档链接语法：
```rust
/// 参见 [`ToolDefinition`](crate::ToolDefinition) 了解更多
```

## 进阶技巧

### 1. 条件文档

```rust
#[cfg(feature = "advanced")]
#[tool]
impl AdvancedTools {
    /// 高级功能（需要 `advanced` 特性）
    #[tool]
    pub fn advanced_feature(&self) {}
}
```

### 2. 废弃标记

```rust
#[tool]
impl LegacyTools {
    /// 🚫 已废弃：使用 [`new_method`](Self::new_method) 替代
    ///
    /// 此方法将在 v0.4.0 中移除。
    #[tool(deprecated, replaced_by = "new_method")]
    pub fn old_method(&self) {}
    
    /// 新的推荐方法
    pub fn new_method(&self) {}
}
```

### 3. 跨 crate 链接

```rust
/// 使用 [`tokitai::ToolProvider`] 获取工具定义
pub fn get_tools() {}
```

## 资源

- [Rustdoc 官方文档](https://doc.rust-lang.org/rustdoc/)
- [cargo-doc 配置](https://doc.rust-lang.org/cargo/commands/cargo-doc.html)
- [docs.rs 自定义文档](https://docs.rs/about)
