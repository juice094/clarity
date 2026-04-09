//! # Todo Skill
//!
//! A skill for managing tasks and todo lists.
//!
//! ## Commands
//!
//! - `add <task>` - Add a new task
//! - `list` - List all tasks
//! - `done <index>` - Mark a task as completed
//! - `remove <index>` - Remove a task
//! - `clear` - Clear all completed tasks
//! - `clear-all` - Clear all tasks
//!
//! ## Example
//!
//! ```rust
//! use clarity_core::skill::{Skill, TodoSkill};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let todo = TodoSkill::new();
//!
//! // Add tasks
//! todo.execute("add Buy groceries").await?;
//! todo.execute("add Finish report").await?;
//!
//! // List tasks
//! let list = todo.execute("list").await?;
//! println!("{}", list);
//!
//! // Mark task as done
//! todo.execute("done 1").await?;
//! # Ok(())
//! # }
//! ```

use super::{Skill, SkillError, SkillResult};
use async_trait::async_trait;
use std::sync::Mutex;

/// Represents a single todo item
#[derive(Debug, Clone, PartialEq)]
pub struct TodoItem {
    /// The task description
    pub task: String,
    /// Whether the task is completed
    pub completed: bool,
    /// Optional due date
    pub due_date: Option<String>,
}

impl TodoItem {
    /// Create a new todo item
    pub fn new(task: impl Into<String>) -> Self {
        Self {
            task: task.into(),
            completed: false,
            due_date: None,
        }
    }

    /// Create a new todo item with a due date
    pub fn with_due_date(task: impl Into<String>, due_date: impl Into<String>) -> Self {
        Self {
            task: task.into(),
            completed: false,
            due_date: Some(due_date.into()),
        }
    }

    /// Mark the item as completed
    pub fn complete(&mut self) {
        self.completed = true;
    }

    /// Mark the item as incomplete
    pub fn uncomplete(&mut self) {
        self.completed = false;
    }

    /// Format the item for display
    pub fn format(&self, index: usize) -> String {
        let status = if self.completed { "✓" } else { "○" };
        let due = self
            .due_date
            .as_ref()
            .map(|d| format!(" (due: {})", d))
            .unwrap_or_default();
        format!("{}. {} {}{}", index, status, self.task, due)
    }
}

/// Skill for managing todo lists
///
/// The TodoSkill maintains an in-memory list of tasks that can be
/// added, completed, and removed through simple commands.
pub struct TodoSkill {
    items: Mutex<Vec<TodoItem>>,
}

impl TodoSkill {
    /// Create a new TodoSkill with an empty task list
    pub fn new() -> Self {
        Self {
            items: Mutex::new(Vec::new()),
        }
    }

    /// Create a new TodoSkill with pre-populated items
    pub fn with_items(items: Vec<TodoItem>) -> Self {
        Self {
            items: Mutex::new(items),
        }
    }

    /// Get all items
    fn get_items(&self) -> Vec<TodoItem> {
        self.items.lock().unwrap().clone()
    }

    /// Add a new item
    fn add_item(&self, task: String) {
        let mut items = self.items.lock().unwrap();
        items.push(TodoItem::new(task));
    }

    /// Mark an item as completed
    fn complete_item(&self, index: usize) -> SkillResult<()> {
        let mut items = self.items.lock().unwrap();
        if index == 0 || index > items.len() {
            return Err(SkillError::invalid_input(format!(
                "Invalid task index: {}. Use 'list' to see valid indices.",
                index
            )));
        }
        items[index - 1].complete();
        Ok(())
    }

    /// Remove an item
    fn remove_item(&self, index: usize) -> SkillResult<()> {
        let mut items = self.items.lock().unwrap();
        if index == 0 || index > items.len() {
            return Err(SkillError::invalid_input(format!(
                "Invalid task index: {}. Use 'list' to see valid indices.",
                index
            )));
        }
        items.remove(index - 1);
        Ok(())
    }

    /// Clear all completed items
    fn clear_completed(&self) {
        let mut items = self.items.lock().unwrap();
        items.retain(|item| !item.completed);
    }

    /// Clear all items
    fn clear_all(&self) {
        let mut items = self.items.lock().unwrap();
        items.clear();
    }

    /// List all items as formatted string
    fn list_items(&self) -> String {
        let items = self.get_items();
        if items.is_empty() {
            return "No tasks. Use 'add <task>' to create one.".to_string();
        }

        let mut output = format!("Todo List ({} items):\n", items.len());
        for (i, item) in items.iter().enumerate() {
            output.push_str(&item.format(i + 1));
            output.push('\n');
        }

        let completed = items.iter().filter(|i| i.completed).count();
        let pending = items.len() - completed;
        output.push_str(&format!("\n{} pending, {} completed", pending, completed));

        output
    }

