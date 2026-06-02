//! Multi-tool collaborative chatbot example
//!
//! Demonstrates how to combine multiple tool providers into a complete AI assistant.

use serde_json::Value;
use tokitai::json;
use tokitai::tool;
use tokitai::ToolProvider;

// ==================== Tool Definitions ====================

/// Todo list manager
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
    /// Add a new todo item
    pub fn add_todo(&self, title: String) -> String {
        let mut todos = self.todos.lock().unwrap();
        let id = todos.len() + 1;
        todos.push(TodoItem {
            id,
            title: title.clone(),
            completed: false,
        });
        format!("Added todo #{}: {}", id, title)
    }

    /// List all todo items
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

    /// Mark a todo item as complete
    pub fn complete_todo(&self, id: usize) -> Result<String, String> {
        let mut todos = self.todos.lock().unwrap();
        for todo in todos.iter_mut() {
            if todo.id == id {
                todo.completed = true;
                return Ok(format!("Completed todo #{}: {}", id, todo.title));
            }
        }
        Err(format!("Todo #{} not found", id))
    }
}

/// Note management tools
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
    /// Create a new note
    pub fn create_note(&self, title: String, content: String) -> String {
        let mut notes = self.notes.lock().unwrap();
        let id = notes.len() + 1;
        notes.push(NoteItem { id, title, content });
        format!("Created note #{}: {}", id, notes.last().unwrap().title)
    }

    /// Get the list of notes
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

    /// Read a note's content
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
        Err(format!("Note #{} not found", id))
    }
}

/// Reminder tools
pub struct ReminderService;

#[tool]
impl ReminderService {
    /// Set a reminder
    pub fn set_reminder(&self, time: String, message: String) -> String {
        format!("Reminder set [{}]: {}", time, message)
    }

    /// Cancel a reminder
    pub fn cancel_reminder(&self, reminder_id: String) -> String {
        format!("Cancelled reminder #{}", reminder_id)
    }
}

// ==================== Chatbot ====================

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

    /// Get all tool definitions
    fn get_all_tools(&self) -> Vec<&tokitai::ToolDefinition> {
        let mut tools = Vec::new();
        tools.extend(TodoManager::tool_definitions().iter());
        tools.extend(NoteManager::tool_definitions().iter());
        tools.extend(ReminderService::tool_definitions().iter());
        tools
    }

    /// Handle a tool call
    fn handle_tool_call(&self, name: &str, args: &Value) -> Result<Value, String> {
        println!("   [Execute tool] {}({:?})", name, args);

        let result = match name {
            // TodoManager tools
            "add_todo" | "list_todos" | "complete_todo" => self
                .todo_manager
                .call_tool(name, args)
                .map_err(|e| format!("Tool call failed: {:?}", e))?,
            // NoteManager tools
            "create_note" | "list_notes" | "read_note" => {
                self.note_manager
                    .call_tool(name, args)
                    .map_err(|e| format!("Tool call failed: {:?}", e))?
            }
            // ReminderService tools
            "set_reminder" | "cancel_reminder" => self
                .reminder_service
                .call_tool(name, args)
                .map_err(|e| format!("Tool call failed: {:?}", e))?,
            _ => return Err(format!("Unknown tool: {}", name)),
        };

        Ok(result)
    }

    /// Simulate processing a user request
    fn process_request(&self, user_message: &str) -> Result<String, String> {
        println!("\n[User] {}", user_message);

        // Simulate the AI deciding which tools to call
        // In a real application this would invoke an AI API
        let tool_calls = self.simulate_ai_decision(user_message);

        for (tool_name, tool_args) in tool_calls {
            let result = self.handle_tool_call(tool_name, &tool_args)?;
            println!("   [Return] {}", result);
        }

        Ok("Operation complete".to_string())
    }

    /// Simulate AI decision-making (replace with a real AI call in production)
    fn simulate_ai_decision(&self, message: &str) -> Vec<(&str, Value)> {
        let mut calls = Vec::new();

        match message {
            m if m.contains("todo") && m.contains("add") => {
                calls.push(("add_todo", json!({"title": "Sample todo"})));
            }
            m if m.contains("todo") && m.contains("list") => {
                calls.push(("list_todos", json!({})));
            }
            m if m.contains("note") && m.contains("create") => {
                calls.push((
                    "create_note",
                    json!({"title": "Sample note", "content": "Note content"}),
                ));
            }
            m if m.contains("note") && m.contains("list") => {
                calls.push(("list_notes", json!({})));
            }
            m if m.contains("reminder") => {
                calls.push((
                    "set_reminder",
                    json!({"time": "tomorrow 10:00", "message": "Sample reminder"}),
                ));
            }
            _ => {
                println!("   [AI] Sorry, I don't quite understand your request");
            }
        }

        calls
    }
}

fn main() -> Result<(), String> {
    println!("=== Tokitai Multi-Tool Chatbot ===\n");

    let assistant = PersonalAssistant::new();

    // Show all available tools
    println!("Available tools:");
    for tool in assistant.get_all_tools() {
        println!(
            "   - {}: {} (schema: {})",
            tool.name, tool.description, tool.input_schema
        );
    }

    // Simulate a conversation
    println!("\n=== Conversation Demo ===");

    assistant.process_request("Please add a todo item for me")?;
    assistant.process_request("I want to view my todo list")?;
    assistant.process_request("Create a new note")?;
    assistant.process_request("Show my list of notes")?;
    assistant.process_request("Set a reminder for tomorrow")?;

    // Export tool definitions (to send to the AI)
    println!("\n=== Tool Definition Export (sent to AI) ===");
    let tools_json = serde_json::to_string_pretty(
        &assistant
            .get_all_tools()
            .iter()
            .map(|t| {
                json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": serde_json::from_str::<Value>(&t.input_schema).unwrap_or_default()
                })
            })
            .collect::<Vec<_>>(),
    ).map_err(|e| format!("JSON serialization failed: {}", e))?;
    println!("{}", tools_json);

    println!("\n=== Demo Complete ===");
    println!("\nHint: to integrate a real AI, you only need to:");
    println!("1. Send the tool definitions to an AI API (e.g. Ollama, Claude, etc.)");
    println!("2. Receive the tool-call requests returned by the AI");
    println!("3. Call handle_tool_call to execute the tool");
    println!("4. Return the result to the AI");

    Ok(())
}
