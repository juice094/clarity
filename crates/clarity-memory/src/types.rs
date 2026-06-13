//! Core types for the memory system

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A fact stored in the memory system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Fact {
    /// Unique fact identifier
    pub id: i64,
    /// The fact content
    pub fact: String,
    /// Tags associated with the fact
    pub tags: Vec<String>,
    /// Optional time reference
    pub time: Option<String>,
    /// Optional originating session identifier
    pub session_id: Option<String>,
    /// Timestamp when the fact was created
    pub created_at: DateTime<Utc>,
}

/// A meta-fact extracted by LLM
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetaFact {
    /// The extracted fact content
    pub fact: String,
    /// Tags categorizing the fact
    pub tags: Vec<String>,
    /// Optional time reference
    pub time: Option<String>,
}

/// A chat message in a session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    /// Message role (e.g. "user", "assistant", "system")
    pub role: String,
    /// Message content
    pub content: String,
    /// Timestamp when the message was created
    pub timestamp: DateTime<Utc>,
}

impl Message {
    /// Create a new message with the given role and content
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            timestamp: Utc::now(),
        }
    }

    /// Create a new user message
    pub fn user(content: impl Into<String>) -> Self {
        Self::new("user", content)
    }

    /// Create a new assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new("assistant", content)
    }

    /// Create a new system message
    pub fn system(content: impl Into<String>) -> Self {
        Self::new("system", content)
    }
}

/// Status of a compilation operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CompileStatus {
    /// Compilation succeeded with new content
    Success {
        /// Content fingerprint
        fingerprint: String,
    },
    /// Compilation skipped (no changes detected)
    Skipped {
        /// Content fingerprint
        fingerprint: String,
    },
    /// Compilation failed
    Failed {
        /// Error message
        error: String,
    },
}

impl CompileStatus {
    /// Return the fingerprint, if any
    pub fn fingerprint(&self) -> Option<&str> {
        match self {
            CompileStatus::Success { fingerprint } | CompileStatus::Skipped { fingerprint } => {
                Some(fingerprint)
            }
            CompileStatus::Failed { .. } => None,
        }
    }

    /// Check whether the compilation succeeded
    pub fn is_success(&self) -> bool {
        matches!(self, CompileStatus::Success { .. })
    }

    /// Check whether the compilation was skipped
    pub fn is_skipped(&self) -> bool {
        matches!(self, CompileStatus::Skipped { .. })
    }
}

impl fmt::Display for CompileStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CompileStatus::Success { fingerprint } => {
                write!(
                    f,
                    "Success(fingerprint={})",
                    &fingerprint[..8.min(fingerprint.len())]
                )
            }
            CompileStatus::Skipped { fingerprint } => {
                write!(
                    f,
                    "Skipped(fingerprint={})",
                    &fingerprint[..8.min(fingerprint.len())]
                )
            }
            CompileStatus::Failed { error } => write!(f, "Failed(error={})", error),
        }
    }
}

/// Configuration for memory compilation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompileConfig {
    /// Number of turns before triggering a summary
    pub turns_per_summary: u32,
    /// Maximum tokens for the today-level summary
    pub max_tokens_today: usize,
    /// Maximum tokens for the week-level summary
    pub max_tokens_week: usize,
    /// Maximum tokens for the long-term summary
    pub max_tokens_longterm: usize,
    /// Model to use for compilation
    pub compile_model: String,
    /// Model to use for fact extraction
    pub extractor_model: String,
}

impl Default for CompileConfig {
    fn default() -> Self {
        Self {
            turns_per_summary: 6,
            max_tokens_today: 2048,
            max_tokens_week: 2048,
            max_tokens_longterm: 2048,
            compile_model: "gpt-4".to_string(),
            extractor_model: "gpt-4".to_string(),
        }
    }
}

/// A record in a session file (JSONL format)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SessionRecord {
    /// A stored message
    Message {
        /// The stored message
        message: MessageRecord,
        /// Timestamp when the record was written
        timestamp: DateTime<Utc>,
    },
    /// A stored summary
    Summary {
        /// Summary content
        content: String,
        /// Timestamp when the summary was written
        timestamp: DateTime<Utc>,
    },
}

/// A serializable message record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRecord {
    /// Message role
    pub role: String,
    /// Message content
    pub content: String,
}

impl From<Message> for SessionRecord {
    fn from(msg: Message) -> Self {
        SessionRecord::Message {
            message: MessageRecord {
                role: msg.role,
                content: msg.content,
            },
            timestamp: msg.timestamp,
        }
    }
}

impl From<SessionRecord> for Option<Message> {
    fn from(record: SessionRecord) -> Self {
        match record {
            SessionRecord::Message { message, timestamp } => Some(Message {
                role: message.role,
                content: message.content,
                timestamp,
            }),
            SessionRecord::Summary { .. } => None,
        }
    }
}

/// Errors that can occur in the memory system
#[derive(thiserror::Error, Debug)]
pub enum MemoryError {
    /// Database error
    #[cfg(feature = "sqlite")]
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// LLM client error
    #[error("LLM client error: {0}")]
    LlmClient(String),

    /// Compilation error
    #[error("Compilation error: {0}")]
    Compilation(String),

    /// Requested session was not found
    #[error("Session not found: {0}")]
    SessionNotFound(String),

    /// Invalid input
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Storage error
    #[error("Storage error: {0}")]
    Storage(String),
}

/// Structured notes extracted from a single conversation turn.
/// Used by TurnMemoryExtractor to persist key information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionNotes {
    /// Originating session identifier
    pub session_id: String,
    /// Summary of the current session state
    pub current_state: String,
    /// Errors encountered during the turn
    pub errors: Vec<String>,
    /// Key learnings from the turn
    pub learnings: Vec<String>,
    /// Key results produced during the turn
    pub key_results: Vec<String>,
    /// Timestamp when the notes were created
    pub created_at: DateTime<Utc>,
}

impl SessionNotes {
    /// Create empty notes for the given session
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
            current_state: String::new(),
            errors: Vec::new(),
            learnings: Vec::new(),
            key_results: Vec::new(),
            created_at: Utc::now(),
        }
    }
}

/// A flashcard for spaced-repetition review (Anki-compatible)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Flashcard {
    /// Front side of the card
    pub front: String,
    /// Back side of the card
    pub back: String,
    /// Comma-separated tags
    pub tags: String,
}

/// Result type alias for memory operations
pub type Result<T> = std::result::Result<T, MemoryError>;
