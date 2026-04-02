//! File operation tools: FileRead, FileEdit, FileWrite

use async_trait::async_trait;
use serde_json::json;
use serde_json::Value;
use std::path::Path;
use tokio::fs;
use tracing::{debug, warn};

use crate::error::ToolError;
use crate::tools::helpers;
use crate::tools::{Tool, ToolContext, ToolResult};

/// Tool for reading file contents
///
/// Supports reading text files with optional offset and limit for
/// reading large files efficiently.
pub struct FileReadTool;

impl FileReadTool {
    /// Create a new FileReadTool instance
    pub fn new() -> Self {
        Self
    }
    
    /// Read a file with optional offset and limit
    async fn read_file(
        &self,
        path: &Path,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> ToolResult<String> {
        let content = fs::read_to_string(path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ToolError::not_found(path.display().to_string())
            } else {
                ToolError::from_io(e)
            }
        })?;
        
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();
        
        if total_lines == 0 {
            return Ok(String::new());
        }
        
        let offset = offset.unwrap_or(0);
        let limit = limit.unwrap_or(total_lines);
        
        if offset >= total_lines {
            return Ok(String::new());
        }
        
        let end = (offset + limit).min(total_lines);
        let selected = &lines[offset..end];
        
        Ok(selected.join("\n"))
    }
}

impl Default for FileReadTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FileReadTool {
    fn name(&self) -> &str {
        "file_read"
    }
    
    fn description(&self) -> &str {
        "Read the contents of a file. Supports optional line offset and limit for large files. \
         Returns the file content as text."
    }
    
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute or relative path to the file"
                },
                "offset": {
                    "type": "integer",
                    "description": "Line number to start reading from (0-indexed)",
                    "minimum": 0
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of lines to read",
                    "minimum": 1,
                    "maximum": 1000
                }
            },
            "required": ["path"]
        })
    }
    
    async fn execute(&self, args: Value, ctx: ToolContext) -> ToolResult<Value> {
        let path_str = helpers::required_str(&args, "path")?;
        let path = helpers::resolve_path(&ctx, path_str);
        
        let offset = args.get("offset").and_then(|v| v.as_u64()).map(|v| v as usize);
        let limit = args.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize);
        
        debug!("Reading file: {:?}, offset={:?}, limit={:?}", path, offset, limit);
        
        let content = self.read_file(&path, offset, limit).await?;
        
        Ok(json!({
            "path": path.display().to_string(),
            "content": content,
            "size": content.len()
        }))
    }
}

/// Tool for writing files
///
/// Creates new files or overwrites existing ones. Will create
/// parent directories if they don't exist.
pub struct FileWriteTool;

impl FileWriteTool {
    /// Create a new FileWriteTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileWriteTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FileWriteTool {
    fn name(&self) -> &str {
        "file_write"
    }
    
    fn description(&self) -> &str {
        "Write content to a file. Creates the file if it doesn't exist, \
         overwrites if it does. Creates parent directories as needed."
    }
    
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute or relative path to the file"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }
    
    async fn execute(&self, args: Value, ctx: ToolContext) -> ToolResult<Value> {
        if ctx.read_only {
            return Err(ToolError::PermissionDenied(
                "Cannot write files in read-only mode".to_string()
            ));
        }
        
        let path_str = helpers::required_str(&args, "path")?;
        let content = helpers::required_str(&args, "content")?;
        let path = helpers::resolve_path(&ctx, path_str);
        
        debug!("Writing file: {:?}", path);
        
        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(ToolError::from_io)?;
        }
        
        fs::write(&path, content).await.map_err(|e| {
            ToolError::execution_failed(format!("Failed to write file: {}", e))
        })?;
        
        Ok(json!({
            "path": path.display().to_string(),
            "bytes_written": content.len(),
            "success": true
        }))
    }
}

/// Tool for editing files
///
/// Performs string replacements in files. Supports multiple replacements
/// in a single operation and can validate changes.
pub struct FileEditTool;

