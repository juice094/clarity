//! Session V2 — unified SQLite-backed session storage.
//!
//! Replaces the JSON+JSONL dual system with a single SQLite schema that
//! supports append-only event logs, compacted contexts, and handoff lineage.
//!
//! # Schema
//!
//! ```sql
//! CREATE TABLE sessions_v2 (
//!     id              TEXT PRIMARY KEY,
//!     title           TEXT,
//!     soul_id         TEXT,
//!     created_at      INTEGER NOT NULL,
//!     updated_at      INTEGER,
//!     parent_session_id TEXT,
//!     state           TEXT CHECK(state IN ('active','archived','handoff_pending','compacting')),
//!     config_hash     TEXT
//! );
//!
//! CREATE TABLE event_log (
//!     id          INTEGER PRIMARY KEY AUTOINCREMENT,
//!     session_id  TEXT NOT NULL,
//!     turn_id     INTEGER NOT NULL,
//!     event_id    INTEGER NOT NULL,
//!     timestamp   INTEGER NOT NULL,
//!     event_type  TEXT NOT NULL,
//!     payload     BLOB NOT NULL,
//!     payload_hash TEXT,
//!     FOREIGN KEY (session_id) REFERENCES sessions_v2(id)
//! );
//!
//! CREATE TABLE compacted_context (
//!     session_id          TEXT PRIMARY KEY,
//!     turn_id             INTEGER NOT NULL,
//!     event_id            INTEGER NOT NULL,
//!     context_json        BLOB NOT NULL,
//!     compression_method  TEXT,
//!     source_hash         TEXT,
//!     created_at          INTEGER,
//!     FOREIGN KEY (session_id) REFERENCES sessions_v2(id)
//! );
//! ```

use std::path::{Path, PathBuf};

use chrono::Utc;
use rusqlite::{Connection, OptionalExtension};

use crate::types::Result as MemoryResult;

// ============================================================================
// SessionStoreV2
// ============================================================================

/// Unified SQLite session store (V2).
pub struct SessionStoreV2 {
    conn: Connection,
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

    // ------------------------------------------------------------------
    // Event log (append-only)
    // ------------------------------------------------------------------

    /// Append an event to the event log.
    pub fn append_event(
        &self,
        session_id: &str,
        turn_id: i64,
        event_type: EventType,
        payload: &serde_json::Value,
    ) -> MemoryResult<()> {
        let payload_bytes = serde_json::to_vec(payload)?;
        let payload_hash = format!("{:016x}", seahash::hash(&payload_bytes));
        let now = Utc::now().timestamp_millis();

        // event_id = max existing + 1 within this session.
        let next_event_id: i64 = self
            .conn
            .query_row(
                "SELECT COALESCE(MAX(event_id), 0) + 1 FROM event_log WHERE session_id = ?1",
                [session_id],
                |row| row.get(0),
            )
            .unwrap_or(1);

        self.conn.execute(
            "INSERT INTO event_log (session_id, turn_id, event_id, timestamp, event_type, payload, payload_hash)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                session_id,
                turn_id,
                next_event_id,
                now,
                event_type.as_str(),
                payload_bytes,
                payload_hash,
            ],
        )?;

        // Update session updated_at.
        self.conn.execute(
            "UPDATE sessions_v2 SET updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, session_id],
        )?;

