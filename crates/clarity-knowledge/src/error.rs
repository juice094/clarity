//! Error types for `clarity-knowledge`.

use std::path::PathBuf;

/// Result type alias for knowledge operations.
pub type Result<T> = std::result::Result<T, KnowledgeError>;

/// Errors that can occur when indexing or querying knowledge.
#[derive(Debug, thiserror::Error)]
pub enum KnowledgeError {
    /// An I/O operation failed.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A SQLite operation failed.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),

    /// Failed to parse YAML frontmatter.
    #[error("yaml parse error in {path}: {source}")]
    Yaml {
        /// Path to the file that failed to parse.
        path: PathBuf,
        /// Underlying YAML error.
        #[source]
        source: serde_yaml::Error,
    },

    /// The knowledge index has not been initialized.
    #[error("knowledge index not initialized")]
    NotInitialized,

    /// The requested source was not found.
    #[error("source not found: {0}")]
    SourceNotFound(String),

    /// A path is outside all configured knowledge sources.
    #[error("path outside knowledge sources: {0}")]
    PathOutsideSources(PathBuf),

    /// A requested operation is not yet implemented.
    #[error("not implemented: {0}")]
    NotImplemented(String),

    /// A local embedding operation failed (feature `local-embedding`).
    #[error("embedding error: {0}")]
    Embedding(String),
}

impl KnowledgeError {
    /// Create a YAML error bound to a specific file path.
    pub fn yaml(path: impl Into<PathBuf>, source: serde_yaml::Error) -> Self {
        Self::Yaml {
            path: path.into(),
            source,
        }
    }
}
