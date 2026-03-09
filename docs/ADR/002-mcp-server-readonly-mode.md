# ADR 002: McpServer 只读模式

**状态**: 已接受  
**日期**: 2026 年 3 月  
**优先级**: P1

## 背景

MCP (Model Context Protocol) 服务器需要向 AI 暴露工具定义，并处理 AI 的工具调用请求。

## 决策

`McpServer` 采用**只读模式**设计：

- ✅ `GET /tools` - 返回工具定义列表
- ✅ `GET /health` - 健康检查
- ❌ `POST /call` - 返回 `501 Not Implemented`

## 原因

1. **安全性**: 只读服务器可以安全地暴露工具定义，而无需担心未授权的工具调用。

2. **职责分离**: 工具调用逻辑应该由应用层处理，而不是 MCP 服务器本身。

3. **灵活性**: 用户可以根据需要选择是否启用工具调用端点，使用 `McpServerWithProvider` 来提供完整功能。

## 设计

```rust
// 只读服务器（默认）
let server = McpServer::new(config);
server.run().await?;  // /call 端点返回 501

// 完整功能服务器（带工具提供者）
let provider = Calculator::default();
let server = McpServerWithProvider::new(config, provider);
server.run().await?;  // /call 端点正常工作
```

## 后果

### 正面
- ✅ 默认安全（只读）
- ✅ 清晰的职责分离
- ✅ 用户可以选择是否需要完整功能

### 负面
- ❌ 可能需要额外文档说明两种服务器的区别
- ❌ 用户可能需要额外步骤来启用工具调用

## 替代方案

1. **默认启用 /call 端点**: 会带来安全风险，用户可能无意中暴露工具调用接口。

2. **通过配置开关**: 增加配置复杂度，不如使用不同的类型清晰。

## 使用示例

### 只读模式（文档/发现服务）

```rust
use tokitai_mcp_server::{McpServer, McpServerConfig};

let config = McpServerConfig::builder()
    .host("localhost")
    .port(3000)
    .build();

let server = McpServer::new(config);
server.run().await?;
```

### 完整功能（带工具提供者）

```rust
use tokitai_mcp_server::{McpServerWithProvider, McpServerConfig};

let config = McpServerConfig::builder()
    .host("localhost")
    .port(3000)
    .build();

let provider = Calculator::default();
let server = McpServerWithProvider::new(config, provider);
server.run().await?;
```

## 参考

- `McpServer` 实现：`tokitai-mcp-server/src/server.rs`
- MCP 架构文档：`docs/MCP_ARCHITECTURE.md`
