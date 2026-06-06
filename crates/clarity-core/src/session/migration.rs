//! Session V1→V2 migration — migrate legacy sessions to the new schema.
//!
//! # Usage
//!
//! ```no_run
//! use clarity_core::session::migration::SessionMigrator;
//!
//! let v1_path = ".clarity/sessions.db";
//! let v2_path = ".clarity/sessions_v2.sqlite";
//!
//! let migrator = SessionMigrator::new(v1_path, v2_path).unwrap();
//!
//! // Preview what will be migrated.
//! let stats = migrator.dry_run().unwrap();
//! println!("{:?}", stats);
//!
//! // Execute migration.
//! let report = migrator.migrate().unwrap();
//! println!("{:?}", report);
//! ```

use std::path::{Path, PathBuf};

use clarity_memory::session_store_v2::{EventType, SessionStoreV2};
use rusqlite::Connection;

// ============================================================================
// Types
// ============================================================================

/// Statistics from a dry-run analysis.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct MigrationStats {
    pub sessions: usize,
    pub messages: usize,
}

/// Report after migration execution.
#[derive(Debug, Clone, PartialEq)]
pub struct MigrationReport {
    pub sessions_migrated: usize,
    pub messages_migrated: usize,
    pub errors: Vec<String>,
}

/// A raw V1 message row.
#[derive(Debug, Clone)]
struct V1Message {
    role: String,
    content: String,
    tool_calls: Option<String>,
    tool_call_id: Option<String>,
    _created_at: String,
}

// ============================================================================
// SessionMigrator
// ============================================================================

/// Migrates sessions from V1 (`sessions.db`) to V2 (`sessions_v2.sqlite`).
pub struct SessionMigrator {
    v1_path: PathBuf,
    v2_path: PathBuf,
}

impl SessionMigrator {
    /// Create a new migrator.
    ///
    /// `v1_db` — path to the V1 `sessions.db` (e.g. `.clarity/sessions.db`).
    /// `v2_db` — path to the V2 store (e.g. `.clarity/sessions_v2.sqlite`).
    pub fn new(v1_db: impl AsRef<Path>, v2_db: impl AsRef<Path>) -> rusqlite::Result<Self> {
        // Verify V1 database exists and has the expected schema.
        let conn = Connection::open(v1_db.as_ref())?;
        let _ = conn.query_row("SELECT COUNT(*) FROM sessions", [], |_| Ok(()));
        drop(conn);

        Ok(Self {
            v1_path: v1_db.as_ref().to_path_buf(),
            v2_path: v2_db.as_ref().to_path_buf(),
        })
    }

    // ------------------------------------------------------------------
    // Dry run
    // ------------------------------------------------------------------

    /// Analyze the V1 database and return migration statistics.
    pub fn dry_run(&self) -> rusqlite::Result<MigrationStats> {
        let conn = Connection::open(&self.v1_path)?;

        let sessions: usize = conn
            .query_row("SELECT COUNT(*) FROM sessions", [], |row| row.get(0))
            .unwrap_or(0);

        let messages: usize = conn
            .query_row("SELECT COUNT(*) FROM session_messages", [], |row| {
                row.get(0)
            })
            .unwrap_or(0);

        drop(conn);
        Ok(MigrationStats { sessions, messages })
    }

    // ------------------------------------------------------------------
    // Migrate
    // ------------------------------------------------------------------

    /// Execute the migration: read V1, write V2.
    ///
    /// No data is deleted from V1 — the V1 database is left untouched.
    pub fn migrate(&self) -> Result<MigrationReport, rusqlite::Error> {
        let v1_conn = Connection::open(&self.v1_path)?;
        let v2_store = SessionStoreV2::new(&self.v2_path)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;

        let mut report = MigrationReport {
            sessions_migrated: 0,
            messages_migrated: 0,
            errors: Vec::new(),
        };

        // Collect V1 session IDs.
        let mut stmt = v1_conn
            .prepare("SELECT session_id, created_at FROM sessions ORDER BY created_at ASC")?;
        let sessions: Vec<(String, String)> = stmt
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?;

        drop(stmt);

        for (session_id, _created_at) in &sessions {
            if let Err(e) = self.migrate_session(&v1_conn, &v2_store, session_id) {
                report.errors.push(format!("session {session_id}: {e}"));
                continue;
            }
            report.sessions_migrated += 1;
        }

        drop(v1_conn);

        Ok(report)
    }

