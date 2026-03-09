# Tokitai v0.3.4 开发计划

**版本**: 0.3.4  
**目标**: 混合模式参数描述支持  
**预计工时**: 2 周

---

## 🎯 开发目标

实现三种灵活的工具描述方式：

1. **方式 1**: 文档注释自动提取（已有）
2. **方式 2**: `#[tool(desc)]` 属性覆盖（已有，需完善参数级）
3. **方式 3**: `tokitai!` 配置宏（新增）

---

## 📋 任务清单

### 任务 1: 验证并修复参数级描述传递

**文件**: `tokitai-macros/src/tool.rs`

**问题**: 参数的 `description` 字段没有正确传递到 Schema

**修复位置**:
- 第 2600 行：`generate_schema_json_with_deprecated_and_tags` 函数
- 第 2738 行：`generate_schema_for_type_with_default_and_example` 函数

**修复代码**:

```rust
// 在 generate_schema_json_with_deprecated_and_tags 中
for p in params {
    let schema_name = p.schema_name.clone();
    let mut schema = generate_schema_for_type_with_default_and_example(
        &p.ty,
        p.description.clone(),  // ← 确保这里传递了 description
        p.example.as_ref(),
        p.default.as_ref(),
    );
    
    // ✅ 添加：如果 schema 没有 description，但参数有，则使用参数的
    if schema.description().is_none() && p.description.is_some() {
        schema.set_description(p.description.clone());
    }
    
    properties.insert(schema_name.clone(), schema);
    // ...
}
```

**需要添加的辅助方法** (在 `JsonSchema` enum 中):

```rust
impl JsonSchema {
    fn description(&self) -> Option<&String> {
        match self {
            JsonSchema::Basic { description, .. } => description.as_ref(),
            JsonSchema::Array { description, .. } => description.as_ref(),
            JsonSchema::Object { description, .. } => description.as_ref(),
            _ => None,
        }
    }
    
    fn set_description(&mut self, desc: Option<String>) {
        match self {
            JsonSchema::Basic { description, .. } => *description = desc,
            JsonSchema::Array { description, .. } => *description = desc,
            JsonSchema::Object { description, .. } => *description = desc,
            _ => {}
        }
    }
}
```

---

### 任务 2: 添加参数级 `#[tool]` 属性支持

**问题**: 当前参数级的 `#[tool(...)]` 属性不被识别（测试报错）

**原因**: `#[tool]` 是 proc_macro_attribute，只能用于 impl 块和方法，不能用于参数

**解决方案**: 使用 `param_tool` 属性（代码中已存在，需启用）

**修改文件**: `tokitai-macros/src/lib.rs`

```rust
/// 参数级别的工具属性
///
/// ## 用法
///
/// ```rust,ignore
/// #[tool]
/// impl MyTools {
///     pub fn process(
///         &self,
///         #[param_tool(desc = "参数描述", example = "示例")]
///         name: String,
///     ) {}
/// }
/// ```
#[proc_macro_attribute]
pub fn param_tool(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // 这是一个 no-op 属性，由 #[tool] 宏处理
    item
}
```

**修改文件**: `tokitai-macros/src/tool.rs` (第 940-980 行)

```rust
// 在 extract_method_args 函数中
for attr in &param.attrs {
    if attr.path().is_ident("param_tool") || attr.path().is_ident("tool") {
        // ✅ 同时支持 #[param_tool(...)] 和 #[tool(...)]
        if let Meta::List(meta) = &attr.meta {
            let tokens = &meta.tokens;
            // 解析 ParamToolAttrs
            if let Ok(param_attrs) = syn::parse2::<ParamToolAttrs>(tokens.clone()) {
                // 应用到参数信息
                param_info.description = param_attrs.desc;
                param_info.example = param_attrs.example;
                param_info.default = param_attrs.default;
                // ... 其他属性
            }
        }
    }
}
```

---

### 任务 3: 新增 `tokitai!` 配置宏

**文件**: `tokitai-macros/src/lib.rs`

```rust
/// # `tokitai!` 配置宏
///
/// 用于集中配置工具属性，无需修改原有代码
///
/// ## 用法
///
/// ```rust,ignore
/// tokitai::config! {
///     MyService {
///         get_user: {
///             desc: "获取用户信息",
///             tags: ["user", "read"],
///             params: {
///                 id: {
///                     desc: "用户唯一标识",
///                     example: "1001"
///                 }
///             }
///         }
///     }
/// }
/// ```
#[proc_macro]
pub fn config(item: TokenStream) -> TokenStream {
    tool::config(item)
}
```

**文件**: `tokitai-macros/src/tool.rs` - 新增函数

```rust
pub fn config(item: TokenStream) -> TokenStream {
    // 解析配置
    let config_input = parse_macro_input!(item as ConfigInput);
    
    // 生成配置代码
    generate_config_code(&config_input)
}

