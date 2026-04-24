//! Capability Token for subagent permission isolation

use crate::tools::helpers::normalize_path;
use crate::tools::ToolContext;
use std::path::PathBuf;

/// Error type for capability token verification
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenError {
    /// Tool is not in the allowed whitelist
    ToolNotAllowed(String),
    /// Write operation is forbidden in read-only mode
    ReadOnlyViolation(String),
    /// Operation is outside the sandbox directory
    SandboxEscape {
        /// The tool being invoked
        tool: String,
        /// The path that escaped the sandbox
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

/// Capability token for subagent permission isolation
///
/// Controls which tools a subagent can invoke, whether write operations
/// are permitted, and the sandbox directory boundary.
#[derive(Debug, Clone)]
pub struct CapabilityToken {
    /// Allowed tool whitelist
    pub allowed_tools: Vec<String>,
    /// Sandbox directory (if set, operations must stay within)
    pub sandbox_dir: Option<PathBuf>,
    /// Read-only flag (blocks write tools)
    pub read_only: bool,
    /// Optional maximum iteration limit
    pub max_iterations: Option<usize>,
}

impl CapabilityToken {
    /// Create a new capability token
    pub fn new(allowed_tools: Vec<String>) -> Self {
        Self {
            allowed_tools,
            sandbox_dir: None,
            read_only: false,
            max_iterations: None,
        }
    }

    /// Create a read-only token that only allows file read and search tools
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

    /// Set sandbox directory
    pub fn with_sandbox_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.sandbox_dir = Some(dir.into());
        self
    }

    /// Set read-only mode
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }

    /// Set maximum iterations limit
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = Some(max);
        self
    }

    /// Verify if a tool call is permitted by this token
    pub fn verify(&self, tool_name: &str, ctx: &ToolContext) -> Result<(), TokenError> {
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
            let working_abs = normalize_path(&cwd.join(&ctx.working_dir));

            if !working_abs.starts_with(&sandbox_abs) {
                return Err(TokenError::SandboxEscape {
                    tool: tool_name.to_string(),
                    path: ctx.working_dir.clone(),
                });
            }
        }

        Ok(())
    }
}

/// Check if a tool is a write tool
fn is_write_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "file_write" | "file_edit" | "bash" | "powershell"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::ToolContext;
    use std::path::PathBuf;

    #[test]
    fn test_token_new() {
        let token = CapabilityToken::new(vec!["file_read".to_string()]);
        assert_eq!(token.allowed_tools, vec!["file_read"]);
        assert!(!token.read_only);
        assert!(token.sandbox_dir.is_none());
        assert!(token.max_iterations.is_none());
    }

    #[test]
    fn test_token_read_only() {
        let token = CapabilityToken::read_only();
        assert!(token.read_only);
        assert!(token.allowed_tools.contains(&"file_read".to_string()));
        assert!(!token.allowed_tools.contains(&"file_write".to_string()));
    }

    #[test]
    fn test_verify_allowed_tool() {
        let token = CapabilityToken::new(vec!["file_read".to_string()]);
        let ctx = ToolContext::new();
        assert!(token.verify("file_read", &ctx).is_ok());
    }

    #[test]
    fn test_verify_disallowed_tool() {
        let token = CapabilityToken::new(vec!["file_read".to_string()]);
        let ctx = ToolContext::new();
        let result = token.verify("bash", &ctx);
        assert!(matches!(result, Err(TokenError::ToolNotAllowed(ref t)) if t == "bash"));
    }

    #[test]
    fn test_verify_read_only_blocks_write() {
        let token = CapabilityToken::new(vec!["file_read".to_string(), "file_write".to_string()])
            .with_read_only(true);
        let ctx = ToolContext::new();
        let result = token.verify("file_write", &ctx);
        assert!(matches!(result, Err(TokenError::ReadOnlyViolation(ref t)) if t == "file_write"));
    }

    #[test]
    fn test_verify_sandbox_escape() {
        let token = CapabilityToken::new(vec!["file_read".to_string()])
            .with_sandbox_dir(PathBuf::from("/tmp/sandbox"));
        let ctx = ToolContext::new().with_working_dir(PathBuf::from("/tmp/other"));
        let result = token.verify("file_read", &ctx);
        assert!(
            matches!(result, Err(TokenError::SandboxEscape { ref tool, .. }) if tool == "file_read"),
            "Expected SandboxEscape, got {:?}",
            result
        );
    }

    #[test]
    fn test_verify_sandbox_within() {
        let token = CapabilityToken::new(vec!["file_read".to_string()])
            .with_sandbox_dir(PathBuf::from("/tmp/sandbox"));
        let ctx = ToolContext::new().with_working_dir(PathBuf::from("/tmp/sandbox/sub"));
        assert!(token.verify("file_read", &ctx).is_ok());
    }

    #[test]
    fn test_token_display() {
        let err = TokenError::ToolNotAllowed("bash".to_string());
        assert!(err.to_string().contains("bash"));

        let err = TokenError::ReadOnlyViolation("file_write".to_string());
        assert!(err.to_string().contains("read-only"));

        let err = TokenError::SandboxEscape {
            tool: "file_read".to_string(),
            path: PathBuf::from("/etc/passwd"),
        };
        assert!(err.to_string().contains("Sandbox escape"));
    }

    #[test]
    fn test_max_iterations_builder() {
        let token = CapabilityToken::new(vec!["think".to_string()]).with_max_iterations(5);
        assert_eq!(token.max_iterations, Some(5));
    }
}