        Ok(())
    }

    /// Read all events for a session, ordered by (turn_id, event_id).
    pub fn read_events(&self, session_id: &str) -> MemoryResult<Vec<EventRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, turn_id, event_id, timestamp, event_type, payload, payload_hash
             FROM event_log WHERE session_id = ?1
             ORDER BY turn_id ASC, event_id ASC",
        )?;
        let events = stmt
            .query_map([session_id], |row| {
                let payload: Vec<u8> = row.get(5)?;
                let payload_json =
                    serde_json::from_slice(&payload).unwrap_or(serde_json::Value::Null);
                Ok(EventRecord {
                    session_id: row.get(0)?,
                    turn_id: row.get(1)?,
                    event_id: row.get(2)?,
                    timestamp: row.get(3)?,
                    event_type: row
                        .get::<_, String>(4)?
                        .parse()
                        .unwrap_or(EventType::Unknown),
                    payload: payload_json,
                    payload_hash: row.get(6)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        Ok(events)
    }

    /// Read events up to a specific (turn_id, event_id) inclusive.
    pub fn read_events_until(
        &self,
        session_id: &str,
        turn_id: i64,
        event_id: i64,
    ) -> MemoryResult<Vec<EventRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, turn_id, event_id, timestamp, event_type, payload, payload_hash
             FROM event_log WHERE session_id = ?1
               AND (turn_id < ?2 OR (turn_id = ?3 AND event_id <= ?4))
             ORDER BY turn_id ASC, event_id ASC",
        )?;
        let events = stmt
            .query_map(
                rusqlite::params![session_id, turn_id, turn_id, event_id],
                |row| {
                    let payload: Vec<u8> = row.get(5)?;
                    let payload_json =
                        serde_json::from_slice(&payload).unwrap_or(serde_json::Value::Null);
                    Ok(EventRecord {
                        session_id: row.get(0)?,
                        turn_id: row.get(1)?,
                        event_id: row.get(2)?,
                        timestamp: row.get(3)?,
                        event_type: row
                            .get::<_, String>(4)?
                            .parse()
                            .unwrap_or(EventType::Unknown),
                        payload: payload_json,
                        payload_hash: row.get(6)?,
                    })
                },
            )?
            .collect::<Result<_, _>>()?;
        Ok(events)
    }

    // ------------------------------------------------------------------
    // Compacted context
    // ------------------------------------------------------------------

    /// Store a compacted context snapshot.
    pub fn store_compacted_context(
        &self,
        session_id: &str,
        turn_id: i64,
        event_id: i64,
        context_json: &serde_json::Value,
        compression_method: &str,
        source_hash: &str,
    ) -> MemoryResult<()> {
        let context_bytes = serde_json::to_vec(context_json)?;
        let now = Utc::now().timestamp_millis();

        self.conn.execute(
            "INSERT OR REPLACE INTO compacted_context
             (session_id, turn_id, event_id, context_json, compression_method, source_hash, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![
                session_id,
                turn_id,
                event_id,
                context_bytes,
                compression_method,
                source_hash,
                now,
            ],
        )?;
        Ok(())
    }

    /// Load the latest compacted context for a session.
    pub fn load_compacted_context(
        &self,
        session_id: &str,
    ) -> MemoryResult<Option<CompactedContext>> {
        let mut stmt = self.conn.prepare(
            "SELECT session_id, turn_id, event_id, context_json, compression_method, source_hash, created_at
             FROM compacted_context WHERE session_id = ?1"
        )?;
        let row = stmt
            .query_row([session_id], |row| {
                let context_bytes: Vec<u8> = row.get(3)?;
                let context_json =
                    serde_json::from_slice(&context_bytes).unwrap_or(serde_json::Value::Null);
                Ok(CompactedContext {
                    session_id: row.get(0)?,
                    turn_id: row.get(1)?,
                    event_id: row.get(2)?,
                    context_json,
                    compression_method: row.get(4)?,
                    source_hash: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })
            .optional()?;
        Ok(row)
    }

    /// Delete a session and all associated events / compacted contexts (cascade).
    pub fn delete_session(&self, session_id: &str) -> MemoryResult<()> {
        self.conn
            .execute("DELETE FROM sessions_v2 WHERE id = ?1", [session_id])?;
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

// ============================================================================
// Schema
// ============================================================================

const TABLES_SQL: &[&str] = &[
    "CREATE TABLE IF NOT EXISTS sessions_v2 (\
     id TEXT PRIMARY KEY, \
     title TEXT, \
     soul_id TEXT, \
     created_at INTEGER NOT NULL, \
     updated_at INTEGER, \
     parent_session_id TEXT REFERENCES sessions_v2(id), \
     state TEXT CHECK(state IN ('active','archived','handoff_pending','compacting')) NOT NULL DEFAULT 'active', \
     config_hash TEXT)",
    "CREATE TABLE IF NOT EXISTS event_log (\
     id INTEGER PRIMARY KEY AUTOINCREMENT, \
     session_id TEXT NOT NULL REFERENCES sessions_v2(id) ON DELETE CASCADE, \
     turn_id INTEGER NOT NULL, \
     event_id INTEGER NOT NULL, \
     timestamp INTEGER NOT NULL, \
     event_type TEXT NOT NULL, \
     payload BLOB NOT NULL, \
     payload_hash TEXT, \
     UNIQUE(session_id, turn_id, event_id))",
    "CREATE TABLE IF NOT EXISTS compacted_context (\
     session_id TEXT PRIMARY KEY REFERENCES sessions_v2(id) ON DELETE CASCADE, \
     turn_id INTEGER NOT NULL, \
     event_id INTEGER NOT NULL, \
     context_json BLOB NOT NULL, \
     compression_method TEXT, \
     source_hash TEXT, \
     created_at INTEGER)",
];

const INDEXES_SQL: &[&str] = &[
    "CREATE INDEX IF NOT EXISTS idx_sessions_v2_state ON sessions_v2(state)",
    "CREATE INDEX IF NOT EXISTS idx_sessions_v2_soul ON sessions_v2(soul_id)",
    "CREATE INDEX IF NOT EXISTS idx_sessions_v2_parent ON sessions_v2(parent_session_id)",
    "CREATE INDEX IF NOT EXISTS idx_event_log_session ON event_log(session_id)",
    "CREATE INDEX IF NOT EXISTS idx_event_log_session_turn_event ON event_log(session_id, turn_id, event_id)",
    "CREATE INDEX IF NOT EXISTS idx_event_log_timestamp ON event_log(timestamp)",
];

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

// Simple hash for payload integrity (avoids adding heavy crypto deps).
mod seahash {
    pub fn hash(bytes: &[u8]) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut hasher = DefaultHasher::new();
        bytes.hash(&mut hasher);
        hasher.finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> SessionStoreV2 {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        SessionStoreV2::new(tmp.path()).unwrap()
    }

    #[test]
    fn test_create_and_get_session() {
        let store = temp_store();
        store
            .create_session(
                "sess-1",
                Some("Test Session"),
                Some("soul-a"),
                Some("abc123"),
            )
            .unwrap();

        let sess = store.get_session("sess-1").unwrap().unwrap();
        assert_eq!(sess.id, "sess-1");
        assert_eq!(sess.title, Some("Test Session".to_string()));
        assert_eq!(sess.soul_id, Some("soul-a".to_string()));
        assert_eq!(sess.state, SessionState::Active);
    }

    #[test]
    fn test_event_log_append_and_read() {
        let store = temp_store();
        store.create_session("sess-1", None, None, None).unwrap();

        let payload = serde_json::json!({"role": "user", "content": "hello"});
        store
            .append_event("sess-1", 1, EventType::UserMessage, &payload)
            .unwrap();
        store
            .append_event(
                "sess-1",
                1,
                EventType::AssistantMessage,
                &serde_json::json!({"content": "hi"}),
            )
            .unwrap();
        store
            .append_event(
                "sess-1",
                2,
                EventType::UserMessage,
                &serde_json::json!({"content": "world"}),
            )
            .unwrap();

        let events = store.read_events("sess-1").unwrap();
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].event_type, EventType::UserMessage);
        assert_eq!(events[0].event_id, 1);
        assert_eq!(events[1].event_id, 2);
        assert_eq!(events[2].turn_id, 2);
    }

    #[test]
    fn test_read_events_until() {
        let store = temp_store();
        store.create_session("sess-1", None, None, None).unwrap();

        for i in 1..=5 {
            store
                .append_event(
                    "sess-1",
                    i,
                    EventType::UserMessage,
                    &serde_json::json!({"i": i}),
                )
                .unwrap();
        }

        // turn_id 3 has event_id 3 (auto-assigned by append_event).
        let events = store.read_events_until("sess-1", 3, 3).unwrap();
        assert_eq!(events.len(), 3); // turns 1,2,3
    }

    #[test]
    fn test_compacted_context_roundtrip() {
        let store = temp_store();
        store.create_session("sess-1", None, None, None).unwrap();

        let context = serde_json::json!([{"role": "system", "content": "compacted"}]);
        store
            .store_compacted_context("sess-1", 5, 12, &context, "tier2", "hash-abc")
            .unwrap();

        let loaded = store.load_compacted_context("sess-1").unwrap().unwrap();
        assert_eq!(loaded.turn_id, 5);
        assert_eq!(loaded.event_id, 12);
        assert_eq!(loaded.compression_method, "tier2");
        assert_eq!(loaded.context_json, context);
    }

    #[test]
    fn test_session_state_transition() {
        let store = temp_store();
        store.create_session("sess-1", None, None, None).unwrap();

        store
            .set_session_state("sess-1", SessionState::Compacting)
            .unwrap();
        let sess = store.get_session("sess-1").unwrap().unwrap();
        assert_eq!(sess.state, SessionState::Compacting);
    }

    #[test]
    fn test_parent_session_lineage() {
        let store = temp_store();
        store.create_session("parent", None, None, None).unwrap();
        store.create_session("child", None, None, None).unwrap();
        store.set_parent("child", "parent").unwrap();

        let child = store.get_session("child").unwrap().unwrap();
        assert_eq!(child.parent_session_id, Some("parent".to_string()));
    }

    #[test]
    fn test_delete_session_cascades() {
        let store = temp_store();
        store.create_session("sess-1", None, None, None).unwrap();
        store
            .append_event("sess-1", 1, EventType::UserMessage, &serde_json::json!({}))
            .unwrap();
        store
            .store_compacted_context("sess-1", 1, 1, &serde_json::json!([]), "tier1", "h")
            .unwrap();

        store.delete_session("sess-1").unwrap();
        assert!(store.get_session("sess-1").unwrap().is_none());
        assert!(store.read_events("sess-1").unwrap().is_empty());
        assert!(store.load_compacted_context("sess-1").unwrap().is_none());
    }
}