struct ConfigInput {
    // 解析配置结构
    // 例如：MyService { get_user: { desc: "...", params: {...} } }
}

fn generate_config_code(config: &ConfigInput) -> TokenStream {
    // 生成代码覆盖工具描述
    quote! {
        // 在编译期修改 ToolDefinition
        // ...
    }
}
```

---

### 任务 4: 添加完整测试用例

**文件**: `tokitai-macros/tests/param_description_test.rs`

```rust
use tokitai::tool;
use serde_json::Value;

#[test]
fn test_method_doc_comment() {
    struct TestTools;
    
    #[tool]
    impl TestTools {
        /// 这是方法描述
        pub fn test_method(&self, name: String) -> String {
            format!("Hello, {}", name)
        }
    }
    
    let tool = TestTools::TOOL_DEFINITIONS[0];
    assert_eq!(tool.description, "这是方法描述");
}

#[test]
fn test_method_custom_desc() {
    struct TestTools;
    
    #[tool]
    impl TestTools {
        #[tool(desc = "自定义描述")]
        pub fn test_method(&self, name: String) -> String {
            format!("Hello, {}", name)
        }
    }
    
    let tool = TestTools::TOOL_DEFINITIONS[0];
    assert_eq!(tool.description, "自定义描述");
}

#[test]
fn test_param_description() {
    struct TestTools;
    
    #[tool]
    impl TestTools {
        pub fn test_method(
            &self,
            #[param_tool(desc = "参数 1 描述", example = "示例")]
            param1: String,
            #[param_tool(desc = "参数 2 描述", min = 0, max = 150)]
            param2: i32,
        ) -> String {
            format!("{}: {}", param1, param2)
        }
    }
    
    let tool = TestTools::TOOL_DEFINITIONS[0];
    let schema: Value = serde_json::from_str(tool.input_schema).unwrap();
    
    assert_eq!(
        schema["properties"]["param1"]["description"],
        "参数 1 描述"
    );
    assert_eq!(
        schema["properties"]["param1"]["example"],
        "示例"
    );
    assert_eq!(
        schema["properties"]["param2"]["description"],
        "参数 2 描述"
    );
}

#[test]
fn test_config_macro() {
    struct TestTools;
    
    #[tool]
    impl TestTools {
        /// 默认描述
        pub fn get_user(&self, id: i32) -> String {
            format!("User {}", id)
        }
    }
    
    // 使用配置宏覆盖
    tokitai::config! {
        TestTools {
            get_user: {
                desc: "配置覆盖的描述",
                params: {
                    id: {
                        desc: "用户 ID",
                        example: "1001"
                    }
                }
            }
        }
    }
    
    let tool = TestTools::TOOL_DEFINITIONS[0];
    assert_eq!(tool.description, "配置覆盖的描述");
    
    let schema: Value = serde_json::from_str(tool.input_schema).unwrap();
    assert_eq!(
        schema["properties"]["id"]["description"],
        "用户 ID"
    );
}
```

---

### 任务 5: 更新示例代码

**文件**: `examples/param_attrs.rs`

```rust
//! 参数属性示例
//!
//! 运行：cargo run --example param_attrs