    /// Parse and execute a command
    async fn execute_command(&self, command: &str, args: &str) -> SkillResult<String> {
        match command {
            "add" => {
                if args.trim().is_empty() {
                    return Err(SkillError::invalid_input("Task description cannot be empty"));
                }
                self.add_item(args.to_string());
                Ok(format!("Added task: {}", args))
            }
            "list" => Ok(self.list_items()),
            "done" | "complete" => {
                let index: usize = args
                    .trim()
                    .parse()
                    .map_err(|_| SkillError::invalid_input("Please provide a valid task number"))?;
                self.complete_item(index)?;
                Ok(format!("Marked task {} as completed", index))
            }
            "undo" => {
                let index: usize = args
                    .trim()
                    .parse()
                    .map_err(|_| SkillError::invalid_input("Please provide a valid task number"))?;
                let mut items = self.items.lock().unwrap();
                if index == 0 || index > items.len() {
                    return Err(SkillError::invalid_input(format!(
                        "Invalid task index: {}",
                        index
                    )));
                }
                items[index - 1].uncomplete();
                Ok(format!("Marked task {} as incomplete", index))
            }
            "remove" | "rm" | "delete" => {
                let index: usize = args
                    .trim()
                    .parse()
                    .map_err(|_| SkillError::invalid_input("Please provide a valid task number"))?;
                self.remove_item(index)?;
                Ok(format!("Removed task {}", index))
            }
            "clear" => {
                self.clear_completed();
                Ok("Cleared all completed tasks".to_string())
            }
            "clear-all" => {
                self.clear_all();
                Ok("Cleared all tasks".to_string())
            }
            "help" => Ok(self.help_text()),
            _ => Err(SkillError::invalid_input(format!(
                "Unknown command: '{}'. Type 'help' for available commands.",
                command
            ))),
        }
    }

    /// Get help text
    fn help_text(&self) -> String {
        r#"Todo Skill - Commands:
  add <task>       - Add a new task
  list             - List all tasks
  done <index>     - Mark task as completed
  undo <index>     - Mark task as incomplete
  remove <index>   - Remove a task
  clear            - Clear completed tasks
  clear-all        - Clear all tasks
  help             - Show this help message

Examples:
  todo add Buy groceries
  todo list
  todo done 1
  todo remove 2"#
            .to_string()
    }
}

impl Default for TodoSkill {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Skill for TodoSkill {
    fn name(&self) -> &str {
        "todo"
    }

    fn description(&self) -> &str {
        "Manage todo lists and tasks. Supports: add, list, done, remove, clear"
    }

    async fn execute(&self, input: &str) -> SkillResult<String> {
        let input = input.trim();

        if input.is_empty() || input == "help" {
            return Ok(self.help_text());
        }

        // Parse command and arguments
        let parts: Vec<&str> = input.splitn(2, ' ').collect();
        let command = parts[0].to_lowercase();
        let args = parts.get(1).copied().unwrap_or("");

        self.execute_command(&command, args).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_todo_item_new() {
        let item = TodoItem::new("Test task");
        assert_eq!(item.task, "Test task");
        assert!(!item.completed);
        assert!(item.due_date.is_none());
    }

    #[test]
    fn test_todo_item_with_due_date() {
        let item = TodoItem::with_due_date("Test task", "2024-12-25");
        assert_eq!(item.task, "Test task");
        assert_eq!(item.due_date, Some("2024-12-25".to_string()));
    }

    #[test]
    fn test_todo_item_complete() {
        let mut item = TodoItem::new("Test task");
        assert!(!item.completed);
        item.complete();
        assert!(item.completed);
        item.uncomplete();
        assert!(!item.completed);
    }

    #[test]
    fn test_todo_item_format() {
        let item = TodoItem::new("Test task");
        assert_eq!(item.format(1), "1. ○ Test task");

        let mut completed = TodoItem::new("Done task");
        completed.complete();
        assert_eq!(completed.format(2), "2. ✓ Done task");

        let with_due = TodoItem::with_due_date("Task with due", "2024-12-25");
        assert_eq!(with_due.format(3), "3. ○ Task with due (due: 2024-12-25)");
    }

    #[test]
    fn test_todo_skill_new() {
        let skill = TodoSkill::new();
        assert_eq!(skill.name(), "todo");
        assert!(skill.get_items().is_empty());
    }

    #[test]
    fn test_todo_skill_with_items() {
        let items = vec![
            TodoItem::new("Task 1"),
            TodoItem::new("Task 2"),
        ];
        let skill = TodoSkill::with_items(items);
        assert_eq!(skill.get_items().len(), 2);
    }

    #[tokio::test]
    async fn test_todo_add() {
        let skill = TodoSkill::new();
        let result = skill.execute("add Test task").await.unwrap();
        assert!(result.contains("Added task"));
        assert_eq!(skill.get_items().len(), 1);
        assert_eq!(skill.get_items()[0].task, "Test task");
    }

    #[tokio::test]
    async fn test_todo_add_empty() {
        let skill = TodoSkill::new();
        let result = skill.execute("add  ").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("cannot be empty"));
    }

