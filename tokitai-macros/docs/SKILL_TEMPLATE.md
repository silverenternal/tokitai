# Skill 文件模板

Skill 文件是一份简洁的 Markdown 文档，用于向 AI 或团队成员说明你的工具集。

## 用途

1. **发给 AI 作为工具说明** - 比 JSON schema 更易读
2. **团队内部文档** - 快速了解项目提供了哪些工具
3. **代码审查参考** - 审查工具设计是否合理

---

## 模板

```markdown
# 我的工具集

简要描述你的工具集用途，例如：
> 这套工具用于处理文件转换、数据分析和自动化任务。

## 工具列表

### get_weather
- **描述**: 查询指定城市的天气
- **参数**:
  - `city` (string, 必需): 城市名称
- **返回**: 天气描述字符串
- **示例**:
  ```json
  {"city": "北京"}
  ```
  返回：`"北京 晴朗，25°C"`

### add_todo
- **描述**: 添加待办事项
- **参数**:
  - `title` (string, 必需): 待办标题
  - `priority` (string, 可选): 优先级 (high/medium/low)，默认 medium
- **返回**: 确认消息
- **示例**:
  ```json
  {"title": "学习 Tokitai", "priority": "high"}
  ```
  返回：`"已添加待办：学习 Tokitai（优先级：high）"`

### calculate
- **描述**: 执行数学计算
- **参数**:
  - `expression` (string, 必需): 数学表达式
- **返回**: 计算结果（数字）
- **注意**: 支持 +、-、*、/ 和括号
```

---

## 完整示例

### 示例 1：计算器工具集

```markdown
# 计算器工具集

提供基础数学运算和科学计算功能。

## 工具列表

### add
- **描述**: 两个数相加
- **参数**:
  - `a` (number, 必需): 第一个数
  - `b` (number, 必需): 第二个数
- **返回**: 和

### divide
- **描述**: 两个数相除
- **参数**:
  - `dividend` (number, 必需): 被除数
  - `divisor` (number, 必需): 除数
- **返回**: 商
- **注意**: 除数不能为零，否则返回错误

### sqrt
- **描述**: 计算平方根
- **参数**:
  - `number` (number, 必需): 要开方的数
- **返回**: 平方根
- **注意**: 负数返回错误
```

### 示例 2：文件处理工具集

```markdown
# 文件处理工具集

用于读取、解析和转换各种文件格式。

## 工具列表

### read_csv
- **描述**: 读取 CSV 文件并返回数据
- **参数**:
  - `path` (string, 必需): CSV 文件路径
  - `has_header` (boolean, 可选): 是否有表头，默认 true
- **返回**: JSON 数组，每行一个对象

### convert_to_json
- **描述**: 将 XML 文件转换为 JSON
- **参数**:
  - `input_path` (string, 必需): XML 文件路径
  - `output_path` (string, 必需): JSON 输出路径
- **返回**: 转换状态消息

### search_in_file
- **描述**: 在文件中搜索文本
- **参数**:
  - `path` (string, 必需): 文件路径
  - `pattern` (string, 必需): 搜索模式（支持正则）
  - `case_sensitive` (boolean, 可选): 是否区分大小写，默认 false
- **返回**: 匹配行列表
```

### 示例 3：自动化任务工具集

```markdown
# 自动化任务工具集

执行日常自动化任务，如发送通知、备份文件等。

## 工具列表

### send_email
- **描述**: 发送邮件
- **参数**:
  - `to` (string, 必需): 收件人邮箱
  - `subject` (string, 必需): 邮件主题
  - `body` (string, 必需): 邮件正文
  - `attachments` (array, 可选): 附件路径列表
- **返回**: 发送状态

### backup_folder
- **描述**: 备份文件夹到指定位置
- **参数**:
  - `source` (string, 必需): 源文件夹路径
  - `destination` (string, 必需): 目标文件夹路径
  - `compress` (boolean, 可选): 是否压缩，默认 false
- **返回**: 备份结果消息

### schedule_task
- **描述**: 安排定时任务
- **参数**:
  - `command` (string, 必需): 要执行的命令
  - `cron` (string, 必需): cron 表达式
  - `name` (string, 可选): 任务名称
- **返回**: 任务 ID
```

---

## 使用建议

1. **保持简洁** - 每个工具的描述控制在 1-2 句话
2. **提供示例** - 示例帮助 AI 理解如何调用
3. **注明边界情况** - 如"除数不能为零"、"负数返回错误"
4. **定期更新** - 工具变更时同步更新此文档

---

## 生成 Skill 文件

你可以手动编写 Skill 文件，也可以使用以下脚本自动生成：

```rust
// 自动生成 tools.md
use tokitai::tool;

#[tool]
impl MyTools {
    // ...
}

fn main() {
    let tools = MyTools::TOOL_DEFINITIONS;
    let mut md = String::from("# 我的工具集\n\n## 工具列表\n\n");
    
    for tool in tools {
        md.push_str(&format!("### {}\n", tool.name));
        md.push_str(&format!("- **描述**: {}\n", tool.description));
        md.push_str(&format!("- **Schema**: `{}`\n", tool.input_schema));
        md.push('\n');
    }
    
    std::fs::write("skills/tools.md", md).unwrap();
}
```