use tokitai::tool;
use serde_json::Value;

struct ParamTools;

#[tool]
impl ParamTools {
    /// 方式 1：文档注释（最简单）
    pub fn method_with_doc(
        &self,
        name: String,  // 描述来自文档注释
        age: i32,
    ) -> String {
        format!("{} is {} years old", name, age)
    }

    /// 方式 2：#[tool] 属性覆盖方法描述
    #[tool(
        desc = "自定义方法描述",
        tags = ["demo", "test"]
    )]
    pub fn method_with_custom_desc(
        &self,
        name: String,
        age: i32,
    ) -> String {
        format!("{} is {} years old", name, age)
    }

    /// 方式 3：参数级属性
    pub fn method_with_param_attrs(
        &self,
        #[param_tool(
            desc = "用户姓名",
            example = "张三",
            min_length = 1,
            max_length = 50
        )]
        name: String,
        #[param_tool(
            desc = "用户年龄",
            example = 25,
            min = 0,
            max = 150
        )]
        age: i32,
        #[param_tool(
            desc = "邮箱地址（可选）",
            default = "null",
            pattern = r"^[a-zA-Z0-9._-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}$"
        )]
        email: Option<String>,
    ) -> String {
        format!("{} <{}>", name, email.unwrap_or_default())
    }
}

fn main() {
    println!("=== 参数属性示例 ===\n");

    for tool in ParamTools::TOOL_DEFINITIONS {
        println!("方法：{}", tool.name);
        println!("描述：{}\n", tool.description);
        println!("Schema: {}\n", pretty_json(tool.input_schema));
    }
}

fn pretty_json(json_str: &str) -> String {
    let value: Value = serde_json::from_str(json_str).unwrap();
    serde_json::to_string_pretty(&value).unwrap()
}
```

---

### 任务 6: 更新文档

**文件**: `docs/USAGE.md` - 新增章节

```markdown
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

    pub fn update_profile(
        &self,
        #[param_tool(
            desc = "用户 ID",
            example = "12345",
            required
        )]
        id: i32,
        #[param_tool(
            desc = "用户昵称",
            min_length = 2,
            max_length = 20,
            pattern = r"^[a-zA-Z\u4e00-\u9fa5]+$"
        )]
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
3. 参数级：`#[param_tool]` > 默认推断

## 最佳实践

- **简单场景**: 使用文档注释
- **复杂参数**: 使用 `#[param_tool]`
- **批量管理**: 使用 `tokitai!` 配置宏
```

---

## 📅 开发时间表

| 周次 | 任务 | 交付物 |
|------|------|--------|
| Week 1 | 任务 1-2：参数描述修复 | 可运行的参数级描述 |
| Week 1 | 任务 3：配置宏实现 | `tokitai!` 宏可用 |
| Week 2 | 任务 4-5：测试和示例 | 完整测试覆盖 + 示例 |
| Week 2 | 任务 6：文档更新 | 用户文档 + 迁移指南 |

---

## ✅ 验收标准

1. **功能验收**:
   - [ ] 三种方式都能正确传递描述给 AI
   - [ ] Schema 中包含参数 description 字段
   - [ ] 配置宏能覆盖默认描述

2. **测试验收**:
   - [ ] 单元测试覆盖率 > 90%
   - [ ] 集成测试通过
   - [ ] 示例代码可运行

3. **文档验收**:
   - [ ] 用户文档完整
   - [ ] 迁移指南清晰
   - [ ] API 文档更新

---

## 🔗 相关文件

- `tokitai-macros/src/tool.rs` - 核心实现
- `tokitai-macros/src/lib.rs` - 宏导出
- `examples/param_attrs.rs` - 示例代码
- `docs/USAGE.md` - 使用文档