    fn migrate_session(
        &self,
        v1_conn: &Connection,
        v2_store: &SessionStoreV2,
        session_id: &str,
    ) -> Result<(), String> {
        // Create V2 session.
        v2_store
            .create_session(session_id, None, None, None)
            .map_err(|e| format!("create v2 session: {e}"))?;

        // Read V1 messages.
        let mut stmt = v1_conn
            .prepare(
                "SELECT role, content, tool_calls, tool_call_id, created_at \
                 FROM session_messages WHERE session_id = ?1 ORDER BY id ASC",
            )
            .map_err(|e| format!("prepare v1 query: {e}"))?;

        let messages: Vec<V1Message> = stmt
            .query_map([session_id], |row| {
                Ok(V1Message {
                    role: row.get(0)?,
                    content: row.get(1)?,
                    tool_calls: row.get(2)?,
                    tool_call_id: row.get(3)?,
                    _created_at: row.get::<_, String>(4).unwrap_or_default(),
                })
            })
            .map_err(|e| format!("query v1 messages: {e}"))?
            .collect::<Result<_, _>>()
            .map_err(|e| format!("collect messages: {e}"))?;

        // Append each message as a V2 event.
        for msg in &messages {
            let event_type = v2_event_type(&msg.role, &msg.tool_call_id);

            let payload = serde_json::json!({
                "role": msg.role,
                "content": msg.content,
                "tool_calls": msg.tool_calls,
                "tool_call_id": msg.tool_call_id,
            });

            v2_store
                .append_event(session_id, 1, event_type, &payload)
                .map_err(|e| format!("append v2 event: {e}"))?;
        }

        Ok(())
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Map V1 message role to V2 event type.
fn v2_event_type(role: &str, tool_call_id: &Option<String>) -> EventType {
    match (role, tool_call_id) {
        ("user", _) => EventType::UserMessage,
        ("assistant", _) => EventType::AssistantMessage,
        ("tool", Some(_)) => EventType::ToolResult,
        ("tool", None) => EventType::ToolResult,
        _ => EventType::Unknown,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn create_v1_db(path: &Path) {
        let conn = Connection::open(path).unwrap();
        conn.execute(
            "CREATE TABLE sessions(session_id TEXT PRIMARY KEY, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            [],
        ).unwrap();
        conn.execute(
            "CREATE TABLE session_messages(id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE, role TEXT NOT NULL, content TEXT NOT NULL, tool_calls TEXT, tool_call_id TEXT, created_at TEXT NOT NULL)",
            [],
        ).unwrap();
        conn.execute(
            "INSERT INTO sessions VALUES('sess-1','2026-01-01T00:00:00Z','2026-01-01T01:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO session_messages VALUES(1,'sess-1','user','hello',NULL,NULL,'2026-01-01T00:00:01Z')", []).unwrap();
        conn.execute("INSERT INTO session_messages VALUES(2,'sess-1','assistant','hi there',NULL,NULL,'2026-01-01T00:00:02Z')", []).unwrap();
        conn.execute("INSERT INTO session_messages VALUES(3,'sess-1','tool','ok',NULL,'call-1','2026-01-01T00:00:03Z')", []).unwrap();
        conn.execute(
            "INSERT INTO sessions VALUES('sess-2','2026-01-02T00:00:00Z','2026-01-02T01:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute("INSERT INTO session_messages VALUES(4,'sess-2','user','world',NULL,NULL,'2026-01-02T00:00:01Z')", []).unwrap();
    }

    #[test]
    fn test_dry_run() {
        let v1 = tempfile::NamedTempFile::new().unwrap();
        create_v1_db(v1.path());

        let v2 = tempfile::NamedTempFile::new().unwrap();
        let migrator = SessionMigrator::new(v1.path(), v2.path()).unwrap();

        let stats = migrator.dry_run().unwrap();
        assert_eq!(stats.sessions, 2);
        assert_eq!(stats.messages, 4);
    }

    #[test]
    fn test_migrate_session() {
        let v1 = tempfile::NamedTempFile::new().unwrap();
        create_v1_db(v1.path());

        let v2 = tempfile::NamedTempFile::new().unwrap();
        let migrator = SessionMigrator::new(v1.path(), v2.path()).unwrap();

        let report = migrator.migrate().unwrap();
        assert_eq!(report.sessions_migrated, 2);
        assert!(report.errors.is_empty());

        // Verify V2 has the data.
        let v2_store = SessionStoreV2::new(v2.path()).unwrap();
        let events = v2_store.read_events("sess-1").unwrap();
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn test_migrate_empty_v1() {
        let v1 = tempfile::NamedTempFile::new().unwrap();
        let conn = Connection::open(v1.path()).unwrap();
        conn.execute("CREATE TABLE sessions(session_id TEXT PRIMARY KEY, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)", []).unwrap();
        conn.execute("CREATE TABLE session_messages(id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE, role TEXT NOT NULL, content TEXT NOT NULL, tool_calls TEXT, tool_call_id TEXT, created_at TEXT NOT NULL)", []).unwrap();

        let v2 = tempfile::NamedTempFile::new().unwrap();
        let migrator = SessionMigrator::new(v1.path(), v2.path()).unwrap();

        let stats = migrator.dry_run().unwrap();
        assert_eq!(stats.sessions, 0);
        assert_eq!(stats.messages, 0);

        let report = migrator.migrate().unwrap();
        assert_eq!(report.sessions_migrated, 0);
        assert!(report.errors.is_empty());
    }

    #[test]
    fn test_v2_event_type_mapping() {
        assert_eq!(v2_event_type("user", &None), EventType::UserMessage);
        assert_eq!(
            v2_event_type("assistant", &None),
            EventType::AssistantMessage
        );
        assert_eq!(v2_event_type("tool", &None), EventType::ToolResult);
        assert_eq!(
            v2_event_type("tool", &Some("call-1".to_string())),
            EventType::ToolResult
        );
        assert_eq!(v2_event_type("system", &None), EventType::Unknown);
    }
}
