# Tokitai 入门项目

这是一个完整的 Tokitai 入门项目模板，演示如何从零开始构建 AI 工具集成应用。

## 快速开始

### 1. 运行项目

```bash
cd examples/starter_project
cargo run
```

### 2. 项目结构

```
starter_project/
├── Cargo.toml              # 项目配置
├── src/
│   ├── main.rs             # 主程序入口
│   ├── ai_client.rs        # AI 客户端模块
│   └── tools/
│       ├── mod.rs          # 工具模块导出
│       ├── weather.rs      # 天气查询工具
│       └── calculator.rs   # 计算器工具
└── skills/
    └── tools.md            # 工具说明文档
```

### 3. 添加你自己的工具

1. 在 `src/tools/` 目录下创建新的 `.rs` 文件
2. 使用 `#[tool]` 宏标记你的 impl 块
3. 在 `mod.rs` 中导出新模块
4. 在 `main.rs` 中使用

示例：

```rust
// src/tools/my_tool.rs
use tokitai::tool;

pub struct MyTool;

#[tool]
impl MyTool {
    /// 我的自定义功能
    pub fn do_something(&self, input: String) -> String {
        format!("处理结果：{}", input)
    }
}
```

### 4. 集成真实 AI

参考 [`docs/AI_INTEGRATION.md`](../../docs/AI_INTEGRATION.md) 了解如何集成：
- Ollama（本地 AI）
- Claude API
- OpenAI API
- 其他 AI 服务

## 学习资源

- [5 分钟快速上手](../../README.md#5-分钟快速上手)
- [完整使用指南](../../docs/USAGE.md)
- [Skill 文件模板](../../docs/SKILL_TEMPLATE.md)
- [API 文档](https://docs.rs/tokitai)

## 许可证

MIT
