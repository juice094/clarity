//! Core types for the memory system

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;

/// A fact stored in the memory system
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Fact {
    pub id: i64,
    pub fact: String,
    pub tags: Vec<String>,
    pub time: Option<String>,
    pub session_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

/// A meta-fact extracted by LLM
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MetaFact {
    pub fact: String,
    pub tags: Vec<String>,
    pub time: Option<String>,
}

/// A chat message in a session
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub role: String,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

impl Message {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            timestamp: Utc::now(),
        }
    }

    pub fn user(content: impl Into<String>) -> Self {
        Self::new("user", content)
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new("assistant", content)
    }

    pub fn system(content: impl Into<String>) -> Self {
        Self::new("system", content)
    }
}

/// Status of a compilation operation
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum CompileStatus {
    /// Compilation succeeded with new content
    Success { fingerprint: String },
    /// Compilation skipped (no changes detected)
    Skipped { fingerprint: String },
    /// Compilation failed
    Failed { error: String },
}

impl CompileStatus {
    pub fn fingerprint(&self) -> Option<&str> {
        match self {
            CompileStatus::Success { fingerprint } | CompileStatus::Skipped { fingerprint } => {
                Some(fingerprint)
            }
            CompileStatus::Failed { .. } => None,
        }
    }

    pub fn is_success(&self) -> bool {
        matches!(self, CompileStatus::Success { .. })
    }

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
    /// Maximum tokens for each summary level
    pub max_tokens_today: usize,
    pub max_tokens_week: usize,
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
    Message {
        message: MessageRecord,
        timestamp: DateTime<Utc>,
    },
    Summary {
        content: String,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRecord {
    pub role: String,
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
    #[cfg(feature = "sqlite")]
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("LLM client error: {0}")]
    LlmClient(String),

    #[error("Compilation error: {0}")]
    Compilation(String),

    #[error("Session not found: {0}")]
    SessionNotFound(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Storage error: {0}")]
    Storage(String),
}

/// Structured notes extracted from a single conversation turn.
/// Used by TurnMemoryExtractor to persist key information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionNotes {
    pub session_id: String,
    pub current_state: String,
    pub errors: Vec<String>,
    pub learnings: Vec<String>,
    pub key_results: Vec<String>,
    pub created_at: DateTime<Utc>,
}

impl SessionNotes {
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
    pub front: String,
    pub back: String,
    pub tags: String,
}

pub type Result<T> = std::result::Result<T, MemoryError>;
