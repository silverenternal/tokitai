# ADR 004: MultiToolProvider Design

**状态**: 已接受  
**日期**: 2026 年 3 月  
**优先级**: P1

## 背景

用户需要组合多个工具提供者（例如：Calculator + WeatherService + TimeService）到一个统一的接口中。

## 决策

设计 `MultiToolProvider` 来组合多个工具提供者：

```rust
use tokitai_mcp_server::MultiToolProvider;

let mut provider = MultiToolProvider::new();
provider.add(Calculator::default());
provider.add(WeatherService::default());
provider.add(TimeService::default());

let server = McpServerBuilder::new()
    .port(3000)
    .build_with_provider(provider);
```

## 设计细节

### 核心结构

```rust
pub struct MultiToolProvider {
    providers: Vec<Box<dyn ToolProvider + ToolCaller + Send + Sync + 'static>>,
    tool_defs: Vec<mcp::McpTool>,
}
```

### 关键方法

```rust
impl MultiToolProvider {
    pub fn new() -> Self;
    pub fn add<T>(&mut self, tool: T) where T: ToolProvider + ToolCaller + ...;
    pub fn tool_definitions(&self) -> &[mcp::McpTool];
    pub fn clone_definitions(&self) -> Self;  // 只克隆定义，不克隆实现
}
```

## 设计决策

### 1. 为什么不实现 Clone trait？

**问题**: `MultiToolProvider` 存储 trait 对象 (`Box<dyn Trait>`)，这些对象无法克隆。

**决策**: 不实现 `Clone` trait，改用显式方法 `clone_definitions()`。

**原因**:
- `Clone` trait 的语义是产生一个功能等价的副本
- 但 trait 对象无法克隆，只能克隆定义（metadata）
- 显式方法名清楚地告知用户：只会克隆定义，不会克隆实现

```rust
// ❌ 不实现 Clone trait
// impl Clone for MultiToolProvider { ... }

// ✅ 使用显式方法
impl MultiToolProvider {
    pub fn clone_definitions(&self) -> Self {
        // 只克隆 tool_defs，providers 为空
    }
}
```

### 2. 为什么使用类型判断而非 RuntimeToolProvider trait？

**问题**: 需要特殊处理 `MultiToolProvider` 的工具定义获取。

**决策**: 使用 `Any::downcast_ref` 做类型判断，而非引入新 trait。

**原因**:
- `RuntimeToolProvider` trait 只会有一个实现者（`MultiToolProvider`）
- 引入这样的 trait 是"为了抽象而抽象"
- 类型判断更直接，代码更清晰

```rust
fn get_tools_from_provider_runtime<T>(provider: &T) -> Vec<mcp::McpTool>
where
    T: ToolProvider + ToolCaller + Send + Sync + 'static,
{
    use std::any::Any;
    if let Some(multi) = (provider as &dyn Any).downcast_ref::<MultiToolProvider>() {
        return multi.tool_definitions().to_vec();
    }
    Vec::new()
}
```

### 3. 工具调用路由

`MultiToolProvider` 在运行时路由工具调用到对应的提供者：

```rust
impl ToolCaller for MultiToolProvider {
    fn call_tool(&self, name: &str, args: &Value) -> Result<Value, ToolError> {
        for provider in &self.providers {
            // 检查该提供者是否有这个工具
            if provider.tool_definitions().iter().any(|t| t.name == name) {
                return provider.call_tool(name, args);
            }
        }
        Err(ToolError::ToolNotFound(name.to_string()))
    }
}
```

## 后果

### 正面
- ✅ 支持组合任意数量的工具提供者
- ✅ 类型安全的工具调用路由
- ✅ 清晰的 API 语义（`clone_definitions` vs `Clone`）

### 负面
- ❌ `MultiToolProvider` 在运行时会遍历所有提供者查找工具
- ❌ 工具调用有轻微的运行时开销（遍历查找）

## 性能优化建议

1. **工具缓存**: 可以添加 `HashMap<String, usize>` 缓存工具名称到提供者索引的映射。

2. **懒加载**: 只在第一次调用时构建缓存。

## 使用示例

```rust
use tokitai_mcp_server::{MultiToolProvider, McpServerBuilder};

// 创建组合提供者
let mut provider = MultiToolProvider::new();
provider.add(Calculator::default());
provider.add(WeatherService::default());

// 启动服务器
let server = McpServerBuilder::new()
    .port(3000)
    .build_with_provider(provider);

server.run().await?;
```

## 参考

- `MultiToolProvider` 实现：`tokitai-mcp-server/src/server.rs`
- 使用示例：`tokitai-mcp-server/examples/mcp_builder_demo.rs`
- 集成测试：`tokitai-mcp-server/tests/integration_test.rs`
