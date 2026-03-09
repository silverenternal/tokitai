# 从 LangChain 迁移到 Tokitai

本指南帮助 Python/LangChain 开发者快速迁移到 Rust/Tokitai，享受编译期类型安全和零运行时性能优势。

## 为什么要迁移？

| 维度 | LangChain (Python) | Tokitai (Rust) |
|------|-------------------|----------------|
| **类型安全** | 运行时错误 | 编译期检查 |
| **性能** | 解释执行，慢 | 编译优化，快 10-100x |
| **部署** | 依赖管理复杂 | 单一二进制文件 |
| **并发** | GIL 限制 | 原生异步支持 |
| **内存** | GC 开销 | 零成本抽象 |

## 核心概念映射

```
LangChain Python          →    Tokitai Rust
─────────────────────────────────────────────
@tool                     →    #[tool]
Tool(name, description)   →    ToolDefinition
BaseTool abstract class   →    trait ToolProvider
tool decorator args      →    #[tool(...)] attributes
```

## 快速迁移示例

### 1. 基础工具定义

#### Python (LangChain)

```python
from langchain.tools import tool

@tool
def search(query: str) -> str:
    """Search for information about a topic.
    
    Args:
        query: The search query string.
    
    Returns:
        Search results as a string.
    """
    return f"Search results for: {query}"

@tool
def calculator(expression: str) -> float:
    """Evaluate a mathematical expression.
    
    Args:
        expression: The mathematical expression (e.g., "2 + 2").
    
    Returns:
        The result of the calculation.
    """
    return eval(expression)
```

#### Rust (Tokitai)

```rust
use tokitai::tool;

#[tool]
impl SearchTools {
    /// Search for information about a topic
    ///
    /// **Parameters:**
    /// - `query` - The search query string
    ///
    /// **Returns:**
    /// Search results as a string
    #[tool(desc = "Search for information")]
    pub fn search(&self, query: String) -> String {
        format!("Search results for: {}", query)
    }
    
    /// Evaluate a mathematical expression
    ///
    /// **Parameters:**
    /// - `expression` - The mathematical expression (e.g., "2 + 2")
    ///
    /// **Returns:**
    /// The result of the calculation
    #[tool(desc = "Calculate expression")]
    pub fn calculator(&self, expression: String) -> f64 {
        // 安全实现（不使用 eval！）
        meval::eval_str(&expression).unwrap_or(0.0)
    }
}
```

### 2. 带参数的工具

#### Python (LangChain)

```python
@tool
def weather(city: str, units: str = "celsius") -> dict:
    """Get current weather for a city.
    
    Args:
        city: The city name.
        units: Temperature units (celsius or fahrenheit).
    
    Returns:
        Dictionary with temperature and conditions.
    """
    return {
        "city": city,
        "temperature": 25 if units == "celsius" else 77,
        "conditions": "Sunny"
    }
```

#### Rust (Tokitai)

```rust
use tokitai::tool;
use serde::Serialize;

#[derive(Serialize)]
struct WeatherResult {
    city: String,
    temperature: f64,
    conditions: String,
}

#[tool]
impl WeatherTools {
    /// Get current weather for a city
    ///
    /// **Parameters:**
    /// - `city` - The city name
    /// - `units` - Temperature units ("celsius" or "fahrenheit", default: "celsius")
    ///
    /// **Returns:**
    /// Weather information with temperature and conditions
    #[tool(desc = "Get current weather")]
    pub fn weather(
        &self,
        city: String,
        units: Option<String>,
    ) -> WeatherResult {
        let temp = if units.as_deref() == Some("fahrenheit") { 77.0 } else { 25.0 };
        WeatherResult {
            city,
            temperature: temp,
            conditions: "Sunny".to_string(),
        }
    }
}
```

### 3. 异步工具

#### Python (LangChain)

```python
@tool
async def fetch_url(url: str) -> str:
    """Fetch content from a URL asynchronously.
    
    Args:
        url: The URL to fetch.
    
    Returns:
        The page content as text.
    """
    async with aiohttp.ClientSession() as session:
        async with session.get(url) as response:
            return await response.text()
```

#### Rust (Tokitai)

```rust
use tokitai::tool;

#[tool]
impl WebTools {
    /// Fetch content from a URL asynchronously
    ///
    /// **Parameters:**
    /// - `url` - The URL to fetch
    ///
    /// **Returns:**
    /// The page content as text
    #[tool(desc = "Fetch URL content", context = "async")]
    pub async fn fetch_url(&self, url: String) -> Result<String, reqwest::Error> {
        reqwest::get(url)
            .await?
            .text()
            .await
    }
}
```

### 4. 带验证的工具

#### Python (LangChain)

```python
from pydantic import BaseModel, Field

class SearchInput(BaseModel):
    query: str = Field(..., description="The search query")
    max_results: int = Field(10, ge=1, le=100, description="Maximum results to return")

@tool(args_schema=SearchInput)
def smart_search(query: str, max_results: int = 10) -> list:
    """Smart search with configurable result limit.
    
    Args:
        query: The search query.
        max_results: Maximum number of results (1-100).
    
    Returns:
        List of search results.
    """
    return [f"Result {i+1} for {query}" for i in range(max_results)]
```

