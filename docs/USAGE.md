# Tokitai 使用指南

**版本**: 0.3.4 | **最后更新**: 2026-03-08

## 目录

1. [快速开始](#快速开始)
2. [安装配置](#安装配置)
3. [基础用法](#基础用法)
4. [高级特性](#高级特性)
5. [工具描述三种方式](#工具描述三种方式)
6. [API 参考](#api-参考)
7. [故障排除](#故障排除)
8. [最佳实践](#最佳实践)

---

## 快速开始

### 1. 添加依赖

```toml
[dependencies]
tokitai = "0.3.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 2. 定义工具

```rust
use tokitai::tool;

pub struct Calculator;

#[tool]
impl Calculator {
    /// 两个数相加
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
```

### 3. 使用工具

```rust
let calc = Calculator;

// 获取工具定义（发送给 AI）
let tools = Calculator::tool_definitions();

// 调用工具（接收 AI 的请求）
let result = calc.call_tool("add", &serde_json::json!({"a": 10, "b": 20}))?;
```

---

## 安装配置

### 标准安装

```toml
[dependencies]
tokitai = "0.3.3"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### 最小化安装

```toml
[dependencies]
tokitai = { version = "0.3.3", default-features = false }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
```

### Features 说明

| Feature | 描述 | 依赖 |
|---------|------|------|
| `default` | 启用完整运行时 | `serde`, `serde_json`, `thiserror` |
| `serde` | serde 序列化支持 | `serde`, `serde_json` |

### 依赖版本要求

| 依赖 | 最低版本 | 推荐版本 |
|------|---------|---------|
| Rust | 1.56.0 | 1.75.0+ |
| serde | 1.0 | 1.0.130+ |
| serde_json | 1.0 | 1.0.70+ |

---

## 基础用法

### 自动注册方法

在 `impl` 块上使用 `#[tool]` 宏，所有 `pub` 方法会自动注册为工具：

```rust
use tokitai::tool;

pub struct WeatherService;

#[tool]
impl WeatherService {
    /// 获取指定城市的天气
    pub fn get_weather(&self, city: String) -> String {
        format!("{} 的天气：晴朗", city)
    }
}
```

### 自定义工具属性

使用 `#[tool(name = "...", desc = "...")]` 自定义工具名称和描述：

```rust
#[tool]
impl WeatherService {
    #[tool(name = "fetch_weather", desc = "从外部 API 获取天气数据")]
    pub fn get_weather(&self, city: String) -> String {
        // 调用外部 API...
    }
}
```

### 排除方法

使用 `#[tool(skip)]` 排除内部方法：

```rust
#[tool]
impl WeatherService {
    pub fn get_weather(&self, city: String) -> String {
        self.fetch_from_api(city)
    }

    #[tool(skip)]
    fn fetch_from_api(&self, city: &str) -> String {
        // 内部实现，不暴露给 AI
        "API response".to_string()
    }
}
```

### 支持的方法签名

#### 同步方法

```rust
#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
```

#### 异步方法

```rust
#[tool]
impl Database {
    pub async fn query(&self, sql: String) -> Result<Vec<Row>, DbError> {
        // 异步数据库查询...
    }
}
```

**注意**: 异步方法需要使用 `call_tool().await` 调用。

#### 返回 Result

```rust
#[tool]
impl Parser {
    pub fn parse(&self, input: String) -> Result<Data, ParseError> {
        // 可能失败的操作...
    }
}
```

---

## 高级特性

### 参数类型详解

#### 基本类型

| Rust 类型 | JSON Schema | 示例值 |
|-----------|-------------|--------|
| `String`, `&str` | `string` | `"hello"` |
| `i8`..=`i128` | `integer` | `42` |
| `u8`..=`u128` | `integer` | `42` |
| `f32`, `f64` | `number` | `3.14` |
| `bool` | `boolean` | `true` |

#### 复合类型

| Rust 类型 | JSON Schema | 示例值 |
|-----------|-------------|--------|
| `Vec<T>` | `array` | `[1, 2, 3]` |
| `Option<T>` | 可选参数 | `null` 或值 |
| `HashMap<K, V>` | `object` | `{"key": "value"}` |
| 自定义类型 | `object` | `{"field": "value"}` |

#### 自定义类型

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
pub struct Location {
    pub latitude: f64,
    pub longitude: f64,
}

#[tool]
impl MapService {
    pub fn get_weather_at(&self, location: Location) -> String {
        format!("位置 ({}, {}) 的天气", location.latitude, location.longitude)
    }
}
```

### 可选参数

```rust
#[tool]
impl SearchEngine {
    /// 搜索文档
    pub fn search(
        &self,
        query: String,
        limit: Option<i32>,  // 可选参数
        offset: Option<i32>, // 可选参数
    ) -> Vec<Document> {
        // ...
    }
}
```

### 获取工具定义

```rust
use tokitai::ToolProvider;

// 获取所有工具定义
let tools = Calculator::tool_definitions();

// 获取工具数量
let count = Calculator::tool_count();

// 查找特定工具
if let Some(tool) = Calculator::find_tool("add") {
    println!("找到工具：{}", tool.name);
}
```

### 工具调用

#### 同步调用

```rust
use serde_json::json;

let calc = Calculator;
let result = calc.call_tool("add", &json!({"a": 10, "b": 20}));
```

#### 异步调用

```rust
let calc = Calculator;
let result = calc.call_tool("query", &json!({"sql": "SELECT *"})).await;
```

#### 错误处理

```rust
use tokitai::ToolError;

match calc.call_tool("divide", &json!({"a": 10, "b": 0})) {
    Ok(result) => println!("结果：{}", result),
    Err(ToolError { kind: tokitai::ToolErrorKind::ValidationError, message }) => {
        eprintln!("验证错误：{}", message);
    }
    Err(ToolError { kind: tokitai::ToolErrorKind::NotFound, message }) => {
        eprintln!("工具未找到：{}", message);
    }
    Err(e) => eprintln!("其他错误：{:?}", e),
}
```

---

## API 参考

### 核心类型

#### `ToolDefinition`

工具定义结构体，包含工具的元信息。

```rust
pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: &'static str,
}
```

**方法**:

| 方法 | 描述 |
|------|------|
| `new(name, description, input_schema)` | 创建新的工具定义 |
| `to_json()` | 转换为 JSON 字符串（需要 `serde` 特性） |
| `to_value()` | 转换为 JSON Value（需要 `serde` 特性） |

#### `ToolProvider`

工具提供者 trait，由 `#[tool]` 宏自动实现。

```rust
pub trait ToolProvider {
    fn tool_definitions() -> &'static [ToolDefinition];
    
    fn tool_count() -> usize {
        Self::tool_definitions().len()
    }
    
    fn find_tool(name: &str) -> Option<&'static ToolDefinition> {
        Self::tool_definitions()
            .iter()
            .find(|t| t.name == name)
    }
}
```

#### `ToolError`

工具调用错误类型。

```rust
pub struct ToolError {
    pub kind: ToolErrorKind,
    pub message: String,
}
```

**错误类型**:

| 变体 | 值 | 描述 |
|------|-----|------|
| `ToolErrorKind::ValidationError` | 0 | 参数验证失败 |
| `ToolErrorKind::NotFound` | 1 | 工具未找到 |
| `ToolErrorKind::InternalError` | 2 | 内部错误 |
| `ToolErrorKind::TypeError` | 3 | 类型错误 |

### 宏属性

#### `#[tool]` (impl 块级别)

应用于 `impl` 块，启用工具注册。

```rust
#[tool]
impl MyStruct {
    // 所有 pub 方法自动注册为工具
}
```

#### `#[tool(name = "...", desc = "...")]` (方法级别)

自定义工具名称和描述。

```rust
#[tool]
impl MyStruct {
    #[tool(name = "custom_name", desc = "自定义描述")]
    pub fn my_method(&self) {}
}
```

#### `#[tool(skip)]` (方法级别)

排除方法，不注册为工具。

```rust
#[tool]
impl MyStruct {
    #[tool(skip)]
    fn internal_helper(&self) {}
}
```

---

## 工具描述三种方式

Tokitai v0.3.4 支持三种灵活的工具描述方式：

### 方式 1：文档注释（推荐）

最简单的方式，使用 Rust 标准文档注释：

```rust
#[tool]
impl MyService {
    /// 获取用户信息
    ///
    /// # Parameters
    /// - `id`: 用户 ID
    /// - `include_profile`: 是否包含详细信息
    pub fn get_user(
        &self,
        id: i32,
        include_profile: Option<bool>,
    ) -> User {
        // ...
    }
}
```

**优点**:
- ✅ 标准 Rust 风格
- ✅ IDE 友好
- ✅ 无需额外学习

**缺点**:
- ❌ 描述不能包含特殊字符（如引号）
- ❌ 无法添加 tags 等元数据

### 方式 2：`#[tool]` 属性覆盖

更精确的控制：

```rust
#[tool]
impl MyService {
    #[tool(
        desc = "从数据库获取用户详细信息",
        tags = ["user", "read", "database"],
        group = "user_service",
        cache = "ttl=300"
    )]
    pub fn get_user_detail(&self, user_id: i32) -> User {
        // ...
    }

    /// 更新用户资料
    ///
    /// @param id 用户 ID
    /// @param nickname 用户昵称
    #[tool(
        example_id = "12345",
        min_length_nickname = 2,
        max_length_nickname = 20,
        pattern_nickname = r"^[a-zA-Z\u4e00-\u9fa5]+$"
    )]
    pub fn update_profile(
        &self,
        id: i32,
        nickname: String,
    ) -> Result<(), Error> {
        // ...
    }
}
```

**优点**:
- ✅ 支持 tags、group 等元数据
- ✅ 参数级精确控制
- ✅ 支持验证规则

**缺点**:
- ❌ 代码稍显冗长

### 方式 3：`tokitai!` 配置宏

批量集中管理：

```rust
// 原有代码完全不变
impl MyService {
    /// 默认描述
    pub fn get_user(&self, id: i32) -> User {
        // ...原有业务逻辑
    }
}

// 在入口处统一配置
tokitai::config! {
    MyService {
        get_user: {
            desc: "配置覆盖的描述",
            tags = ["user", "read"],
            params: {
                id: {
                    desc: "用户唯一标识",
                    example: "1001"
                }
            }
        }
    }
}
```

**优点**:
- ✅ 原有代码 0 修改
- ✅ 集中管理所有工具
- ✅ 支持条件编译

**缺点**:
- ❌ 需要额外配置文件

## 优先级

三种方式可以混合使用，优先级如下：

1. `#[tool(desc = "...")]` > 文档注释
2. `tokitai!` 配置 > `#[tool]` 属性
3. 参数级：`#[tool(xxx_param = "...")]` > 默认推断

## 最佳实践

- **简单场景**: 使用文档注释
- **复杂参数**: 使用 `#[tool(...)]` 参数级属性
- **批量管理**: 使用 `tokitai!` 配置宏

---

## 故障排除

### 编译错误

#### 错误：泛型方法不支持

```
error: Generic methods are not supported
  = help: Remove generic parameters or use concrete types
```

**原因**: `#[tool]` 宏不支持泛型方法。

**解决方案**: 使用具体类型替代泛型。

```rust
// ❌ 错误
#[tool]
impl MyTools {
    pub fn process<T: Serialize>(&self, data: T) -> String {
        // ...
    }
}

// ✅ 正确
#[tool]
impl MyTools {
    pub fn process_string(&self, data: String) -> String {
        // ...
    }
    
    pub fn process_json(&self, data: serde_json::Value) -> String {
        // ...
    }
}
```

#### 错误：缺少 serde 特性

```
error[E0433]: failed to resolve: use of undeclared crate or module `serde_json`
```

**原因**: 未启用 `serde` 特性。

**解决方案**: 在 `Cargo.toml` 中启用特性。

```toml
[dependencies]
tokitai = { version = "0.3.3", features = ["serde"] }
serde_json = "1.0"
```

### 运行时错误

#### 错误：异步方法在没有运行时的情况下调用

```
Error: 异步工具调用需要 tokio 运行时
```

**原因**: 异步工具方法需要 tokio 运行时。

**解决方案**: 使用 `#[tokio::main]` 或 `tokio::runtime::Runtime`。

```rust
#[tokio::main]
async fn main() {
    let calc = Calculator;
    let result = calc.call_tool("async_method", &args).await;
}
```

#### 错误：工具未找到

```
Error: 工具未找到：unknown_tool
```

**原因**: 调用了不存在的工具名称。

**解决方案**: 检查工具名称是否正确。

```rust
// 打印所有可用工具
for tool in MyTools::tool_definitions() {
    println!("可用工具：{}", tool.name);
}
```

### 常见问题

#### Q: 如何调试工具调用？

**A**: 启用日志记录：

```rust
// Cargo.toml
[dependencies]
env_logger = "0.10"

// main.rs
fn main() {
    env_logger::init();
    // ...
}
```

#### Q: 如何查看宏生成的代码？

**A**: 使用 `cargo expand`:

```bash
cargo install cargo-expand
cargo expand --example basic_usage
```

#### Q: 支持哪些 AI 平台？

**A**: Tokitai 生成的工具定义是通用的，兼容任何支持 Function Calling 的 AI 平台：

- Ollama (本地/云端)
- Claude
- GPT-4
- 其他 OpenAI 兼容平台

---

## 最佳实践

### 1. 工具命名

- 使用动词 + 名词格式：`get_weather`, `create_user`, `delete_file`
- 避免缩写，除非是通用缩写
- 保持命名一致性

### 2. 文档注释

为每个工具方法添加清晰的文档注释：

```rust
#[tool]
impl Calculator {
    /// 计算两个整数的和，返回结果
    ///
    /// # 参数
    /// - `a`: 第一个整数
    /// - `b`: 第二个整数
    ///
    /// # 返回
    /// 返回两个整数的和
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
```

### 3. 错误处理

使用 `Result` 类型返回可能的错误：

```rust
#[tool]
impl FileService {
    pub fn read_file(&self, path: String) -> Result<String, String> {
        std::fs::read_to_string(&path)
            .map_err(|e| format!("读取文件失败：{}", e))
    }
}
```

### 4. 工具分组

将相关工具组织在同一个 struct 中：

```rust
// 用户管理工具
pub struct UserService { /* ... */ }

#[tool]
impl UserService {
    pub fn create_user(&self, name: String) -> User { /* ... */ }
    pub fn get_user(&self, id: i32) -> Option<User> { /* ... */ }
    pub fn delete_user(&self, id: i32) -> bool { /* ... */ }
}
```

### 5. 性能考虑

- 工具方法应尽量轻量，避免长时间阻塞
- 对于耗时操作，使用异步方法
- 考虑使用缓存减少重复计算

---

## 示例代码

更多示例请查看：

- [`examples/basic_usage.rs`](../examples/basic_usage.rs) - 基础使用示例
- [`examples/ollama_integration.rs`](../examples/ollama_integration.rs) - Ollama AI 集成
- [`examples/starter_project/`](../examples/starter_project/) - 完整项目模板

---

## 相关链接

- [README](../README.md) - 项目主页
- [AI 集成指南](AI_INTEGRATION.md) - 与 AI 平台集成
- [架构设计](ARCHITECTURE.md) - 内部设计说明
- [API 文档](https://docs.rs/tokitai) - Rust API 参考
