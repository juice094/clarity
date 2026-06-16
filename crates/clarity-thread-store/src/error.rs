//! Errors returned by thread-store operations.
//!
//! Modeled after `codex_thread_store::ThreadStoreError` from the OpenAI Codex
//! project, licensed under Apache-2.0. See `NOTICES.md` for attribution.

use std::path::PathBuf;

/// Result type alias for thread-store operations.
pub type ThreadStoreResult<T> = Result<T, ThreadStoreError>;

/// Errors that can occur when interacting with a [`ThreadStore`](crate::ThreadStore).
#[derive(Debug, thiserror::Error)]
pub enum ThreadStoreError {
    /// An I/O operation failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// The requested thread could not be found.
    #[error("thread not found: {thread_id}")]
    NotFound {
        /// Identifier of the missing thread.
        thread_id: String,
    },

    /// The thread exists but its rollout data is corrupt or unreadable.
    #[error("invalid rollout for thread {thread_id}: {message}")]
    InvalidRollout {
        /// Identifier of the affected thread.
        thread_id: String,
        /// Human-readable description of the problem.
        message: String,
    },

    /// A thread with the requested identifier already exists.
    #[error("thread already exists: {thread_id}")]
    Duplicate {
        /// Identifier of the duplicate thread.
        thread_id: String,
    },

    /// The requested operation is not supported by this store implementation.
    #[error("unsupported operation: {operation}")]
    Unsupported {
        /// Name of the unsupported operation.
        operation: &'static str,
    },

    /// JSON serialization or deserialization failed.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    /// SQLite operation failed.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// The supplied path is not a valid rollout file.
    #[error("invalid rollout path: {path:?}")]
    InvalidPath {
        /// Supplied path.
        path: PathBuf,
    },

    /// A catch-all for store-specific failures.
    #[error("{0}")]
    Other(String),
}

impl ThreadStoreError {
    /// Create a `NotFound` error from a thread identifier.
    pub fn not_found(thread_id: impl Into<String>) -> Self {
        Self::NotFound {
            thread_id: thread_id.into(),
        }
    }

    /// Create an `InvalidRollout` error from a thread identifier and message.
    pub fn invalid_rollout(thread_id: impl Into<String>, message: impl Into<String>) -> Self {
        Self::InvalidRollout {
            thread_id: thread_id.into(),
            message: message.into(),
        }
    }

    /// Create a `Duplicate` error from a thread identifier.
    pub fn duplicate(thread_id: impl Into<String>) -> Self {
        Self::Duplicate {
            thread_id: thread_id.into(),
        }
    }
}
