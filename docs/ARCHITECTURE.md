# Tokitai 架构设计

**版本**: 0.4.0 | **最后更新**: 2026-03-10

本文档描述 Tokitai 的内部设计和工作原理。

---

## 目录

1. [整体架构](#整体架构)
2. [核心组件](#核心组件)
3. [宏展开原理](#宏展开原理)
4. [编译期代码生成](#编译期代码生成)
5. [运行时调用流程](#运行时调用流程)
6. [设计决策](#设计决策)

---

## 整体架构

```
┌─────────────────────────────────────────────────────────────┐
│                     用户代码层                               │
│                                                             │
│   #[tool]                                                   │
│   impl MyTools {                                            │
│       pub fn my_method(&self, arg: String) -> String { }    │
│   }                                                         │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                   宏处理层 (tokitai-macros)                  │
│                                                             │
│   1. 解析 impl 块和 pub 方法                                   │
│   2. 提取方法签名和文档注释                                   │
│   3. 生成 ToolDefinition const                              │
│   4. 生成 call_tool 分发器                                   │
│   5. 生成参数解析辅助函数                                    │
└─────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────┐
│                   核心类型层 (tokitai-core)                  │
│                                                             │
│   - ToolDefinition: 工具定义                                │
│   - ToolProvider: 工具提供者 trait                          │
│   - ToolError: 错误类型                                     │
│   - ParamType: 参数类型枚举                                  │
└─────────────────────────────────────────────────────────────┘
```

---

## 核心组件

### tokitai-macros

**职责**: 过程宏实现，编译期代码生成

**输入**:
```rust
#[tool]
impl Calculator {
    /// 两个数相加
    pub fn add(&self, a: i32, b: i32) -> i32 {
        a + b
    }
}
```

**输出**:
```rust
impl Calculator {
    // ① 工具定义数组（v0.4.0+ 现在是方法）
    pub fn tool_definitions() -> &'static [ToolDefinition] {
        &[
            ToolDefinition {
                name: "add",
                description: "两个数相加",
                input_schema: "{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"integer\"},\"b\":{\"type\":\"integer\"}},\"required\":[\"a\",\"b\"]}",
            }
        ]
    }

    // ② call_tool 分发器
    fn call_tool(&self, name: &str, args: &Value) -> Result<Value, ToolError> {
        match name {
            "add" => {
                let a: i32 = parse_arg(args, "a")?;
                let b: i32 = parse_arg(args, "b")?;
                let result = self.add(a, b);
                Ok(json!(result))
            }
            _ => Err(ToolError::not_found(name)),
        }
    }

    // ③ 参数解析辅助函数
    fn parse_arg<T: DeserializeOwned>(args: &Value, key: &str) -> Result<T, ToolError> {
        // ...
    }
}
```

### tokitai-core

**职责**: 核心类型定义，零运行时依赖

**类型**:

| 类型 | 描述 |
|------|------|
| `ToolDefinition` | 工具定义，包含名称、描述、输入 schema |
| `ToolProvider` | 工具提供者 trait |
| `ToolError` | 工具调用错误 |
| `ToolErrorKind` | 错误类型枚举 |
| `ParamType` | 参数类型枚举 |

### tokitai

**职责**: 运行时库（可选），提供完整功能

**特性**:
- `default`: 启用完整运行时
- `serde`: serde 序列化支持

---

## 宏展开原理

### 解析阶段

1. **解析 impl 块**
   ```rust
   let impl_item: ItemImpl = syn::parse(item)?;
   ```

2. **收集 pub 方法**
   ```rust
   for item in &impl_item.items {
       if let ImplItem::Fn(fn_item) = item {
           if is_public(fn_item) && !is_skipped(fn_item) {
               tools.push(extract_tool_info(fn_item));
           }
       }
   }
   ```

3. **提取方法信息**
   - 方法名
   - 参数列表（类型、名称）
   - 返回类型
   - 文档注释
   - 异步/同步标记

### 代码生成阶段

#### 1. 生成 ToolDefinition

```rust
fn generate_tool_def(tool: &ToolMethodInfo) -> TokenStream2 {
    let name = &tool.tool_name;
    let desc = &tool.description;
    let schema = generate_json_schema(&tool.params);
    
    quote! {
        ToolDefinition {
            name: #name,
            description: #desc,
            input_schema: #schema,
        }
    }
}
```

#### 2. 生成 JSON Schema

```rust
fn generate_json_schema(params: &[ParamInfo]) -> String {
    let mut schema = String::from("{\"type\":\"object\",\"properties\":{");
    
    for param in params {
        schema.push_str(&format!(
            "\"{}\":{{\"type\":\"{}\"}}",
            param.name,
            rust_type_to_json_type(&param.ty)
        ));
    }
    
    schema.push_str("},\"required\":[");
    
    for param in params.iter().filter(|p| !p.is_option) {
        schema.push_str(&format!("\"{}\",", param.name));
    }
    
    schema.push_str("]}");
    schema
}
```

#### 3. 生成 call_tool 分发器

```rust
fn generate_call_tool(tools: &[ToolMethodInfo]) -> TokenStream2 {
    let match_arms = tools.iter().map(|tool| {
        let name = &tool.tool_name;
        let method_name = &tool.name;
        let param_parsing = generate_param_parsing(&tool.params);
        
        quote! {
            #name => {
                #param_parsing
                let result = self.#method_name(#(#param_names),*);
                Ok(json!(result))
            }
        }
    });
    
    quote! {
        match name {
            #(#match_arms)*
            _ => Err(ToolError::not_found(name)),
        }
    }
}
```

---

## 编译期代码生成

### 示例：完整展开

**输入代码**:
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

**展开后代码** (简化版):
```rust
pub struct Calculator;

impl Calculator {
    /// 工具定义数组（编译期生成，v0.4.0+ 现在是方法）
    pub fn tool_definitions() -> &'static [tokitai::ToolDefinition] {
        &[
            tokitai::ToolDefinition {
                name: "add",
                description: "两个数相加",
                input_schema: "{\"type\":\"object\",\"properties\":{\"a\":{\"type\":\"integer\"},\"b\":{\"type\":\"integer\"}},\"required\":[\"a\",\"b\"]}",
            }
        ]
    }

    /// 工具调用分发器
    pub fn call_tool(
        &self,
        name: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, tokitai::ToolError> {
        match name {
            "add" => {
                // 参数解析
                let a: i32 = serde_json::from_value(
                    args.get("a").ok_or_else(|| {
                        tokitai::ToolError::validation_error("缺少参数 'a'")
                    })?.clone()
                ).map_err(|e| {
                    tokitai::ToolError::validation_error(
                        format!("参数 'a' 类型错误：{}", e)
                    )
                })?;

                let b: i32 = serde_json::from_value(
                    args.get("b").ok_or_else(|| {
                        tokitai::ToolError::validation_error("缺少参数 'b'")
                    })?.clone()
                ).map_err(|e| {
                    tokitai::ToolError::validation_error(
                        format!("参数 'b' 类型错误：{}", e)
                    )
                })?;

                // 调用原始方法
                let result = self.add(a, b);

                // 返回 JSON
                Ok(serde_json::json!(result))
            }
            _ => Err(tokitai::ToolError::not_found(
                format!("未知工具：{}", name)
            )),
        }
    }
}

impl tokitai::ToolProvider for Calculator {
    fn tool_definitions() -> &'static [tokitai::ToolDefinition] {
        Self::tool_definitions()
    }
}
```

---

## 运行时调用流程

### 完整调用链路

```
用户请求："计算 100 + 250"
         ↓
┌─────────────────────────────────┐
│ 1. AI 分析请求并决定调用工具       │
│    → 返回：{"name": "add",      │
│              "arguments":       │
│              {"a": 100,         │
│               "b": 250}}        │
└─────────────────────────────────┘
         ↓
┌─────────────────────────────────┐
│ 2. 调用 call_tool               │
│    calc.call_tool("add", args)  │
└─────────────────────────────────┘
         ↓
┌─────────────────────────────────┐
│ 3. 分发器匹配工具名称             │
│    match name { "add" => ... }  │
└─────────────────────────────────┘
         ↓
┌─────────────────────────────────┐
│ 4. 解析 JSON 参数                 │
│    a: i32 = 100                 │
│    b: i32 = 250                 │
└─────────────────────────────────┘
         ↓
┌─────────────────────────────────┐
│ 5. 调用原始 Rust 方法              │
│    self.add(100, 250)           │
│    → 返回：350                  │
└─────────────────────────────────┘
         ↓
┌─────────────────────────────────┐
│ 6. 包装结果为 JSON               │
│    json!(350) → 350             │
└─────────────────────────────────┘
         ↓
┌─────────────────────────────────┐
│ 7. 返回给调用者                  │
│    Ok(350)                      │
└─────────────────────────────────┘
```

---

## 设计决策

### 1. 为什么使用编译期代码生成？

**优点**:
- ✅ 类型安全：编译期检查类型错误
- ✅ 零运行时开销：无需反射或动态类型检查
- ✅ 更好的 IDE 支持：代码补全、跳转
- ✅ 文档自动生成：rustdoc 可以直接提取

**缺点**:
- ❌ 宏调试困难：需要 `cargo expand` 查看展开代码
- ❌ 编译时间略长：需要处理宏

### 2. 为什么使用 `&'static str` 存储 schema？

**原因**:
- 编译期生成，生命周期为 `'static`
- 避免运行时分配和序列化开销
- 工具定义在程序整个生命周期内不变

### 3. 为什么 `call_tool` 返回 `Result<Value, ToolError>`？

**原因**:
- `Result`: 标准错误处理方式
- `Value`: 灵活返回任何 JSON 可序列化的值
- `ToolError`: 提供结构化错误信息

### 4. 为什么不直接暴露方法，而是通过 `call_tool` 分发？

**原因**:
- 统一接口：AI 调用时使用统一格式
- 参数验证：在分发器中统一处理参数解析
- 错误处理：统一的错误类型和消息

### 5. 为什么支持异步方法？

**原因**:
- 实际场景中很多操作是异步的（数据库、网络请求）
- tokio 是 Rust 异步生态的事实标准
- 宏可以自动生成异步分发代码

---

## 性能考虑

### 编译期 vs 运行时

| 操作 | 编译期 | 运行时 |
|------|--------|--------|
| JSON Schema 生成 | ✅ | ❌ |
| 工具定义注册 | ✅ | ❌ |
| 参数类型检查 | 部分 | ✅ |
| 方法分发 | ❌ | ✅ |

### 内存占用

- `ToolDefinition`: 约 100 字节/工具（编译期常量，不占用运行时内存）
- `call_tool` 分发器：约 1KB 代码/工具
- 参数解析：栈上分配，无堆分配

---

## 未来计划

### 短期 (v0.4)

- [ ] 支持泛型方法（通过具体化）
- [ ] 支持工具分组和命名空间
- [ ] 改进错误消息，提供更详细的修复建议

### 中期 (v0.5)

- [ ] MCP (Model Context Protocol) 完整支持
- [ ] 工具版本控制
- [ ] 工具依赖关系声明

### 长期 (v1.0)

- [ ] 支持更多参数验证选项
- [ ] 工具性能分析工具
- [ ] IDE 插件支持

---

## 相关链接

- [使用指南](USAGE.md) - 详细使用说明
- [API 文档](https://docs.rs/tokitai) - Rust API 参考
- [GitHub](https://github.com/silverenternal/tokitai) - 源代码