    #[tokio::test]
    async fn test_todo_list_empty() {
        let skill = TodoSkill::new();
        let result = skill.execute("list").await.unwrap();
        assert!(result.contains("No tasks"));
    }

    #[tokio::test]
    async fn test_todo_list_with_items() {
        let skill = TodoSkill::new();
        skill.execute("add Task 1").await.unwrap();
        skill.execute("add Task 2").await.unwrap();

        let result = skill.execute("list").await.unwrap();
        assert!(result.contains("Task 1"));
        assert!(result.contains("Task 2"));
        assert!(result.contains("2 pending, 0 completed"));
    }

    #[tokio::test]
    async fn test_todo_done() {
        let skill = TodoSkill::new();
        skill.execute("add Task to complete").await.unwrap();

        let result = skill.execute("done 1").await.unwrap();
        assert!(result.contains("Marked task 1 as completed"));
        assert!(skill.get_items()[0].completed);
    }

    #[tokio::test]
    async fn test_todo_done_invalid_index() {
        let skill = TodoSkill::new();
        skill.execute("add Task").await.unwrap();

        let result = skill.execute("done 5").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid task index"));
    }

    #[tokio::test]
    async fn test_todo_undo() {
        let skill = TodoSkill::new();
        skill.execute("add Task").await.unwrap();
        skill.execute("done 1").await.unwrap();
        assert!(skill.get_items()[0].completed);

        let result = skill.execute("undo 1").await.unwrap();
        assert!(result.contains("Marked task 1 as incomplete"));
        assert!(!skill.get_items()[0].completed);
    }

    #[tokio::test]
    async fn test_todo_remove() {
        let skill = TodoSkill::new();
        skill.execute("add Task 1").await.unwrap();
        skill.execute("add Task 2").await.unwrap();

        let result = skill.execute("remove 1").await.unwrap();
        assert!(result.contains("Removed task 1"));
        assert_eq!(skill.get_items().len(), 1);
        assert_eq!(skill.get_items()[0].task, "Task 2");
    }

    #[tokio::test]
    async fn test_todo_clear_completed() {
        let skill = TodoSkill::new();
        skill.execute("add Task 1").await.unwrap();
        skill.execute("add Task 2").await.unwrap();
        skill.execute("done 1").await.unwrap();

        let result = skill.execute("clear").await.unwrap();
        assert!(result.contains("Cleared all completed tasks"));
        assert_eq!(skill.get_items().len(), 1);
        assert_eq!(skill.get_items()[0].task, "Task 2");
    }

    #[tokio::test]
    async fn test_todo_clear_all() {
        let skill = TodoSkill::new();
        skill.execute("add Task 1").await.unwrap();
        skill.execute("add Task 2").await.unwrap();

        let result = skill.execute("clear-all").await.unwrap();
        assert!(result.contains("Cleared all tasks"));
        assert!(skill.get_items().is_empty());
    }

    #[tokio::test]
    async fn test_todo_help() {
        let skill = TodoSkill::new();
        let result = skill.execute("").await.unwrap();
        assert!(result.contains("Todo Skill"));
        assert!(result.contains("add"));
        assert!(result.contains("list"));
        assert!(result.contains("done"));
    }

    #[tokio::test]
    async fn test_todo_unknown_command() {
        let skill = TodoSkill::new();
        let result = skill.execute("unknown").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown command"));
    }

    #[tokio::test]
    async fn test_todo_complete_workflow() {
        let skill = TodoSkill::new();

        // Add tasks
        skill.execute("add Buy milk").await.unwrap();
        skill.execute("add Finish report").await.unwrap();
        skill.execute("add Call mom").await.unwrap();
        assert_eq!(skill.get_items().len(), 3);

        // Complete one
        skill.execute("done 2").await.unwrap();

        // List should show 2 pending, 1 completed
        let list = skill.execute("list").await.unwrap();
        assert!(list.contains("2 pending, 1 completed"));

        // Clear completed
        skill.execute("clear").await.unwrap();
        assert_eq!(skill.get_items().len(), 2);

        // Remove another
        skill.execute("remove 1").await.unwrap();
        assert_eq!(skill.get_items().len(), 1);
        assert_eq!(skill.get_items()[0].task, "Call mom");
    }
}
