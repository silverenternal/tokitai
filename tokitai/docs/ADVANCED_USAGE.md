# Tokitai 高级用法指南

本文档介绍 Tokitai 的高级功能和最佳实践。

## 目录

- [#[tool(skip)] 排除方法](#toolskip-排除方法)
- [同步与异步工具](#同步与异步工具)
- [自定义错误类型](#自定义错误类型)
- [复杂类型支持](#复杂类型支持)
- [多工具组合](#多工具组合)
- [call_tool 返回值处理](#call_tool-返回值处理)
- [性能优化建议](#性能优化建议)

---

## #[tool(skip)] 排除方法

默认情况下，`#[tool]` impl 块中的所有 `pub` 方法都会被暴露给 AI。如果你希望排除某些方法（如内部辅助函数、调试方法），可以使用 `#[tool(skip)]` 属性。

### 示例

```rust
use tokitai::tool;

pub struct DataProcessor {
    cache: std::collections::HashMap<String, String>,
}

#[tool]
impl DataProcessor {
    /// 处理数据并返回结果
    pub fn process(&self, input: String) -> String {
        let cached = self.get_cached(&input);
        if let Some(result) = cached {
            return result;
        }
        // 处理逻辑...
        format!("Processed: {}", input)
    }

    /// 内部缓存查找方法 - 不暴露给 AI
    #[tool(skip)]
    pub fn get_cached(&self, key: &str) -> Option<String> {
        self.cache.get(key).cloned()
    }

    /// 调试方法 - 不暴露给 AI
    #[tool(skip)]
    pub fn debug_info(&self) -> String {
        format!("Cache size: {}", self.cache.len())
    }
}
```

在这个例子中：
- `process` 方法会被暴露给 AI
- `get_cached` 和 `debug_info` 方法不会被暴露

---

## 同步与异步工具

Tokitai 支持同步和异步工具方法。宏会根据方法的 async/sync 属性自动生成对应版本的 `call_tool`。

### 同步工具

```rust
use tokitai::tool;

pub struct Calculator;

#[tool]
impl Calculator {
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}

// 同步调用
let calc = Calculator;
let result = calc.call_tool("add", &serde_json::json!({"a": 10, "b": 20}))?;
```

### 异步工具

```rust
use tokitai::tool;

pub struct DatabaseService;

#[tool]
impl DatabaseService {
    pub async fn query(&self, sql: String) -> Result<Vec<serde_json::Value>, String> {
        // 异步数据库查询
        // tokio_postgres::query(...)
        Ok(vec![])
    }
}

// 异步调用
let db = DatabaseService;
let result = db.call_tool("query", &serde_json::json!({"sql": "SELECT * FROM users"})).await?;
```

### 混合工具（同时包含同步和异步方法）

当 impl 块中同时包含同步和异步方法时，宏会生成：
- `call_tool()` - 异步版本
- `call_tool_sync()` - 同步阻塞版本（内部使用 `tokio::runtime::Handle::block_on`）

```rust
use tokitai::tool;

pub struct HybridService;

#[tool]
impl HybridService {
    // 同步方法
    pub fn compute(&self, data: Vec<i32>) -> i32 {
        data.iter().sum()
    }

    // 异步方法
    pub async fn fetch(&self, url: String) -> Result<String, String> {
        // reqwest::get(&url).await?.text().await
        Ok("data".to_string())
    }
}

// 在异步上下文中
let service = HybridService;

// 异步调用（推荐）
let result = service.call_tool("compute", &serde_json::json!({"data": [1, 2, 3]})).await?;

// 同步调用（会阻塞当前线程）
let result = service.call_tool_sync("compute", &serde_json::json!({"data": [1, 2, 3]}))?;
```

---

## 自定义错误类型

Tokitai 支持工具方法返回自定义错误类型。宏会自动处理错误转换。

### 使用 thiserror

```rust
use tokitai::tool;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum CalculatorError {
    #[error("除数不能为零")]
    DivisionByZero,
    #[error("溢出错误：{0}")]
    Overflow(String),
}

pub struct Calculator;

#[tool]
impl Calculator {
    pub fn divide(&self, a: f64, b: f64) -> Result<f64, CalculatorError> {
        if b == 0.0 {
            Err(CalculatorError::DivisionByZero)
        } else {
            Ok(a / b)
        }
    }
}

// 调用时，错误会被转换为 tokitai::ToolError
let calc = Calculator;
match calc.call_tool("divide", &serde_json::json!({"a": 10.0, "b": 0.0})) {
    Ok(_) => println!("成功"),
    Err(e) => println!("错误：{:?}", e), // ToolError::InternalError
}
```

### 错误处理最佳实践

1. **使用 Result 返回类型**：让宏自动处理错误转换
2. **提供有意义的错误消息**：帮助用户理解问题
3. **避免暴露内部实现细节**：错误消息应该对用户友好

---

## 复杂类型支持

### Option 参数

`Option<T>` 类型的参数是可选的。如果 AI 没有提供该参数，值为 `None`。

```rust
use tokitai::tool;

pub struct Greeter;

#[tool]
impl Greeter {
    /// 打招呼，language 是可选参数
    pub fn greet(&self, name: String, language: Option<String>) -> String {
        match language.as_deref() {
            Some("zh") => format!("你好，{}！", name),
            Some("es") => format!("¡Hola, {}!", name),
            _ => format!("Hello, {}!", name),
        }
    }
}

// 不带可选参数
let result = greeter.call_tool("greet", &serde_json::json!({"name": "Alice"}))?;
// 输出：Hello, Alice!

// 带可选参数
let result = greeter.call_tool("greet", &serde_json::json!({"name": "Bob", "language": "zh"}))?;
// 输出：你好，Bob!
```

### Vec 参数

```rust
use tokitai::tool;

pub struct MathService;

#[tool]
impl MathService {
    /// 计算数字列表的总和
    pub fn sum(&self, numbers: Vec<i32>) -> i32 {
        numbers.iter().sum()
    }

    /// 过滤偶数
    pub fn filter_even(&self, numbers: Vec<i32>) -> Vec<i32> {
        numbers.into_iter().filter(|n| n % 2 == 0).collect()
    }
}
```

### 自定义结构体参数

对于复杂的自定义结构体，建议使用 `serde_json::Value` 作为参数类型，然后在方法内部进行解析：

```rust
use tokitai::tool;
use serde_json::Value;

pub struct UserService;

#[derive(serde::Deserialize)]
struct CreateUserRequest {
    name: String,
    email: String,
    age: Option<i32>,
}

#[tool]
impl UserService {
    /// 创建新用户
    pub fn create_user(&self, request: Value) -> Result<Value, String> {
        let req: CreateUserRequest = serde_json::from_value(request)
            .map_err(|e| format!("参数解析错误：{}", e))?;
        
        // 处理创建逻辑...
        
        Ok(serde_json::json!({
            "id": 123,
            "name": req.name,
            "email": req.email
        }))
    }
}
```

---

## 多工具组合

在大型应用中，你可能需要组合多个工具提供者。

### 示例：个人助理系统

```rust
use tokitai::{tool, ToolProvider};
use serde_json::Value;

// 待办事项管理
pub struct TodoManager;

#[tool]
impl TodoManager {
    pub fn add_todo(&self, title: String) -> String {
        format!("已添加待办：{}", title)
    }

    pub fn list_todos(&self) -> Value {
        serde_json::json!([])
    }
}

// 笔记管理
pub struct NoteManager;

#[tool]
impl NoteManager {
    pub fn create_note(&self, content: String) -> String {
        "笔记已创建".to_string()
    }

    pub fn list_notes(&self) -> Value {
        serde_json::json!([])
    }
}

// 组合所有工具
pub struct PersonalAssistant {
    todo_manager: TodoManager,
    note_manager: NoteManager,
}

impl PersonalAssistant {
    pub fn new() -> Self {
        Self {
            todo_manager: TodoManager,
            note_manager: NoteManager,
        }
    }

    /// 获取所有工具定义（合并多个工具提供者）
    pub fn get_all_tools(&self) -> Vec<tokitai::ToolDefinition> {
        let mut tools = Vec::new();
        tools.extend_from_slice(TodoManager::tool_definitions());
        tools.extend_from_slice(NoteManager::tool_definitions());
        tools
    }

    /// 统一工具调用入口
    pub fn call_tool(&self, name: &str, args: &Value) -> Result<Value, String> {
        // 路由到对应的工具提供者
        match name {
            "add_todo" | "list_todos" => {
                self.todo_manager.call_tool(name, args)
                    .map_err(|e| e.to_string())
            }
            "create_note" | "list_notes" => {
                self.note_manager.call_tool(name, args)
                    .map_err(|e| e.to_string())
            }
            _ => Err(format!("未知工具：{}", name)),
        }
    }
}
```

---

## call_tool 返回值处理

`call_tool` 返回 `Result<serde_json::Value, ToolError>`。以下是处理返回值的几种方式：

### 直接获取值

```rust
let result = calc.call_tool("add", &json!({"a": 10, "b": 20}))?;
let sum = result.as_i64().unwrap();
println!("结果：{}", sum);
```

### 反序列化为具体类型

```rust
use serde::Deserialize;

#[derive(Deserialize)]
struct WeatherResponse {
    temperature: f64,
    condition: String,
}

let result = weather.call_tool("get_weather", &json!({"city": "北京"}))?;
let weather: WeatherResponse = serde_json::from_value(result)?;
println!("温度：{}°C", weather.temperature);
```

### 处理错误

```rust
match calculator.call_tool("divide", &json!({"a": 10, "b": 0})) {
    Ok(result) => println!("结果：{}", result),
    Err(tokitai::ToolError { kind: tokitai::ToolErrorKind::ValidationError, message }) => {
        eprintln!("参数验证错误：{}", message);
    }
    Err(tokitai::ToolError { kind: tokitai::ToolErrorKind::NotFound, message }) => {
        eprintln!("工具未找到：{}", message);
    }
    Err(e) => {
        eprintln!("内部错误：{:?}", e);
    }
}
```

---

## 性能优化建议

### 1. 避免在工具调用中创建过多临时对象

```rust
// ❌ 不推荐：每次调用都创建新的 Vec
pub fn process(&self, data: Vec<i32>) -> i32 {
    data.iter().sum()
}

// ✅ 推荐：使用切片
pub fn process(&self, data: &[i32]) -> i32 {
    data.iter().sum()
}
```

### 2. 对于计算密集型任务，考虑使用 spawn_blocking

如果在异步上下文中调用同步工具，且工具执行时间较长：

```rust
// 在异步环境中，长时间运行的同步工具可能会阻塞事件循环
// 考虑在工具内部使用 tokio::task::spawn_blocking
pub async fn heavy_computation(&self, n: i32) -> i32 {
    tokio::task::spawn_blocking(move || {
        // 计算密集型逻辑
        (0..n).sum()
    })
    .await
    .unwrap()
}
```

### 3. 缓存工具定义

工具定义在编译期生成，不需要每次调用时重新创建：

```rust
// ✅ 推荐：使用静态引用
let tools = Calculator::tool_definitions();

// ❌ 不推荐：不必要的克隆
let tools = Calculator::tool_definitions().to_vec();
```

---

## 已知限制

1. **泛型方法不支持**：工具方法不能是泛型的
2. **关联类型限制**：返回类型必须是具体类型或 `Result<T, E>`
3. **no_std 支持有限**：完整功能需要 serde 和 serde_json

---

## 故障排除

### 编译错误：`call_tool` 不是 future

如果你的工具都是同步的，`call_tool` 返回 `Result` 而不是 `Future`：

```rust
// ❌ 错误：同步方法使用 .await
let result = calc.call_tool("add", &args).await?;

// ✅ 正确：同步方法直接调用
let result = calc.call_tool("add", &args)?;
```

### 编译错误：类型推断失败

当参数类型复杂时，可能需要显式类型注解：

```rust
// ❌ 可能失败
let result = service.call_tool(name, &args)?;

// ✅ 添加类型注解
let result: serde_json::Value = service.call_tool(name, &args)?;
```

### 运行时错误：参数类型错误

确保 JSON 参数类型与 Rust 类型匹配：

```rust
// Rust: fn add(&self, a: i32, b: i32)
// ❌ 错误：浮点数
json!({"a": 10.5, "b": 20.5})

// ✅ 正确：整数
json!({"a": 10, "b": 20})
```

---

## 获取更多帮助

- [基础使用文档](docs/USAGE.md)
- [AI 集成指南](docs/AI_INTEGRATION.md)
- [GitHub Issues](https://github.com/silverenternal/tokitai/issues)
