//! Session metadata and lifecycle for `SessionStoreV2`.

use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension};

use crate::session_store_v2::schema::{INDEXES_SQL, TABLES_SQL, home_dir};
use crate::types::Result as MemoryResult;

// ============================================================================
// SessionStoreV2
// ============================================================================

/// Unified SQLite session store (V2).
pub struct SessionStoreV2 {
    /// The underlying SQLite connection. Visible to other `session_store_v2`
    /// submodules so that `impl` blocks can be split by domain.
    pub(crate) conn: Connection,
}

impl SessionStoreV2 {
    /// Open or create a V2 session store at the given path.
    pub fn new(path: impl AsRef<Path>) -> MemoryResult<Self> {
        let path = path.as_ref().to_path_buf();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let conn = Connection::open(&path)?;
        // SAFETY: PRAGMA journal_mode may return a result row on some SQLite builds.
        // We use execute_batch to silently ignore it; failure is non-fatal.
        let _ = conn.execute_batch("PRAGMA journal_mode=WAL;");
        let _ = conn.execute_batch("PRAGMA foreign_keys=ON;");

        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Default path: `~/.clarity/sessions_v2.sqlite`.
    pub fn default_path() -> PathBuf {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".clarity")
            .join("sessions_v2.sqlite")
    }

    fn init_schema(&self) -> MemoryResult<()> {
        for sql in TABLES_SQL {
            self.conn.execute(sql, [])?;
        }
        for sql in INDEXES_SQL {
            self.conn.execute(sql, [])?;
        }
        Ok(())
    }

    // ------------------------------------------------------------------
    // Session CRUD
    // ------------------------------------------------------------------

    /// Create a new session.
    pub fn create_session(
        &self,
        id: &str,
        title: Option<&str>,
        soul_id: Option<&str>,
        config_hash: Option<&str>,
    ) -> MemoryResult<()> {
        let now = Utc::now().timestamp_millis();
        self.conn.execute(
            "INSERT INTO sessions_v2 (id, title, soul_id, created_at, updated_at, state, config_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                id,
                title,
                soul_id,
                now,
                now,
                "active",
                config_hash,
            ],
        )?;
        Ok(())
    }

    /// Get session metadata.
    pub fn get_session(&self, id: &str) -> MemoryResult<Option<SessionV2>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, title, soul_id, created_at, updated_at, parent_session_id, state, config_hash
             FROM sessions_v2 WHERE id = ?1"
        )?;
        let row = stmt
            .query_row([id], |row| {
                Ok(SessionV2 {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    soul_id: row.get(2)?,
                    created_at: row.get(3)?,
                    updated_at: row.get(4)?,
                    parent_session_id: row.get(5)?,
                    state: row
                        .get::<_, String>(6)?
                        .parse()
                        .unwrap_or(SessionState::Active),
                    config_hash: row.get(7)?,
                })
            })
            .optional()?;
        Ok(row)
    }

    /// Update session state.
    pub fn set_session_state(&self, id: &str, state: SessionState) -> MemoryResult<()> {
        let now = Utc::now().timestamp_millis();
        self.conn.execute(
            "UPDATE sessions_v2 SET state = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![state.as_str(), now, id],
        )?;
        Ok(())
    }

    /// List all session IDs.
    pub fn list_sessions(&self, state_filter: Option<SessionState>) -> MemoryResult<Vec<String>> {
        let sql = match state_filter {
            Some(_) => "SELECT id FROM sessions_v2 WHERE state = ?1 ORDER BY updated_at DESC",
            None => "SELECT id FROM sessions_v2 ORDER BY updated_at DESC",
        };
        let mut stmt = self.conn.prepare(sql)?;
        let ids: Vec<String> = match state_filter {
            Some(s) => stmt
                .query_map([s.as_str()], |row| row.get(0))?
                .collect::<Result<_, _>>()?,
            None => stmt
                .query_map([], |row| row.get(0))?
                .collect::<Result<_, _>>()?,
        };
        Ok(ids)
    }

    /// Set parent session (handoff lineage).
    pub fn set_parent(&self, session_id: &str, parent_id: &str) -> MemoryResult<()> {
        self.conn.execute(
            "UPDATE sessions_v2 SET parent_session_id = ?1 WHERE id = ?2",
            [parent_id, session_id],
        )?;
        Ok(())
    }
}

// ============================================================================
// Data types
// ============================================================================

