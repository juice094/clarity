//! Error types for the Clarity contract layer.
//!
//! These errors are designed to be shared across all crates in the workspace.
//! `AgentError` is the primary error type used by the agent loop and LLM providers.

use thiserror::Error;

/// Errors that can occur during tool execution.
#[derive(Error, Debug, Clone)]
pub enum ToolError {
    /// Invalid parameters provided to the tool
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),

    /// Tool execution failed
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    /// Tool not found in registry
    #[error("Tool not found: {0}")]
    NotFound(String),

    /// I/O error during execution
    #[error("I/O error: {0}")]
    IoError(String),

    /// Timeout during execution
    #[error("Execution timeout after {0} seconds")]
    Timeout(u64),

    /// Permission denied
    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    /// Tool is not available (e.g., missing binary)
    #[error("Tool unavailable: {0}")]
    Unavailable(String),
}

impl ToolError {
    /// Create an invalid parameters error
    pub fn invalid_params<S: Into<String>>(msg: S) -> Self {
        Self::InvalidParameters(msg.into())
    }

    /// Create an execution failed error
    pub fn execution_failed<S: Into<String>>(msg: S) -> Self {
        Self::ExecutionFailed(msg.into())
    }

    /// Create a not found error
    pub fn not_found<S: Into<String>>(name: S) -> Self {
        Self::NotFound(name.into())
    }

    /// Create an I/O error from std::io::Error
    pub fn from_io(err: std::io::Error) -> Self {
        Self::IoError(err.to_string())
    }

    /// Whether this error may resolve itself if retried
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Self::IoError(_) | Self::Timeout(_) | Self::Unavailable(_)
        )
    }

    /// Sanitize absolute paths from error messages.
    pub fn sanitize_paths(&self) -> Self {
        match self {
            Self::InvalidParameters(m) => Self::InvalidParameters(sanitize_path_str(m)),
            Self::ExecutionFailed(m) => Self::ExecutionFailed(sanitize_path_str(m)),
            Self::NotFound(m) => Self::NotFound(sanitize_path_str(m)),
            Self::IoError(m) => Self::IoError(sanitize_path_str(m)),
            Self::PermissionDenied(m) => Self::PermissionDenied(sanitize_path_str(m)),
            Self::Unavailable(m) => Self::Unavailable(sanitize_path_str(m)),
            Self::Timeout(s) => Self::Timeout(*s),
        }
    }
}

/// Errors that can occur in the Agent or LLM providers.
///
/// This is the primary error type for the entire Clarity ecosystem.
/// It lives in the contract layer so that downstream crates can handle
/// errors without depending on `clarity-core`.
#[derive(Error, Debug, Clone)]
pub enum AgentError {
    /// Tool execution error
    #[error("Tool error: {0}")]
    Tool(#[from] ToolError),

    /// Registry error
    #[error("Registry error: {0}")]
    Registry(String),

    /// Duplicate tool registration
    #[error("Duplicate tool: {0}")]
    DuplicateTool(String),

    /// Tool execution failed
    #[error("Tool '{0}' execution failed: {1}")]
    ToolExecutionFailed(String, String),

    /// LLM communication error
    #[error("LLM error: {0}")]
    Llm(String),

    /// Maximum iterations exceeded
    #[error("Maximum iterations ({0}) exceeded")]
    MaxIterationsExceeded(usize),

    /// Max iterations reached
    #[error("Maximum iterations reached")]
    MaxIterationsReached,

    /// Context too large
    #[error("Context size exceeded maximum")]
    ContextOverflow,

    /// Invalid response from LLM
    #[error("Invalid LLM response: {0}")]
    InvalidResponse(String),

    /// Operation cancelled
    #[error("Operation cancelled")]
    Cancelled,

    /// Agent not configured with an LLM provider
    #[error("Agent is not configured with an LLM provider")]
    Unconfigured,

    /// Agent is already running a turn
    #[error("Agent is already running a turn")]
    AlreadyRunning,

    /// Agent is in a stalled state and needs reset
    #[error("Agent is in a stalled state; call reset() first")]
    Stalled,

    /// Federation communication error
    #[error("Federation error: {0}")]
    Federation(String),

    /// Flow execution error
    #[error("Flow execution error: {0}")]
    FlowExecution(String),

    /// Budget exceeded (per-turn or per-day)
    #[error("Budget exceeded: limit=${limit:.4}, current=${current:.4}, requested=${requested:.4}")]
    BudgetExceeded {
        /// The limit that was exceeded (USD)
        limit: f64,
        /// Current accumulated cost (USD)
        current: f64,
        /// Cost of the requested operation (USD)
        requested: f64,
    },
}

impl AgentError {
    /// Create an LLM error
    pub fn llm<S: Into<String>>(msg: S) -> Self {
        Self::Llm(msg.into())
    }

    /// Create a registry error
    pub fn registry<S: Into<String>>(msg: S) -> Self {
        Self::Registry(msg.into())
    }

    /// Whether this error may resolve itself if retried
    pub fn is_recoverable(&self) -> bool {
        match self {
            Self::Tool(te) => te.is_recoverable(),
            Self::Llm(_) | Self::Federation(_) => true,
            _ => false,
        }
    }
}

/// Sanitize absolute paths from a string to prevent leaking
/// host directory structure (e.g. `C:\Users\<name>` -> `~`).
pub fn sanitize_path_str(s: &str) -> String {
    let mut out = s.to_string();
    // Replace home directory with ~ (covers both Unix and Windows)
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy().to_string();
        out = out.replace(&home_str, "~");
    }
    // Replace any remaining Windows-style absolute paths
    // Heuristic: look for `C:\` or similar drive letters
    out.split_whitespace()
        .map(|word| {
            if word.len() > 3 && word.as_bytes()[1] == b':' && word.as_bytes()[2] == b'\\' {
                "<absolute-path>".to_string()
            } else {
                word.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Result type for tool operations.
pub type ToolResult<T> = Result<T, ToolError>;

/// Alias for `AgentError` — used in federation contexts where a generic
/// contract-level error is preferred.
pub type ContractError = AgentError;

/// Result alias using `ContractError`.
pub type ContractResult<T> = Result<T, ContractError>;
