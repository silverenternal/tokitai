//! 多工具协作聊天机器人示例
//!
//! 展示如何组合多个工具提供者，构建一个完整的 AI 助手

use serde_json::{json, Value};
use tokitai::tool;

// ==================== 工具定义 ====================

/// 待办事项管理
pub struct TodoManager {
    todos: std::sync::Mutex<Vec<TodoItem>>,
}

#[derive(Clone)]
struct TodoItem {
    id: usize,
    title: String,
    completed: bool,
}

impl TodoManager {
    pub fn new() -> Self {
        Self {
            todos: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl Default for TodoManager {
    fn default() -> Self {
        Self::new()
    }
}

#[tool]
impl TodoManager {
    /// 添加新的待办事项
    pub fn add_todo(&self, title: String) -> String {
        let mut todos = self.todos.lock().unwrap();
        let id = todos.len() + 1;
        todos.push(TodoItem {
            id,
            title: title.clone(),
            completed: false,
        });
        format!("已添加待办事项 #{}: {}", id, title)
    }

    /// 列出所有待办事项
    pub fn list_todos(&self) -> Value {
        let todos = self.todos.lock().unwrap();
        let items: Vec<Value> = todos
            .iter()
            .map(|t| {
                json!({
                    "id": t.id,
                    "title": t.title,
                    "completed": t.completed
                })
            })
            .collect();
        json!(items)
    }

    /// 完成待办事项
    pub fn complete_todo(&self, id: usize) -> Result<String, String> {
        let mut todos = self.todos.lock().unwrap();
        for todo in todos.iter_mut() {
            if todo.id == id {
                todo.completed = true;
                return Ok(format!("已完成待办事项 #{}: {}", id, todo.title));
            }
        }
        Err(format!("未找到待办事项 #{}", id))
    }
}

/// 笔记管理工具
pub struct NoteManager {
    notes: std::sync::Mutex<Vec<NoteItem>>,
}

#[derive(Clone)]
struct NoteItem {
    id: usize,
    title: String,
    content: String,
}

impl NoteManager {
    pub fn new() -> Self {
        Self {
            notes: std::sync::Mutex::new(Vec::new()),
        }
    }
}

impl Default for NoteManager {
    fn default() -> Self {
        Self::new()
    }
}

#[tool]
impl NoteManager {
    /// 创建新笔记
    pub fn create_note(&self, title: String, content: String) -> String {
        let mut notes = self.notes.lock().unwrap();
        let id = notes.len() + 1;
        notes.push(NoteItem { id, title, content });
        format!("已创建笔记 #{}: {}", id, notes.last().unwrap().title)
    }

    /// 获取笔记列表
    pub fn list_notes(&self) -> Value {
        let notes = self.notes.lock().unwrap();
        let items: Vec<Value> = notes
            .iter()
            .map(|n| {
                json!({
                    "id": n.id,
                    "title": n.title
                })
            })
            .collect();
        json!(items)
    }

    /// 读取笔记内容
    pub fn read_note(&self, id: usize) -> Result<Value, String> {
        let notes = self.notes.lock().unwrap();
        for note in notes.iter() {
            if note.id == id {
                return Ok(json!({
                    "id": note.id,
                    "title": note.title,
                    "content": note.content
                }));
            }
        }
        Err(format!("未找到笔记 #{}", id))
    }
}

/// 提醒工具
pub struct ReminderService;

#[tool]
impl ReminderService {
    /// 设置提醒
    pub fn set_reminder(&self, time: String, message: String) -> String {
        format!("已设置提醒 [{}]: {}", time, message)
    }

    /// 取消提醒
    pub fn cancel_reminder(&self, reminder_id: String) -> String {
        format!("已取消提醒 #{}", reminder_id)
    }
}

// ==================== 聊天机器人 ====================

struct PersonalAssistant {
    todo_manager: TodoManager,
    note_manager: NoteManager,
    reminder_service: ReminderService,
}

impl PersonalAssistant {
    fn new() -> Self {
        Self {
            todo_manager: TodoManager::new(),
            note_manager: NoteManager::new(),
            reminder_service: ReminderService,
        }
    }

    /// 获取所有工具定义
    fn get_all_tools(&self) -> Vec<&tokitai::ToolDefinition> {
        let mut tools = Vec::new();
        tools.extend(TodoManager::TOOL_DEFINITIONS.iter());
        tools.extend(NoteManager::TOOL_DEFINITIONS.iter());
        tools.extend(ReminderService::TOOL_DEFINITIONS.iter());
        tools
    }

    /// 处理工具调用
    fn handle_tool_call(
        &self,
        name: &str,
        args: &Value,
    ) -> Result<Value, String> {
        println!("   [执行工具] {}({:?})", name, args);

        let result = match name {
            // TodoManager 工具
            "add_todo" | "list_todos" | "complete_todo" => {
                self.todo_manager.call_tool(name, args)
                    .map_err(|e| format!("工具调用失败：{:?}", e))?
            }
            // NoteManager 工具
            "create_note" | "list_notes" | "read_note" => {
                self.note_manager.call_tool(name, args)
                    .map_err(|e| format!("工具调用失败：{:?}", e))?
            }
            // ReminderService 工具
            "set_reminder" | "cancel_reminder" => {
                self.reminder_service.call_tool(name, args)
                    .map_err(|e| format!("工具调用失败：{:?}", e))?
            }
            _ => return Err(format!("未知工具：{}", name)),
        };

        Ok(result)
    }

    /// 模拟处理用户请求
    fn process_request(&self, user_message: &str) -> Result<String, String> {
        println!("\n[用户] {}", user_message);

        // 模拟 AI 决定调用哪个工具
        // 实际应用中这里会调用 AI API
        let tool_calls = self.simulate_ai_decision(user_message);

        for (tool_name, tool_args) in tool_calls {
            let result = self.handle_tool_call(tool_name, &tool_args)?;
            println!("   [返回] {}", result);
        }

        Ok("操作已完成".to_string())
    }

    /// 模拟 AI 决策（实际应用中替换为真实的 AI 调用）
    fn simulate_ai_decision(&self, message: &str) -> Vec<(&str, Value)> {
        let mut calls = Vec::new();

        match message {
            m if m.contains("待办") && m.contains("添加") => {
                calls.push(("add_todo", json!({"title": "示例待办"})));
            }
            m if m.contains("待办") && m.contains("列表") => {
                calls.push(("list_todos", json!({})));
            }
            m if m.contains("笔记") && m.contains("创建") => {
                calls.push((
                    "create_note",
                    json!({"title": "示例笔记", "content": "笔记内容"}),
                ));
            }
            m if m.contains("笔记") && m.contains("列表") => {
                calls.push(("list_notes", json!({})));
            }
            m if m.contains("提醒") => {
                calls.push((
                    "set_reminder",
                    json!({"time": "明天 10:00", "message": "示例提醒"}),
                ));
            }
            _ => {
                println!("   [AI] 抱歉，我还不太理解您的需求");
            }
        }

        calls
    }
}

fn main() -> Result<(), String> {
    println!("=== Tokitai 多工具协作聊天机器人 ===\n");

    let assistant = PersonalAssistant::new();

    // 展示所有可用工具
    println!("可用工具列表:");
    for tool in assistant.get_all_tools() {
        println!(
            "   - {}: {} (schema: {})",
            tool.name, tool.description, tool.input_schema
        );
    }

    // 模拟对话场景
    println!("\n=== 对话演示 ===");

    assistant.process_request("请帮我添加一个待办事项")?;
    assistant.process_request("我想查看待办列表")?;
    assistant.process_request("创建一个新的笔记")?;
    assistant.process_request("显示我的笔记列表")?;
    assistant.process_request("设置一个明天的提醒")?;

    // 展示工具定义导出（用于发送给 AI）
    println!("\n=== 工具定义导出（发送给 AI）===");
    let tools_json = serde_json::to_string_pretty(
        &assistant
            .get_all_tools()
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": serde_json::from_str::<Value>(t.input_schema).unwrap_or_default()
                })
            })
            .collect::<Vec<_>>(),
    ).map_err(|e| format!("JSON 序列化失败：{}", e))?;
    println!("{}", tools_json);

    println!("\n=== 演示完成 ===");
    println!("\n提示：集成真实 AI 时，只需:");
    println!("1. 将工具定义发送给 AI API（如 Ollama、Claude 等）");
    println!("2. 接收 AI 返回的工具调用请求");
    println!("3. 调用 handle_tool_call 执行工具");
    println!("4. 将结果返回给 AI");

    Ok(())
}
