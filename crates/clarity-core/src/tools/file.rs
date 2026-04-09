//! File operation tools: FileRead, FileEdit, FileWrite

use async_trait::async_trait;
use serde_json::json;
use serde_json::Value;
use std::path::Path;
use tokio::fs;
use tracing::{debug, warn};

use crate::approval::ApprovalMode;
use crate::error::ToolError;
use crate::tools::helpers;
use crate::tools::{Tool, ToolContext, ToolResult};

/// Check whether a path points to a known sensitive file.
pub fn is_sensitive_file(path: &Path) -> bool {
    let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("");

    if name == ".env" || name.starts_with(".env.") {
        return true;
    }
    if matches!(
        name,
        "id_rsa"
            | "id_rsa.pub"
            | "id_ed25519"
            | "id_ed25519.pub"
            | ".npmrc"
            | ".pypirc"
            | ".netrc"
            | "kubeconfig"
    ) {
        return true;
    }
    if name.ends_with(".key") || name.ends_with(".pem") {
        return true;
    }

    let components: Vec<&str> = path
        .components()
        .filter_map(|c| {
            if let std::path::Component::Normal(os) = c {
                os.to_str()
            } else {
                None
            }
        })
        .collect();

    for window in components.windows(2) {
        if window[0] == ".ssh" {
            return true;
        }
        if window[0] == ".aws" && (window[1] == "credentials" || window[1] == "config") {
            return true;
        }
    }

    false
}

