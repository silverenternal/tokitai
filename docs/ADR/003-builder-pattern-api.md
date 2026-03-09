# ADR 003: Builder Pattern API

**状态**: 已接受  
**日期**: 2026 年 3 月  
**优先级**: P1

## 背景

MCP 服务器需要灵活的配置方式，同时保持 API 的简洁性和类型安全性。

## 决策

使用 **Builder Pattern** 作为 MCP 服务器的主要配置方式：

```rust
use tokitai_mcp_server::{McpServerBuilder, McpServerConfig};

// 方式 1: 使用 Builder
let server = McpServerBuilder::new()
    .host("localhost")
    .port(3000)
    .cors_enabled(true)
    .tracing_enabled(true)
    .build_readonly();

// 方式 2: 使用配置对象
let config = McpServerConfig::builder()
    .host("localhost")
    .port(3000)
    .build();
let server = McpServer::new(config);
```

## 原因

1. **可读性**: Builder 模式提供流畅的 API，配置项一目了然。

2. **类型安全**: 编译期检查配置项，避免运行时错误。

3. **可扩展性**: 未来添加新配置项时，不会破坏现有 API。

4. **一致性**: 与 Rust 生态系统中的其他库（如 `tokio`、`hyper`）保持一致。

## 设计

### Builder API

```rust
pub struct McpServerBuilder {
    config: McpServerConfig,
}

impl McpServerBuilder {
    pub fn new() -> Self;
    pub fn host(mut self, host: impl Into<String>) -> Self;
    pub fn port(mut self, port: u16) -> Self;
    pub fn cors_enabled(mut self, enabled: bool) -> Self;
    pub fn tracing_enabled(mut self, enabled: bool) -> Self;
    pub fn build(self) -> McpServer;
    pub fn build_with_provider<T>(self, provider: T) -> McpServerWithProvider<T>;
}
```

### 配置对象

```rust
pub struct McpServerConfig {
    pub host: String,
    pub port: u16,
    pub cors_enabled: bool,
    pub tracing_enabled: bool,
}
```

## 后果

### 正面
- ✅ API 清晰易读
- ✅ 编译期检查配置
- ✅ 易于扩展新配置项

### 负面
- ❌ 相比直接构造函数，代码量略多
- ❌ 需要维护 Builder 和 Config 两套 API

## 替代方案

1. **直接构造函数**: `McpServer::new(host, port, cors, tracing)` - 参数过多，不易读。

2. **配置结构体**: `McpServer::new(McpServerConfig { host, port, .. })` - 需要用户手动构造配置。

3. **环境变量**: 不够灵活，不适合动态配置。

## 使用示例

### 基础用法

```rust
let server = McpServerBuilder::new()
    .port(3000)
    .build_readonly();
```

### 完整配置

```rust
let server = McpServerBuilder::new()
    .host("0.0.0.0")
    .port(8080)
    .cors_enabled(true)
    .tracing_enabled(true)
    .build_readonly();
```

### 带工具提供者

```rust
let server = McpServerBuilder::new()
    .port(3000)
    .build_with_provider(Calculator::default());
```

## 参考

- Builder 实现：`tokitai-mcp-server/src/server.rs`
- 使用示例：`tokitai-mcp-server/examples/mcp_builder_demo.rs`
