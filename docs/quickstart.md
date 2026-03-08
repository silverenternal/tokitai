# 5 分钟快速开始

## 1. 添加依赖

```toml
[dependencies]
tokitai = "0.3.3"
```

就这一行！所有必需的依赖（serde、serde_json、thiserror）都会自动包含。

## 2. 定义工具

```rust
use tokitai::tool;

#[tool]
struct Calculator;

#[tool]
impl Calculator {
    /// 两个数相加
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }

    /// 两个数相乘
    pub fn multiply(&self, a: i32, b: i32) -> i32 {
        a * b
    }
}
```

## 3. 获取工具定义（发送给 AI）

```rust
// 编译期生成的工具定义
let tools = Calculator::TOOL_DEFINITIONS;

// 转换为 JSON 发送给 AI
let json = tokitai::json!({
    "tools": tools.iter().map(|t| {
        tokitai::json!({
            "name": t.name,
            "description": t.description,
            "input_schema": t.input_schema
        })
    }).collect::<Vec<_>>()
});

println!("{}", serde_json::to_string_pretty(&json)?);
```

输出：
```json
[
  {
    "name": "add",
    "description": "两个数相加",
    "input_schema": "{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"integer\"},\"b\":{\"type\":\"integer\"}},\"required\":[\"a\",\"b\"]}"
  },
  {
    "name": "multiply",
    "description": "两个数相乘",
    "input_schema": "{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"integer\"},\"b\":{\"type\":\"integer\"}},\"required\":[\"a\",\"b\"]}"
  }
]
```

## 4. 处理 AI 调用

```rust
use tokitai::json;

let calc = Calculator;

// AI 返回工具调用请求
let call_request = json!({
    "name": "add",
    "arguments": {"a": 10, "b": 20}
});

// 执行工具调用
let result = calc.call_tool(
    call_request["name"].as_str().unwrap(),
    &call_request["arguments"]
)?;

println!("结果：{}", result);  // 30
```

## 完整示例

运行示例查看效果：

```bash
# 基础使用示例
cargo run --example basic_usage

# 快速聊天示例（交互式）
cargo run --example quick_chat
```

## 下一步

- [完整 API 文档](https://docs.rs/tokitai)
- [更多示例](https://github.com/silverenternal/tokitai/tree/main/examples)
- [属性参考](docs/attributes.md)
- [类型映射](docs/types.md)

## 常见问题

### 为什么只需要一个 `#[tool]` 属性？

tokitai 在编译期分析你的代码，自动生成所有必需的元数据。不需要额外的 `#[tool_name]`、`#[tool_description]` 等属性。

### Option 类型参数的警告？

如果参数是 `Option<T>` 类型，建议添加默认值或示例，这样 AI 知道可以不传这个参数：

```rust
#[tool]
impl MyTools {
    // 添加默认值
    pub fn process(&self, data: String, #[tool(default = "null")] options: Option<Value>) {
        // ...
    }
    
    // 或者改为必填
    pub fn process(&self, data: String, options: Value) {
        // ...
    }
}
```

### 如何自定义工具名称？

```rust
#[tool]
impl MyTools {
    #[tool(name = "custom_name")]
    pub fn my_function(&self, x: i32) -> i32 {
        x * 2
    }
}
```

### 支持哪些类型？

Rust 类型自动映射到 JSON Schema：

| Rust 类型 | JSON Schema |
|-----------|-------------|
| `String`, `&str` | `string` |
| `i32`, `i64`, `u32` 等 | `integer` |
| `f32`, `f64` | `number` |
| `bool` | `boolean` |
| `Vec<T>` | `array` |
| 自定义 struct | `object` |
