//! Tool implementations for Clarity Core
//!
//! This module contains all built-in tools and the core `Tool` trait.
//! Tools are organized by category:
//! - `file`: File operations (read, edit, write)
//! - `shell`: Shell execution (bash, powershell)
//! - `search`: Search operations (glob, grep)

use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

pub mod ask_user;
pub mod computer;
pub mod cron;
pub mod file;
pub mod notify;
pub mod plan;
pub mod search;
pub mod shell;
pub mod task;
pub mod think;
pub mod todo;
pub mod web;

pub use ask_user::AskUserTool;
pub use computer::ComputerUseTool;
pub use cron::{CancelCronTool, ListCronTool, ScheduleCronTool};
pub use file::{FileEditTool, FileReadTool, FileWriteTool};
pub use notify::NotifyTool;
pub use plan::PlanTool;
pub use search::{GlobTool, GrepTool};
pub use shell::{BashTool, PowerShellTool};
pub use task::{TaskListTool, TaskOutputTool, TaskStopTool};
pub use think::ThinkTool;
pub use todo::TodoTool;
pub use web::{WebFetchTool, WebSearchTool};

use crate::approval::ApprovalMode;
use crate::error::ToolError;
use crate::subagents::token::CapabilityToken;

/// Result type for tool execution
pub type ToolResult<T> = Result<T, ToolError>;

/// Context passed to tools during execution
///
/// Contains information about the current execution environment,
/// working directory, and shared resources.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Current working directory for the operation
    pub working_dir: PathBuf,

    /// Environment variables
    pub env: HashMap<String, String>,

    /// Request timeout in seconds
    pub timeout_secs: u64,

    /// Maximum output size (bytes)
    pub max_output_size: usize,

    /// Whether the operation is read-only
    pub read_only: bool,

    /// Current approval mode
    pub approval_mode: ApprovalMode,

    /// Optional capability token for permission isolation
    pub capability_token: Option<CapabilityToken>,
}

impl ToolContext {
    /// Create a new tool context with default settings
    pub fn new() -> Self {
        Self {
            working_dir: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            env: std::env::vars().collect(),
            timeout_secs: 30,
            max_output_size: 1024 * 1024, // 1MB
            read_only: false,
            approval_mode: ApprovalMode::Interactive,
            capability_token: None,
        }
    }

    /// Set the working directory
    pub fn with_working_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.working_dir = path.into();
        self
    }

    /// Set the timeout
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = secs;
        self
    }

    /// Set read-only mode
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Add an environment variable
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.insert(key.into(), value.into());
        self
    }

    /// Set approval mode
    pub fn with_approval_mode(mut self, mode: ApprovalMode) -> Self {
        self.approval_mode = mode;
        self
    }

    /// Set capability token for permission isolation
    pub fn with_capability_token(mut self, token: Option<CapabilityToken>) -> Self {
        self.capability_token = token;
        self
    }
}

impl Default for ToolContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Core trait for all tools in the Clarity system
///
/// Implement this trait to create new tools that can be registered
/// with the `ToolRegistry` and used by the `Agent`.
///
/// # Example
///
/// ```rust
/// use async_trait::async_trait;
/// use clarity_core::tools::{Tool, ToolContext, ToolResult};
/// use clarity_core::ToolError;
/// use serde_json::{json, Value};
///
/// pub struct EchoTool;
///
/// #[async_trait]
/// impl Tool for EchoTool {
///     fn name(&self) -> &str {
///         "echo"
///     }
///     
///     fn description(&self) -> &str {
///         "Echoes back the input message"
///     }
///     
///     fn parameters(&self) -> Value {
///         json!({
///             "type": "object",
///             "properties": {
///                 "message": {
///                     "type": "string",
///                     "description": "The message to echo"
///                 }
///             },
///             "required": ["message"]
///         })
///     }
///     
///     async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
///         let message = args.get("message")
///             .and_then(|v| v.as_str())
///             .ok_or_else(|| ToolError::invalid_params("missing 'message'"))?;
///         
///         Ok(json!({ "echo": message }))
///     }
/// }
/// ```
#[async_trait]
pub trait Tool: Send + Sync {
    /// Tool name - must be unique within a registry
    fn name(&self) -> &str;

    /// Human-readable description for LLM
    fn description(&self) -> &str;

    /// JSON Schema for tool parameters
    ///
    /// This should follow the JSON Schema specification and describe
    /// all parameters the tool accepts.
    fn parameters(&self) -> Value;

    /// Execute the tool with the given arguments
    ///
    /// # Arguments
    ///
    /// * `args` - JSON Value containing the tool parameters
    /// * `ctx` - Execution context (working dir, env vars, etc.)
    ///
    /// # Returns
    ///
    /// The result of the execution as a JSON Value
    async fn execute(&self, args: Value, ctx: ToolContext) -> ToolResult<Value>;

    /// Whether this tool requires explicit user approval regardless of global approval mode.
    ///
    /// Tools that directly interact with the OS GUI (e.g. `computer_use`) should return `true`.
    /// The default is `false`.
    fn requires_approval(&self) -> bool {
        false
    }
}

/// Type-erased tool wrapper for storage in collections
pub type BoxedTool = Box<dyn Tool>;

/// Shared tool reference (for concurrency)
pub type SharedTool = Arc<dyn Tool>;