/// Session metadata (V2).
#[derive(Debug, Clone, PartialEq)]
pub struct SessionV2 {
    /// Session identifier
    pub id: String,
    /// Optional session title
    pub title: Option<String>,
    /// Optional associated soul identifier
    pub soul_id: Option<String>,
    /// Creation timestamp (milliseconds since epoch)
    pub created_at: i64,
    /// Last update timestamp (milliseconds since epoch)
    pub updated_at: Option<i64>,
    /// Parent session identifier for handoff lineage
    pub parent_session_id: Option<String>,
    /// Current lifecycle state
    pub state: SessionState,
    /// Hash of the configuration active when the session was created
    pub config_hash: Option<String>,
}

/// Session lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionState {
    /// Session is active and accepting events
    Active,
    /// Session has been archived
    Archived,
    /// Session is waiting to be handed off
    HandoffPending,
    /// Session is being compacted
    Compacting,
}

impl SessionState {
    /// Return the string representation stored in the database
    pub fn as_str(&self) -> &'static str {
        match self {
            SessionState::Active => "active",
            SessionState::Archived => "archived",
            SessionState::HandoffPending => "handoff_pending",
            SessionState::Compacting => "compacting",
        }
    }
}

impl std::str::FromStr for SessionState {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "active" => Ok(SessionState::Active),
            "archived" => Ok(SessionState::Archived),
            "handoff_pending" => Ok(SessionState::HandoffPending),
            "compacting" => Ok(SessionState::Compacting),
            _ => Err(format!("unknown session state: {s}")),
        }
    }
}

/// Event type for the append-only event log.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EventType {
    /// A message sent by the user
    UserMessage,
    /// A message sent by the assistant
    AssistantMessage,
    /// A tool call
    ToolCall,
    /// A successful tool result
    ToolResult,
    /// A tool error
    ToolError,
    /// A compaction event
    Compaction,
    /// A configuration change
    ConfigChange,
    /// Session start marker
    SessionStart,
    /// Session end marker
    SessionEnd,
    /// Unknown or unrecognized event type
    Unknown,
}

impl EventType {
    /// Return the string representation stored in the database
    pub fn as_str(&self) -> &'static str {
        match self {
            EventType::UserMessage => "user_message",
            EventType::AssistantMessage => "assistant_message",
            EventType::ToolCall => "tool_call",
            EventType::ToolResult => "tool_result",
            EventType::ToolError => "tool_error",
            EventType::Compaction => "compaction",
            EventType::ConfigChange => "config_change",
            EventType::SessionStart => "session_start",
            EventType::SessionEnd => "session_end",
            EventType::Unknown => "unknown",
        }
    }
}

impl std::str::FromStr for EventType {
    type Err = String;
    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s {
            "user_message" => Ok(EventType::UserMessage),
            "assistant_message" => Ok(EventType::AssistantMessage),
            "tool_call" => Ok(EventType::ToolCall),
            "tool_result" => Ok(EventType::ToolResult),
            "tool_error" => Ok(EventType::ToolError),
            "compaction" => Ok(EventType::Compaction),
            "config_change" => Ok(EventType::ConfigChange),
            "session_start" => Ok(EventType::SessionStart),
            "session_end" => Ok(EventType::SessionEnd),
            _ => Ok(EventType::Unknown),
        }
    }
}

/// A single event record from the event log.
#[derive(Debug, Clone, PartialEq)]
pub struct EventRecord {
    /// Session identifier
    pub session_id: String,
    /// Turn identifier within the session
    pub turn_id: i64,
    /// Event identifier within the turn
    pub event_id: i64,
    /// Timestamp when the event was recorded (milliseconds since epoch)
    pub timestamp: i64,
    /// Type of event
    pub event_type: EventType,
    /// Event payload
    pub payload: serde_json::Value,
    /// Hash of the payload bytes
    pub payload_hash: String,
}

/// A compacted context snapshot.
#[derive(Debug, Clone, PartialEq)]
pub struct CompactedContext {
    /// Session identifier
    pub session_id: String,
    /// Turn identifier up to which the context was compacted
    pub turn_id: i64,
    /// Last event identifier included in the compaction
    pub event_id: i64,
    /// Compacted context as JSON
    pub context_json: serde_json::Value,
    /// Method used to compress the context
    pub compression_method: String,
    /// Hash of the source events used to build the snapshot
    pub source_hash: String,
    /// Timestamp when the snapshot was created (milliseconds since epoch)
    pub created_at: i64,
}
