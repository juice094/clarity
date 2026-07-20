// =============================================================================
// Error Types
// =============================================================================

/// Error type for MCP client operations.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    /// Failed to establish or maintain a connection.
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Transport configuration is invalid or unsupported.
    #[error("Invalid transport: {0}")]
    InvalidTransport(String),

    /// Spawned command was rejected by the security validator.
    #[error("Command not allowed: {0}")]
    CommandNotAllowed(String),

    /// Request reached the server but failed.
    #[error("Request failed: {0}")]
    RequestFailed(String),

    /// Request exceeded the configured timeout.
    #[error("Request timeout")]
    RequestTimeout,

    /// Response could not be parsed.
    #[error("Invalid response: {0}")]
    InvalidResponse(String),

    /// Server returned an RPC error.
    #[error("RPC error: {0}")]
    RpcError(String),

    /// JSON serialization or deserialization failed.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Underlying I/O error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