/// Helper trait to convert tools to shared references
pub trait IntoSharedTool: Tool + Sized
where
    Self: 'static,
{
    fn into_shared(self) -> SharedTool {
        Arc::new(self)
    }
}

impl<T: Tool + Sized + 'static> IntoSharedTool for T {}

/// Common parameter extraction helpers
pub mod helpers {
    use super::*;

    /// Extract a required string parameter
    pub fn required_str<'a>(args: &'a Value, name: &str) -> ToolResult<&'a str> {
        args.get(name).and_then(|v| v.as_str()).ok_or_else(|| {
            ToolError::invalid_params(format!("missing required parameter: {}", name))
        })
    }

    /// Extract an optional string parameter
    pub fn optional_str<'a>(args: &'a Value, name: &str) -> Option<&'a str> {
        args.get(name).and_then(|v| v.as_str())
    }

    /// Extract a required boolean parameter
    pub fn required_bool(args: &Value, name: &str) -> ToolResult<bool> {
        args.get(name).and_then(|v| v.as_bool()).ok_or_else(|| {
            ToolError::invalid_params(format!("missing required parameter: {}", name))
        })
    }

    /// Extract an optional boolean parameter
    pub fn optional_bool(args: &Value, name: &str, default: bool) -> bool {
        args.get(name).and_then(|v| v.as_bool()).unwrap_or(default)
    }

    /// Extract a required array of strings
    pub fn required_string_array(args: &Value, name: &str) -> ToolResult<Vec<String>> {
        args.get(name)
            .and_then(|v| v.as_array())
            .ok_or_else(|| {
                ToolError::invalid_params(format!("missing required parameter: {}", name))
            })?
            .iter()
            .map(|v| v.as_str().map(|s| s.to_string()))
            .collect::<Option<Vec<_>>>()
            .ok_or_else(|| {
                ToolError::invalid_params(format!("{} must be an array of strings", name))
            })
    }

    /// Normalize a path by resolving `.` and `..` components.
    /// Does not require the path to exist and does not add UNC prefixes.
    pub(crate) fn normalize_path(path: &std::path::Path) -> PathBuf {
        let mut result = PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::Prefix(p) => result.push(p.as_os_str()),
                std::path::Component::RootDir => result.push(component),
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    result.pop();
                }
                std::path::Component::Normal(name) => {
                    result.push(name);
                }
            }
        }
        result
    }

    /// Resolve a path relative to the working directory.
    ///
    /// Returns an error if the resolved path escapes the working directory
    /// (e.g. via `..` traversal or an absolute path outside the working directory).
    pub fn resolve_path(ctx: &ToolContext, path: &str) -> Result<PathBuf, ToolError> {
        let base = &ctx.working_dir;
        let input = PathBuf::from(path);

        // Ensure base is absolute for reliable comparison
        let base_abs = if base.is_absolute() {
            base.clone()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(base)
        };

        let resolved = if input.is_absolute() {
            input.clone()
        } else {
            base_abs.join(&input)
        };

        let base_norm = normalize_path(&base_abs);
        let resolved_norm = normalize_path(&resolved);

        if !resolved_norm.starts_with(&base_norm) {
            return Err(ToolError::invalid_params(format!(
                "Path '{}' is outside working directory '{}'",
                path,
                base.display()
            )));
        }

        Ok(resolved)
    }
}

#[cfg(test)]
mod tests {
    use crate::tools::helpers::{normalize_path, resolve_path};
    use crate::tools::ToolContext;
    use std::path::PathBuf;

    fn test_base() -> PathBuf {
        // Use current_dir for a real absolute path cross-platform
        std::env::current_dir().unwrap().join("test_project")
    }

    #[test]
    fn test_resolve_path_allows_relative_within_base() {
        let base = test_base();
        let ctx = ToolContext::new().with_working_dir(base.clone());
        let result = resolve_path(&ctx, "src/main.rs").unwrap();
        assert!(result.starts_with(&base));
    }

    #[test]
    fn test_resolve_path_rejects_parent_traversal() {
        let base = test_base();
        let ctx = ToolContext::new().with_working_dir(base);
        let result = resolve_path(&ctx, "../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_path_rejects_absolute_outside_base() {
        let base = test_base();
        let ctx = ToolContext::new().with_working_dir(base);
        #[cfg(unix)]
        let result = resolve_path(&ctx, "/etc/passwd");
        #[cfg(windows)]
        let result = resolve_path(&ctx, r"C:\Windows\System32\calc.exe");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_path_allows_absolute_within_base() {
        let base = test_base();
        let ctx = ToolContext::new().with_working_dir(base.clone());
        let abs = base.join("src/main.rs");
        let result = resolve_path(&ctx, abs.to_str().unwrap()).unwrap();
        assert!(result.starts_with(&base));
    }

    #[test]
    fn test_resolve_path_rejects_deep_traversal() {
        let base = test_base();
        let ctx = ToolContext::new().with_working_dir(base);
        let result = resolve_path(&ctx, "src/../../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_resolve_path_allows_dot_relative() {
        let base = test_base();
        let ctx = ToolContext::new().with_working_dir(base.clone());
        let result = resolve_path(&ctx, "./src/main.rs").unwrap();
        assert!(result.starts_with(&base));
    }

    #[test]
    fn test_normalize_path_resolves_dotdot() {
        let path = PathBuf::from("/a/b/c/../../d");
        let norm = normalize_path(&path);
        let expected = PathBuf::from("/a/d");
        assert_eq!(norm, expected);
    }
}
