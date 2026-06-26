//! Capability token types for permission isolation.
//!
//! These types control which tools a subagent can invoke, whether write
//! operations are permitted, and the sandbox directory boundary.

use std::path::{Path, PathBuf};

// ponytail: minimal path normalizer that does not require the path to exist and
// avoids UNC prefixes on Windows. Replace with std::path::absolute only after
// proving equivalent sandbox-escape behavior across platforms.
fn normalize_path(path: &Path) -> PathBuf {
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

/// Check if a tool is a write tool.
fn is_write_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "file_write" | "file_edit" | "bash" | "powershell"
    )
}

/// Error type for capability token verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenError {
    /// Tool is not in the allowed whitelist.
    ToolNotAllowed(String),
    /// Write operation is forbidden in read-only mode.
    ReadOnlyViolation(String),
    /// Operation is outside the sandbox directory.
    SandboxEscape {
        /// The tool being invoked.
        tool: String,
        /// The path that escaped the sandbox.
        path: PathBuf,
    },
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenError::ToolNotAllowed(name) => {
                write!(f, "Tool '{}' is not allowed by capability token", name)
            }
            TokenError::ReadOnlyViolation(name) => {
                write!(f, "Tool '{}' is blocked in read-only mode", name)
            }
            TokenError::SandboxEscape { tool, path } => {
                write!(
                    f,
                    "Sandbox escape for tool '{}': path '{}' is outside sandbox",
                    tool,
                    path.display()
                )
            }
        }
    }
}

impl std::error::Error for TokenError {}

/// Capability token for subagent permission isolation.
///
/// Controls which tools a subagent can invoke, whether write operations
/// are permitted, and the sandbox directory boundary.
#[derive(Debug, Clone)]
pub struct CapabilityToken {
    /// Allowed tool whitelist.
    pub allowed_tools: Vec<String>,
    /// Sandbox directory (if set, operations must stay within).
    pub sandbox_dir: Option<PathBuf>,
    /// Read-only flag (blocks write tools).
    pub read_only: bool,
    /// Optional maximum iteration limit.
    pub max_iterations: Option<usize>,
}

impl CapabilityToken {
    /// Create a new capability token.
    pub fn new(allowed_tools: Vec<String>) -> Self {
        Self {
            allowed_tools,
            sandbox_dir: None,
            read_only: false,
            max_iterations: None,
        }
    }

    /// Create a read-only token that only allows file read and search tools.
    pub fn read_only() -> Self {
        Self {
            allowed_tools: vec![
                "file_read".to_string(),
                "glob".to_string(),
                "grep".to_string(),
                "think".to_string(),
            ],
            sandbox_dir: None,
            read_only: true,
            max_iterations: None,
        }
    }

    /// Set sandbox directory.
    pub fn with_sandbox_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.sandbox_dir = Some(dir.into());
        self
    }

    /// Set read-only mode.
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Set maximum iterations limit.
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = Some(max);
        self
    }

    /// Allow a specific tool.
    pub fn allow_tool(mut self, name: impl Into<String>) -> Self {
        self.allowed_tools.push(name.into());
        self
    }

    /// Verify if a tool call is permitted by this token.
    pub fn verify(&self, tool_name: &str, working_dir: &Path) -> Result<(), TokenError> {
        // 1. Check whitelist
        if !self.allowed_tools.iter().any(|t| t == tool_name) {
            return Err(TokenError::ToolNotAllowed(tool_name.to_string()));
        }

        // 2. Check read-only
        if self.read_only && is_write_tool(tool_name) {
            return Err(TokenError::ReadOnlyViolation(tool_name.to_string()));
        }

        // 3. Check sandbox
        if let Some(ref sandbox) = self.sandbox_dir {
            let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
            let sandbox_abs = normalize_path(&cwd.join(sandbox));
            let working_abs = normalize_path(&cwd.join(working_dir));

            if !working_abs.starts_with(&sandbox_abs) {
                return Err(TokenError::SandboxEscape {
                    tool: tool_name.to_string(),
                    path: working_dir.to_path_buf(),
                });
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn sandbox_escape_via_parent_dir_is_blocked() {
        let token = CapabilityToken::new(vec!["file_read".to_string()])
            .with_sandbox_dir(PathBuf::from("/tmp/sandbox"));
        let escaped = PathBuf::from("/tmp/sandbox/../etc/passwd");
        assert!(
            matches!(
                token.verify("file_read", &escaped),
                Err(TokenError::SandboxEscape { .. })
            ),
            "parent-dir escape should be blocked"
        );
    }

    #[test]
    fn sandbox_subdir_is_allowed() {
        let token = CapabilityToken::new(vec!["file_read".to_string()])
            .with_sandbox_dir(PathBuf::from("/tmp/sandbox"));
        let subdir = PathBuf::from("/tmp/sandbox/src/main.rs");
        assert!(token.verify("file_read", &subdir).is_ok());
    }

    #[test]
    fn normalize_path_resolves_parent_dir() {
        let base = PathBuf::from("/tmp/sandbox");
        let escaped = base.join("../etc/passwd");
        let normalized = normalize_path(&escaped);
        assert_eq!(normalized, PathBuf::from("/tmp/etc/passwd"));
    }
}
