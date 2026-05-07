//! Tool implementations for Clarity Core
//!
//! This module contains all built-in tools and the core `Tool` trait.
//! Tools are organized by category:
//! - `file`: File operations (read, edit, write)
//! - `shell`: Shell execution (bash, powershell)
//! - `search`: Search operations (glob, grep)

use serde_json::Value;
use std::path::PathBuf;

pub mod ask_user;
pub mod channel;
pub mod computer;
pub mod cron;
pub mod file;
pub mod media;
pub mod notify;
pub mod plan;
pub mod search;
pub mod shell;
pub mod task;
pub mod team;
pub mod think;
pub mod todo;
pub mod web;
pub mod web_browser;

pub use ask_user::AskUserTool;
pub use channel::ChannelSendTool;
pub use computer::ComputerUseTool;
pub use cron::{CancelCronTool, ListCronTool, ScheduleCronTool};
pub use file::{FileEditTool, FileReadTool, FileWriteTool};
pub use media::ReadMediaFileTool;
pub use notify::{NotifyTool, PushNotificationTool};
pub use plan::PlanTool;
pub use search::{GlobTool, GrepTool};
#[cfg(not(target_os = "windows"))]
pub use shell::BashTool;
pub use shell::PowerShellTool;
pub use task::{TaskCreateTool, TaskListTool, TaskOutputTool, TaskStopTool};
pub use team::{TeamCreateTool, TeamDeleteTool, TeamListTool};
pub use think::ThinkTool;
pub use todo::TodoTool;
pub use web::{WebFetchTool, WebSearchTool};
pub use web_browser::WebBrowserTool;

// Re-export contract types so existing imports continue to work.
pub use clarity_contract::{
    ApprovalMode, BoxedTool, IntoSharedTool, SharedTool, Tool, ToolContext, ToolError, ToolResult,
};

/// Return the base Clarity data directory with robust cross-platform fallback.
///
/// Priority:
/// 1. `dirs::home_dir()` / `.clarity`  (keeps existing behaviour)
/// 2. `dirs::data_dir()` / `clarity`   (platform-standard fallback)
/// 3. `std::env::current_dir()` / `.clarity` (last resort)
pub fn clarity_data_dir() -> ToolResult<PathBuf> {
    if let Some(home) = dirs::home_dir() {
        return Ok(home.join(".clarity"));
    }
    if let Some(data) = dirs::data_dir() {
        return Ok(data.join("clarity"));
    }
    if let Ok(cwd) = std::env::current_dir() {
        return Ok(cwd.join(".clarity"));
    }
    Err(ToolError::execution_failed(
        "Could not determine a writable data directory for Clarity".to_string(),
    ))
}

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
        // Expand leading ~ to home directory (cross-platform)
        let input = if path.starts_with('~') {
            dirs::home_dir()
                .map(|home| {
                    let rest = path[1..].trim_start_matches(['/', '\\']);
                    home.join(rest)
                })
                .unwrap_or_else(|| PathBuf::from(path))
        } else {
            PathBuf::from(path)
        };

        let base = &ctx.working_dir;

        // Allow absolute paths directly — user explicitly requested them
        if input.is_absolute() {
            return Ok(normalize_path(&input));
        }

        // Ensure base is absolute for reliable comparison
        let base_abs = if base.is_absolute() {
            base.clone()
        } else {
            std::env::current_dir()
                .unwrap_or_else(|_| PathBuf::from("."))
                .join(base)
        };

        let resolved = base_abs.join(&input);
        let base_norm = normalize_path(&base_abs);
        let resolved_norm = normalize_path(&resolved);

        if !resolved_norm.starts_with(&base_norm) {
            return Err(ToolError::invalid_params(format!(
                "Path '{}' escapes working directory '{}'",
                path,
                base.display()
            )));
        }

        Ok(resolved_norm)
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
    fn test_resolve_path_expands_tilde() {
        let base = test_base();
        let ctx = ToolContext::new().with_working_dir(base);
        let home = dirs::home_dir().expect("home_dir should be available in tests");
        let result = resolve_path(&ctx, "~/test.txt").unwrap();
        assert_eq!(result, home.join("test.txt"));
    }

    #[test]
    fn test_resolve_path_expands_tilde_backslash() {
        let base = test_base();
        let ctx = ToolContext::new().with_working_dir(base);
        let home = dirs::home_dir().expect("home_dir should be available in tests");
        let result = resolve_path(&ctx, "~\\test.txt").unwrap();
        assert_eq!(result, home.join("test.txt"));
    }

    #[test]
    fn test_resolve_path_allows_absolute_outside_base() {
        let base = test_base();
        let ctx = ToolContext::new().with_working_dir(base);
        #[cfg(unix)]
        let result = resolve_path(&ctx, "/etc/passwd");
        #[cfg(windows)]
        let result = resolve_path(&ctx, r"C:\Windows\System32\drivers\etc\hosts");
        assert!(result.is_ok());
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
    fn test_resolve_path_rejects_relative_traversal() {
        let base = test_base();
        let ctx = ToolContext::new().with_working_dir(base.clone());
        let result = resolve_path(&ctx, "../outside.rs");
        assert!(result.is_err());
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