impl FileEditTool {
    /// Create a new FileEditTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for FileEditTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for FileEditTool {
    fn name(&self) -> &str {
        "file_edit"
    }
    
    fn description(&self) -> &str {
        "Edit a file by replacing text. Performs string replacements in the file. \
         Supports multiple replacements. Returns the number of replacements made."
    }
    
    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Absolute or relative path to the file"
                },
                "old_string": {
                    "type": "string",
                    "description": "The text to search for and replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The replacement text"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default: false)",
                    "default": false
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }
    
    async fn execute(&self, args: Value, ctx: ToolContext) -> ToolResult<Value> {
        if ctx.read_only {
            return Err(ToolError::PermissionDenied(
                "Cannot edit files in read-only mode".to_string()
            ));
        }
        
        let path_str = helpers::required_str(&args, "path")?;
        let old_string = helpers::required_str(&args, "old_string")?;
        let new_string = helpers::required_str(&args, "new_string")?;
        let replace_all = helpers::optional_bool(&args, "replace_all", false);
        
        let path = helpers::resolve_path(&ctx, path_str);
        
        debug!("Editing file: {:?}", path);
        
        // Read existing content
        let content = fs::read_to_string(&path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                ToolError::not_found(path.display().to_string())
            } else {
                ToolError::from_io(e)
            }
        })?;
        
        // Perform replacement
        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            content.replacen(old_string, new_string, 1)
        };
        
        let count = if replace_all {
            content.matches(old_string).count()
        } else {
            if content.contains(old_string) { 1 } else { 0 }
        };
        
        if count == 0 {
            warn!("Pattern '{}' not found in file {:?}", old_string, path);
            return Err(ToolError::execution_failed(
                format!("Pattern '{}' not found in file", old_string)
            ));
        }
        
        // Write back
        fs::write(&path, new_content).await.map_err(ToolError::from_io)?;
        
        Ok(json!({
            "path": path.display().to_string(),
            "replacements": count,
            "success": true
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    
    #[tokio::test]
    async fn test_file_read() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        fs::write(&file_path, "Hello\nWorld\n!").await.unwrap();
        
        let tool = FileReadTool::new();
        let ctx = ToolContext::new().with_working_dir(temp_dir.path());
        
        // Test full read
        let args = json!({"path": "test.txt"});
        let result = tool.execute(args, ctx.clone()).await.unwrap();
        assert_eq!(result["content"], "Hello\nWorld\n!");
        
        // Test with offset
        let args = json!({"path": "test.txt", "offset": 1});
        let result = tool.execute(args, ctx.clone()).await.unwrap();
        assert_eq!(result["content"], "World\n!");
        
        // Test with limit
        let args = json!({"path": "test.txt", "limit": 1});
        let result = tool.execute(args, ctx.clone()).await.unwrap();
        assert_eq!(result["content"], "Hello");
    }
    
    #[tokio::test]
    async fn test_file_write() {
        let temp_dir = TempDir::new().unwrap();
        
        let tool = FileWriteTool::new();
        let ctx = ToolContext::new().with_working_dir(temp_dir.path());
        
        let args = json!({
            "path": "output.txt",
            "content": "Test content"
        });
        
        let result = tool.execute(args, ctx).await.unwrap();
        assert!(result["success"].as_bool().unwrap());
        
        let content = fs::read_to_string(temp_dir.path().join("output.txt")).await.unwrap();
        assert_eq!(content, "Test content");
    }
    
    #[tokio::test]
    async fn test_file_edit() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("edit.txt");
        fs::write(&file_path, "Hello World").await.unwrap();
        
        let tool = FileEditTool::new();
        let ctx = ToolContext::new().with_working_dir(temp_dir.path());
        
        let args = json!({
            "path": "edit.txt",
            "old_string": "World",
            "new_string": "Rust"
        });
        
        let result = tool.execute(args, ctx).await.unwrap();
        assert_eq!(result["replacements"], 1);
        
        let content = fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "Hello Rust");
    }
}