#### Rust (Tokitai)

```rust
use tokitai::tool;

#[tool]
impl SearchTools {
    /// Smart search with configurable result limit
    ///
    /// **Parameters:**
    /// - `query` - The search query
    /// - `max_results` - Maximum number of results (1-100, default: 10)
    ///
    /// **Returns:**
    /// List of search results
    #[tool(
        desc = "Smart search",
        default_max_results = "10",
        validate_max_results = "if val < 1 || val > 100 { return Err(\"max_results must be 1-100\".to_string()); }"
    )]
    pub fn smart_search(
        &self,
        query: String,
        max_results: Option<usize>,
    ) -> Vec<String> {
        let limit = max_results.unwrap_or(10).min(100);
        (1..=limit)
            .map(|i| format!("Result {} for {}", i, query))
            .collect()
    }
}
```

### 5. 结构化输出

#### Python (LangChain)

```python
from typing import List, Optional
from pydantic import BaseModel

class Person(BaseModel):
    name: str
    age: int
    email: Optional[str] = None

@tool
def create_person(name: str, age: int, email: Optional[str] = None) -> Person:
    """Create a person record.
    
    Args:
        name: Person's name.
        age: Person's age.
        email: Optional email address.
    
    Returns:
        Person object with validated data.
    """
    return Person(name=name, age=age, email=email)
```

#### Rust (Tokitai)

```rust
use tokitai::tool;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct Person {
    name: String,
    age: u32,
    email: Option<String>,
}

#[tool]
impl PersonTools {
    /// Create a person record
    ///
    /// **Parameters:**
    /// - `name` - Person's name
    /// - `age` - Person's age
    /// - `email` - Optional email address
    ///
    /// **Returns:**
    /// Person object with validated data
    #[tool(desc = "Create person record")]
    pub fn create_person(
        &self,
        name: String,
        age: u32,
        email: Option<String>,
    ) -> Person {
        Person { name, age, email }
    }
}
```

## 属性映射表

| LangChain Python | Tokitai Rust | 说明 |
|-----------------|--------------|------|
| `@tool` | `#[tool]` | 基本工具标记 |
| `args_schema=Schema` | Rust 类型系统 | 自动从参数类型推导 |
| `return_direct=True` | (默认行为) | 直接返回结果 |
| 自定义 description | `#[tool(desc = "...")]` | 工具描述 |
| 参数验证 | `#[tool(validate_xxx = "...")]` | 参数验证规则 |
| 默认值 | `#[tool(default_xxx = "...")]` | 参数默认值 |
| 示例 | `#[tool(example_xxx = "...")]` | 参数示例值 |
| 废弃标记 | `#[tool(deprecated, replaced_by = "...")]` | 废弃工具 |

## 完整迁移示例

### Python (LangChain) 完整示例

```python
from langchain.tools import tool
from typing import List, Optional
import asyncio
import aiohttp

class WeatherAPI:
    """Weather API client."""
    
    async def fetch(self, city: str) -> dict:
        """Fetch weather data."""
        # Simulated API call
        return {
            "city": city,
            "temp": 25,
            "humidity": 60,
            "conditions": "Sunny"
        }

@tool
def get_weather(city: str, units: str = "celsius") -> dict:
    """Get current weather for a city.
    
    Args:
        city: The city name.
        units: Temperature units (celsius or fahrenheit).
    
    Returns:
        Weather data dictionary.
    """
    api = WeatherAPI()
    data = asyncio.run(api.fetch(city))
    
    if units == "fahrenheit":
        data["temp"] = data["temp"] * 9/5 + 32
    
    return data

@tool
async def get_weather_async(city: str) -> dict:
    """Get current weather asynchronously.
    
    Args:
        city: The city name.
    
    Returns:
        Weather data dictionary.
    """
    api = WeatherAPI()
    return await api.fetch(city)

# 使用示例
tools = [get_weather, get_weather_async]
```

### Rust (Tokitai) 完整示例

