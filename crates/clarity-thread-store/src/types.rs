//! Parameter and return types for the thread store.
//!
//! Modeled after `codex_thread_store::types` from the OpenAI Codex project,
//! licensed under Apache-2.0. See `NOTICES.md` for attribution.

use std::collections::HashMap;
use std::path::PathBuf;

use chrono::{DateTime, Utc};
use clarity_contract::{RolloutItem, SessionId, SessionSource, ThreadId, ThreadSource};
use serde::{Deserialize, Serialize};

/// Parameters for creating a new thread.
#[derive(Debug, Clone)]
pub struct CreateThreadParams {
    /// Persistent identifier for the new thread.
    pub thread_id: ThreadId,
    /// Runtime session identifier. In Clarity this is currently the same UUID
    /// as the root thread.
    pub session_id: SessionId,
    /// Thread this one was forked from, if any.
    pub forked_from_id: Option<ThreadId>,
    /// Parent thread in a sub-agent spawn graph, if any.
    pub parent_thread_id: Option<ThreadId>,
    /// Runtime source that created the thread.
    pub source: SessionSource,
    /// Classification of how the thread came to exist.
    pub thread_source: Option<ThreadSource>,
    /// Working directory at thread creation.
    pub cwd: PathBuf,
    /// Process or user that originated the thread.
    pub originator: String,
    /// Version of the Clarity binary that created the thread.
    pub cli_version: String,
    /// Base instructions, if any.
    pub base_instructions: Option<serde_json::Value>,
    /// Dynamic tool specifications, if any.
    pub dynamic_tools: Vec<serde_json::Value>,
    /// Model provider identifier.
    pub model_provider: Option<String>,
    /// Whether to generate memories from this thread.
    pub generate_memories: bool,
    /// Multi-agent protocol version, if applicable.
    pub multi_agent_version: Option<String>,
}

impl Default for CreateThreadParams {
    fn default() -> Self {
        Self {
            thread_id: ThreadId::default(),
            session_id: SessionId::default(),
            forked_from_id: None,
            parent_thread_id: None,
            source: SessionSource::default(),
            thread_source: None,
            cwd: PathBuf::new(),
            originator: String::new(),
            cli_version: String::new(),
            base_instructions: None,
            dynamic_tools: Vec::new(),
            model_provider: None,
            generate_memories: false,
            multi_agent_version: None,
        }
    }
}

/// Parameters for resuming an existing thread.
#[derive(Debug, Clone)]
pub struct ResumeThreadParams {
    /// Thread identifier to resume.
    pub thread_id: ThreadId,
    /// Session identifier that owns the thread.
    pub session_id: SessionId,
}

/// Parameters for appending rollout items to a live thread.
#[derive(Debug, Clone)]
pub struct AppendThreadItemsParams {
    /// Target thread.
    pub thread_id: ThreadId,
    /// Items to append.
    pub items: Vec<RolloutItem>,
}

/// Parameters for loading persisted history.
#[derive(Debug, Clone, Default)]
pub struct LoadThreadHistoryParams {
    /// Thread to load history for.
    pub thread_id: ThreadId,
    /// If set, only return items from turns strictly before this index.
    pub before_turn: Option<usize>,
    /// Whether to include compacted replacement history when present.
    pub include_compacted: bool,
}

/// Parameters for reading a thread summary and optional history.
#[derive(Debug, Clone)]
pub struct ReadThreadParams {
    /// Thread to read.
    pub thread_id: ThreadId,
    /// Whether to include the full rollout history in the response.
    pub include_history: bool,
}

/// Parameters for archiving a thread.
#[derive(Debug, Clone)]
pub struct ArchiveThreadParams {
    /// Thread to archive.
    pub thread_id: ThreadId,
}

/// Parameters for deleting a thread.
#[derive(Debug, Clone)]
pub struct DeleteThreadParams {
    /// Thread to delete.
    pub thread_id: ThreadId,
}

/// Cursor-based pagination parameters for listing threads.
#[derive(Debug, Clone)]
pub struct ListThreadsParams {
    /// Maximum number of threads to return.
    pub limit: usize,
    /// Opaque cursor from a previous page.
    pub cursor: Option<String>,
    /// Whether to include archived threads.
    pub include_archived: bool,
}

impl Default for ListThreadsParams {
    fn default() -> Self {
        Self {
            limit: 100,
            cursor: None,
            include_archived: false,
        }
    }
}

/// A metadata patch applied to a thread.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ThreadMetadataPatch {
    /// New title, if set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// New archived state, if set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub archived: Option<bool>,
    /// Arbitrary extra metadata fields.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Parameters for updating thread metadata.
#[derive(Debug, Clone)]
pub struct UpdateThreadMetadataParams {
    /// Thread to update.
    pub thread_id: ThreadId,
    /// Patch to apply.
    pub patch: ThreadMetadataPatch,
}

/// Persistent thread record returned by read/list operations.
#[derive(Debug, Clone)]
pub struct StoredThread {
    /// Thread identifier.
    pub thread_id: ThreadId,
    /// Session identifier.
    pub session_id: SessionId,
    /// Human-readable title, if known.
    pub title: Option<String>,
    /// Rollout file path, if backed by a rollout file.
    pub rollout_path: Option<PathBuf>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Whether the thread is archived.
    pub archived: bool,
    /// Parent thread identifier, if any.
    pub parent_thread_id: Option<ThreadId>,
    /// Fork source thread identifier, if any.
    pub forked_from_id: Option<ThreadId>,
    /// Full history, if requested.
    pub history: Option<StoredThreadHistory>,
}

/// A page of thread summaries.
#[derive(Debug, Clone, Default)]
pub struct ThreadPage {
    /// Thread summaries for this page.
    pub data: Vec<ThreadSummary>,
    /// Cursor for the next page, if any.
    pub next_cursor: Option<String>,
}

/// Lightweight summary of a thread for list views.
#[derive(Debug, Clone)]
pub struct ThreadSummary {
    /// Thread identifier.
    pub thread_id: ThreadId,
    /// Session identifier.
    pub session_id: SessionId,
    /// Human-readable title.
    pub title: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Whether the thread is archived.
    pub archived: bool,
    /// Parent thread identifier, if any.
    pub parent_thread_id: Option<ThreadId>,
    /// Fork source thread identifier, if any.
    pub forked_from_id: Option<ThreadId>,
}

/// Persisted history of a thread.
#[derive(Debug, Clone, Default)]
pub struct StoredThreadHistory {
    /// Rollout items in chronological order.
    pub items: Vec<RolloutItem>,
}

/// Snapshot mode used when forking a thread.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ForkSnapshot {
    /// Fork a committed prefix ending strictly before the nth user message.
    TruncateBeforeNthUserMessage(usize),
    /// Fork the current persisted history as if the source thread had been
    /// interrupted now.
    Interrupted,
}

/// Parameters for forking a thread.
#[derive(Debug, Clone)]
pub struct ForkThreadParams {
    /// Source thread.
    pub source_thread_id: ThreadId,
    /// Snapshot mode.
    pub snapshot: ForkSnapshot,
    /// New thread identifier. If omitted, one is generated.
    pub new_thread_id: Option<ThreadId>,
}
