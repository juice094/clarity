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
    /// Whether to create a git worktree for filesystem isolation.
    /// When enabled, the subagent operates in `.clarity/worktrees/<agent_id>/`
    /// and its sandbox_dir is automatically set to the worktree root.
    pub enable_worktree: bool,
}

impl CapabilityToken {
    /// Create a new capability token.
    pub fn new(allowed_tools: Vec<String>) -> Self {
        Self {
            allowed_tools,
            sandbox_dir: None,
            read_only: false,
            max_iterations: None,
            enable_worktree: false,
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
            enable_worktree: false,
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

    /// Enable git worktree isolation for filesystem safety.
    ///
    /// When enabled, the subagent runner creates a git worktree under
    /// `.clarity/worktrees/<agent_id>/` and sets `sandbox_dir` to the
    /// worktree root. Cleanup is performed on successful completion;
    /// the worktree is preserved on error for debugging.
    pub fn with_worktree(mut self) -> Self {
        self.enable_worktree = true;
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

    #[test]
    fn tool_not_allowed_is_rejected() {
        let token = CapabilityToken::new(vec!["file_read".to_string()]);
        let err = token
            .verify("file_write", &PathBuf::from("/tmp"))
            .unwrap_err();
        assert!(matches!(err, TokenError::ToolNotAllowed(ref name) if name == "file_write"));
    }

    #[test]
    fn read_only_blocks_write_tools() {
        // ponytail: read_only() only whitelists read tools, so add a write tool
        // first to test the read-only block specifically (not the whitelist block).
        let token = CapabilityToken::read_only().allow_tool("file_write");
        let err = token
            .verify("file_write", &PathBuf::from("/tmp"))
            .unwrap_err();
        assert!(matches!(err, TokenError::ReadOnlyViolation(ref name) if name == "file_write"));
    }

    #[test]
    fn read_only_allows_read_tools() {
        let token = CapabilityToken::read_only();
        assert!(token.verify("file_read", &PathBuf::from("/tmp")).is_ok());
        assert!(token.verify("glob", &PathBuf::from("/tmp")).is_ok());
        assert!(token.verify("grep", &PathBuf::from("/tmp")).is_ok());
    }

    #[test]
    fn empty_allowed_tools_rejects_everything() {
        let token = CapabilityToken::new(vec![]);
        let err = token
            .verify("file_read", &PathBuf::from("/tmp"))
            .unwrap_err();
        assert!(matches!(err, TokenError::ToolNotAllowed(_)));
    }

    #[test]
    fn path_exactly_at_sandbox_boundary_is_allowed() {
        let token = CapabilityToken::new(vec!["file_read".to_string()])
            .with_sandbox_dir(PathBuf::from("/tmp/sandbox"));
        assert!(token
            .verify("file_read", &PathBuf::from("/tmp/sandbox"))
            .is_ok());
        assert!(token
            .verify("file_read", &PathBuf::from("/tmp/sandbox/"))
            .is_ok());
    }

    #[test]
    fn worktree_default_is_disabled() {
        let token = CapabilityToken::new(vec!["file_read".to_string()]);
        assert!(!token.enable_worktree);
        let ro = CapabilityToken::read_only();
        assert!(!ro.enable_worktree);
    }

    #[test]
    fn with_worktree_enables_flag_and_preserves_other_fields() {
        let token = CapabilityToken::new(vec!["file_read".to_string()])
            .with_sandbox_dir(PathBuf::from("/tmp/sandbox"))
            .with_worktree();
        assert!(token.enable_worktree);
        assert!(token.sandbox_dir.is_some());
        assert_eq!(token.allowed_tools, vec!["file_read"]);
    }
}
