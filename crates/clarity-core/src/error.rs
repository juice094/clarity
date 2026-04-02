//! Error types for clarity-core

use thiserror::Error;

/// Errors that can occur during tool execution
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
}

/// Errors that can occur in the Agent
#[derive(Error, Debug)]
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
}

/// Result type for tool operations
pub type ToolResult<T> = Result<T, ToolError>;

/// Result type for agent operations
pub type AgentResult<T> = Result<T, AgentError>;