```rust
use tokitai::tool;
use serde::Serialize;
use reqwest;

#[derive(Serialize)]
struct WeatherData {
    city: String,
    temp: f64,
    humidity: u32,
    conditions: String,
}

struct WeatherAPI;

impl WeatherAPI {
    async fn fetch(&self, city: &str) -> Result<WeatherData, reqwest::Error> {
        // Simulated API call
        Ok(WeatherData {
            city: city.to_string(),
            temp: 25.0,
            humidity: 60,
            conditions: "Sunny".to_string(),
        })
    }
}

#[tool]
impl WeatherTools {
    /// Get current weather for a city
    ///
    /// **Parameters:**
    /// - `city` - The city name
    /// - `units` - Temperature units ("celsius" or "fahrenheit", default: "celsius")
    ///
    /// **Returns:**
    /// Weather data with temperature, humidity, and conditions
    #[tool(desc = "Get current weather")]
    pub fn get_weather(
        &self,
        city: String,
        units: Option<String>,
    ) -> WeatherData {
        let api = WeatherAPI;
        let mut data = tokio::runtime::Handle::current()
            .block_on(api.fetch(&city))
            .unwrap();
        
        if units.as_deref() == Some("fahrenheit") {
            data.temp = data.temp * 9.0 / 5.0 + 32.0;
        }
        
        data
    }
    
    /// Get current weather asynchronously
    ///
    /// **Parameters:**
    /// - `city` - The city name
    ///
    /// **Returns:**
    /// Weather data dictionary
    #[tool(desc = "Get weather async", context = "async")]
    pub async fn get_weather_async(&self, city: String) -> Result<WeatherData, reqwest::Error> {
        let api = WeatherAPI;
        api.fetch(&city).await
    }
}

// 使用示例
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tools = WeatherTools;

    // 获取工具定义（用于 AI）
    let definitions = tools.tool_definitions();
    
    // 调用工具
    let result = tools.call_tool("get_weather", &tokitai::json!({
        "city": "Beijing",
        "units": "celsius"
    }))?;
    
    println!("{:?}", result);
    
    Ok(())
}
```

## 性能对比

| 操作 | LangChain (Python) | Tokitai (Rust) | 提升 |
|------|-------------------|----------------|------|
| 工具调用延迟 | ~1-5ms | ~0.01-0.1ms | **10-50x** |
| JSON 序列化 | ~0.5ms | ~0.05ms | **10x** |
| 内存占用 | ~50MB | ~5MB | **10x** |
| 启动时间 | ~500ms | ~10ms | **50x** |

## 迁移步骤

### 第 1 步：安装 Rust

```bash
# Linux/macOS
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Windows
# 下载并运行 rustup-init.exe from https://rustup.rs
```

### 第 2 步：创建 Rust 项目

```bash
cargo new my_ai_tools
cd my_ai_tools
```

### 第 3 步：添加依赖

```toml
[dependencies]
tokitai = "0.3.4"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
tokio = { version = "1.0", features = ["full"] }  # 异步支持
reqwest = { version = "0.11", features = ["json"] }  # HTTP 客户端
```

### 第 4 步：迁移工具代码

按照上面的示例，将 Python 工具逐个转换为 Rust。

### 第 5 步：测试

```bash
# 编译检查
cargo build

# 运行测试
cargo test

# 生成文档
cargo doc --open
```

### 第 6 步：集成到 AI 应用

```rust
use tokitai::{ToolProvider, json};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let tools = MyTools;

    // 获取所有工具定义（发送给 AI）
    let definitions = tools.tool_definitions();
    let definitions_json = serde_json::to_string_pretty(&definitions)?;
    
    // 发送给 AI（如 OpenAI、Anthropic 等）
    // ... AI 调用逻辑 ...
    
    // 处理 AI 的工具调用请求
    let tool_name = "my_tool";
    let args = json!({"param": "value"});
    let result = tools.call_tool(tool_name, &args)?;
    
    println!("Result: {}", result);
    
    Ok(())
}
```

## 常见问题

### Q: Python 的 `Optional[T]` 如何映射？

A: 使用 `Option<T>`：
```rust
// Python: name: Optional[str] = None
// Rust:
pub fn my_tool(&self, name: Option<String>) {}
```

### Q: Python 的 `List[T]` 如何映射？

A: 使用 `Vec<T>`：
```rust
// Python: items: List[str]
// Rust:
pub fn my_tool(&self, items: Vec<String>) {}
```

### Q: Python 的 `Dict[K, V]` 如何映射？

A: 使用 `HashMap<K, V>` 或 `serde_json::Value`：
```rust
use std::collections::HashMap;

// Python: metadata: Dict[str, Any]
// Rust:
pub fn my_tool(&self, metadata: HashMap<String, serde_json::Value>) {}
```

### Q: 如何处理 Python 的 `**kwargs`？

A: 使用 `serde_json::Value`：
```rust
// Python: def my_tool(**kwargs)
// Rust:
pub fn my_tool(&self, args: serde_json::Value) {
    // 访问 args["param_name"]
}
```

### Q: 如何迁移 Pydantic 模型？

A: 使用 Rust struct + serde：
```rust
// Python:
// class Person(BaseModel):
//     name: str
//     age: int

// Rust:
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct Person {
    name: String,
    age: u32,
}
```

## 资源

- [Tokitai 快速开始](docs/quickstart.md)
- [Tokitai 高级用法](docs/ADVANCED_USAGE.md)
- [Rust 编程语言](https://doc.rust-lang.org/book/)
- [LangChain Python 文档](https://python.langchain.com/)

## 获取帮助

遇到问题？
- 查看 [GitHub Issues](https://github.com/silverenternal/tokitai/issues)
- 加入 Rust 中文社区
- 提交问题报告