/// Sniff the first bytes of a file for known binary/media magic numbers.
async fn sniff_media_file(path: &Path) -> Option<&'static str> {
    use tokio::io::AsyncReadExt;

    let metadata = fs::metadata(path).await.ok()?;
    if metadata.len() < 8 {
        return None;
    }

    let mut header = [0u8; 16];
    let mut file = tokio::fs::File::open(path).await.ok()?;
    let n = file.read(&mut header).await.ok()?;
    if n < 8 {
        return None;
    }

    if header.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        return Some("PNG image");
    }
    if header.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("JPEG image");
    }
    if header.starts_with(b"GIF8") {
        return Some("GIF image");
    }
    if header.starts_with(b"%PDF") {
        return Some("PDF document");
    }
    if header.starts_with(&[0x50, 0x4B, 0x03, 0x04]) {
        return Some("ZIP archive (may be docx/xlsx)");
    }

    None
}

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
        // Media file sniffing
        if let Some(media_type) = sniff_media_file(path).await {
            return Err(ToolError::execution_failed(format!(
                "Cannot read binary file {} as text (detected: {}). Use an appropriate tool.",
                path.display(),
                media_type
            )));
        }

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
        
        let is_sensitive = is_sensitive_file(&path);
        let content = self.read_file(&path, offset, limit).await?;
        
        let mut result = json!({
            "path": path.display().to_string(),
            "content": content,
            "size": content.len()
        });
        
        if is_sensitive && ctx.approval_mode == ApprovalMode::Yolo {
            tracing::warn!("Sensitive file read in YOLO mode: {:?}", path);
            if let Some(obj) = result.as_object_mut() {
                obj.insert("sensitive_file_warning".to_string(), json!(format!("Accessed sensitive file: {}", path.display())));
            }
        }
        
        Ok(result)
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
        
        let is_sensitive = is_sensitive_file(&path);
        
        debug!("Writing file: {:?}", path);
        
        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(ToolError::from_io)?;
        }
        
        fs::write(&path, content).await.map_err(|e| {
            ToolError::execution_failed(format!("Failed to write file: {}", e))
        })?;
        
        let mut result = json!({
            "path": path.display().to_string(),
            "bytes_written": content.len(),
            "success": true
        });
        
        if is_sensitive && ctx.approval_mode == ApprovalMode::Yolo {
            tracing::warn!("Sensitive file write in YOLO mode: {:?}", path);
            if let Some(obj) = result.as_object_mut() {
                obj.insert("sensitive_file_warning".to_string(), json!(format!("Wrote sensitive file: {}", path.display())));
            }
        }
        
        Ok(result)
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
        
        let is_sensitive = is_sensitive_file(&path);
        
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
        fs::write(&path, &new_content).await.map_err(ToolError::from_io)?;
        
        let mut result = json!({
            "path": path.display().to_string(),
            "replacements": count,
            "success": true
        });
        
        if ctx.approval_mode != ApprovalMode::Yolo {
            if let Some(obj) = result.as_object_mut() {
                obj.insert("_diff_preview".to_string(), json!({
                    "old": content,
                    "new": new_content
                }));
            }
        }
        
        if is_sensitive && ctx.approval_mode == ApprovalMode::Yolo {
            tracing::warn!("Sensitive file edit in YOLO mode: {:?}", path);
            if let Some(obj) = result.as_object_mut() {
                obj.insert("sensitive_file_warning".to_string(), json!(format!("Edited sensitive file: {}", path.display())));
            }
        }
        
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::approval::ApprovalMode;
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

    #[test]
    fn test_is_sensitive_file_patterns() {
        assert!(is_sensitive_file(Path::new(".env")));
        assert!(is_sensitive_file(Path::new(".env.local")));
        assert!(is_sensitive_file(Path::new("/home/user/.ssh/id_rsa")));
        assert!(is_sensitive_file(Path::new("/home/user/.ssh/known_hosts")));
        assert!(is_sensitive_file(Path::new("C:\\Users\\user\\.aws\\credentials")));
        assert!(is_sensitive_file(Path::new("/root/.aws/config")));
        assert!(is_sensitive_file(Path::new("server.pem")));
        assert!(is_sensitive_file(Path::new("tls.key")));
        assert!(is_sensitive_file(Path::new("kubeconfig")));
        assert!(!is_sensitive_file(Path::new("README.md")));
        assert!(!is_sensitive_file(Path::new("main.rs")));
    }

    #[tokio::test]
    async fn test_sniff_png() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("image.png");
        fs::write(&file_path, &[0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00]).await.unwrap();
        
        let tool = FileReadTool::new();
        let ctx = ToolContext::new().with_working_dir(temp_dir.path());
        let args = json!({"path": "image.png"});
        let result = tool.execute(args, ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("PNG image"));
    }

    #[tokio::test]
    async fn test_sniff_pdf() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("doc.pdf");
        fs::write(&file_path, b"%PDF-1.4 some data").await.unwrap();
        
        let tool = FileReadTool::new();
        let ctx = ToolContext::new().with_working_dir(temp_dir.path());
        let args = json!({"path": "doc.pdf"});
        let result = tool.execute(args, ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("PDF document"));
    }

    #[tokio::test]
    async fn test_read_small_file_skips_sniff() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("tiny.txt");
        fs::write(&file_path, "hi").await.unwrap();
        
        let tool = FileReadTool::new();
        let ctx = ToolContext::new().with_working_dir(temp_dir.path());
        let args = json!({"path": "tiny.txt"});
        let result = tool.execute(args, ctx).await.unwrap();
        assert_eq!(result["content"], "hi");
    }

    #[tokio::test]
    async fn test_sensitive_file_yolo_warning() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join(".env");
        fs::write(&file_path, "SECRET=1").await.unwrap();
        
        let tool = FileReadTool::new();
        let ctx = ToolContext::new()
            .with_working_dir(temp_dir.path())
            .with_approval_mode(ApprovalMode::Yolo);
        
        let args = json!({"path": ".env"});
        let result = tool.execute(args, ctx).await.unwrap();
        assert_eq!(result["content"], "SECRET=1");
        assert!(result["sensitive_file_warning"].as_str().unwrap().contains(".env"));
    }

    #[tokio::test]
    async fn test_file_edit_includes_diff_preview_in_interactive_mode() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("edit.txt");
        fs::write(&file_path, "Hello World").await.unwrap();
        
        let tool = FileEditTool::new();
        let ctx = ToolContext::new()
            .with_working_dir(temp_dir.path())
            .with_approval_mode(ApprovalMode::Interactive);
        
        let args = json!({
            "path": "edit.txt",
            "old_string": "World",
            "new_string": "Rust"
        });
        
        let result = tool.execute(args, ctx).await.unwrap();
        assert_eq!(result["replacements"], 1);
        assert!(result.get("_diff_preview").is_some());
        assert_eq!(result["_diff_preview"]["old"], "Hello World");
        assert_eq!(result["_diff_preview"]["new"], "Hello Rust");
    }

    #[tokio::test]
    async fn test_file_edit_no_diff_preview_in_yolo_mode() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("edit.txt");
        fs::write(&file_path, "Hello World").await.unwrap();
        
        let tool = FileEditTool::new();
        let ctx = ToolContext::new()
            .with_working_dir(temp_dir.path())
            .with_approval_mode(ApprovalMode::Yolo);
        
        let args = json!({
            "path": "edit.txt",
            "old_string": "World",
            "new_string": "Rust"
        });
        
        let result = tool.execute(args, ctx).await.unwrap();
        assert_eq!(result["replacements"], 1);
        assert!(result.get("_diff_preview").is_none());
    }
}
