//! Migrate legacy V1 sessions (`sessions.db`) to V2 threads and rollouts.
//!
//! # Usage
//!
//! ```no_run
//! use clarity_core::session::thread_migration::ThreadMigrator;
//! use clarity_thread_store::RolloutConfig;
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let v1_db = PathBuf::from(".clarity/sessions.db");
//!     let config = RolloutConfig {
//!         clarity_home: PathBuf::from(".clarity"),
//!         sqlite_home: PathBuf::from(".clarity"),
//!         cwd: PathBuf::from("."),
//!         model_provider_id: "default".to_string(),
//!         generate_memories: false,
//!     };
//!
//!     let migrator = ThreadMigrator::new(&v1_db, config)?;
//!     let stats = migrator.dry_run()?;
//!     println!("{:?}", stats);
//!
//!     let report = migrator.migrate().await?;
//!     println!("{:?}", report);
//!     Ok(())
//! }
//! ```

use std::path::{Path, PathBuf};

use clarity_contract::{
    RolloutItem, RolloutResponseItem, SessionId, SessionSource, ThreadId, ThreadSource,
};
use clarity_thread_store::{
    AppendThreadItemsParams, CreateThreadParams, LocalThreadStore, RolloutConfig, ThreadStore,
    UpdateThreadMetadataParams,
};
use rusqlite::Connection;

// ============================================================================
// Errors
// ============================================================================

/// Errors that can occur during thread migration.
#[derive(Debug, thiserror::Error)]
pub enum ThreadMigrationError {
    /// Source V1 database error.
    #[error("V1 database error: {0}")]
    V1Db(#[from] rusqlite::Error),
    /// Destination V2 thread store error.
    #[error("V2 thread store error: {0}")]
    V2Store(#[from] clarity_thread_store::ThreadStoreError),
    /// Invalid V1 session identifier.
    #[error("invalid session id '{session_id}': {source}")]
    InvalidSessionId {
        /// The offending session id.
        session_id: String,
        /// Underlying UUID parse error.
        #[source]
        source: uuid::Error,
    },
}

// ============================================================================
// Types
// ============================================================================

/// Statistics from a dry-run analysis.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct ThreadMigrationStats {
    /// Number of V1 sessions.
    pub sessions: usize,
    /// Number of V1 messages across all sessions.
    pub messages: usize,
}

/// Report after migration execution.
#[derive(Debug, Clone, PartialEq)]
pub struct ThreadMigrationReport {
    /// Number of sessions migrated to threads.
    pub sessions_migrated: usize,
    /// Number of messages migrated to rollout items.
    pub messages_migrated: usize,
    /// Errors indexed by V1 session id.
    pub errors: Vec<String>,
}

/// A raw V1 message row.
#[derive(Debug, Clone)]
struct V1Message {
    role: String,
    content: String,
    tool_calls: Option<String>,
    tool_call_id: Option<String>,
    created_at: String,
}

/// A raw V1 session row.
#[derive(Debug, Clone)]
struct V1Session {
    session_id: String,
    _created_at: String,
}

// ============================================================================
// ThreadMigrator
// ============================================================================

/// Migrates legacy V1 sessions into V2 threads backed by JSONL rollouts.
pub struct ThreadMigrator {
    v1_path: PathBuf,
    config: RolloutConfig,
}

impl ThreadMigrator {
    /// Create a new migrator.
    ///
    /// `v1_db` — path to the V1 `sessions.db`.
    /// `config` — rollout configuration describing the destination Clarity home.
    pub fn new(
        v1_db: impl AsRef<Path>,
        config: RolloutConfig,
    ) -> Result<Self, ThreadMigrationError> {
        let v1_path = v1_db.as_ref().to_path_buf();
        let conn = Connection::open(&v1_path)?;
        conn.query_row("SELECT COUNT(*) FROM sessions", [], |_| Ok(()))?;
        drop(conn);

        Ok(Self { v1_path, config })
    }

    // ------------------------------------------------------------------
    // Dry run
    // ------------------------------------------------------------------

    /// Analyze the V1 database and return migration statistics.
    pub fn dry_run(&self) -> Result<ThreadMigrationStats, ThreadMigrationError> {
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
        Ok(ThreadMigrationStats { sessions, messages })
    }

    // ------------------------------------------------------------------
    // Migrate
    // ------------------------------------------------------------------

    /// Execute the migration: read V1, write V2 threads and rollouts.
    ///
    /// No data is deleted from V1 — the V1 database is left untouched.
    pub async fn migrate(&self) -> Result<ThreadMigrationReport, ThreadMigrationError> {
        let v1_conn = Connection::open(&self.v1_path)?;
        let state_db = LocalThreadStore::default_state_db_path(&self.config);
        let store = LocalThreadStore::new(self.config.clone(), state_db)?;

        let mut report = ThreadMigrationReport {
            sessions_migrated: 0,
            messages_migrated: 0,
            errors: Vec::new(),
        };

        let sessions = self.read_sessions(&v1_conn)?;
        for session in &sessions {
            match self.migrate_session(&v1_conn, &store, session).await {
                Ok(message_count) => {
                    report.sessions_migrated += 1;
                    report.messages_migrated += message_count;
                }
                Err(e) => {
                    report
                        .errors
                        .push(format!("session {}: {e}", session.session_id));
                }
            }
        }

        drop(v1_conn);
        Ok(report)
    }

    fn read_sessions(&self, conn: &Connection) -> Result<Vec<V1Session>, rusqlite::Error> {
        let mut stmt =
            conn.prepare("SELECT session_id, created_at FROM sessions ORDER BY created_at ASC")?;
        let rows: Vec<V1Session> = stmt
            .query_map([], |row| {
                Ok(V1Session {
                    session_id: row.get(0)?,
                    _created_at: row.get(1)?,
                })
            })?
            .collect::<Result<_, _>>()?;
        drop(stmt);
        Ok(rows)
    }

    async fn migrate_session(
        &self,
        v1_conn: &Connection,
        store: &LocalThreadStore,
        session: &V1Session,
    ) -> Result<usize, ThreadMigrationError> {
        let thread_id = ThreadId::from_string(&session.session_id).map_err(|e| {
            ThreadMigrationError::InvalidSessionId {
                session_id: session.session_id.clone(),
                source: e,
            }
        })?;
        let session_id = SessionId::from(thread_id);

        store
            .create_thread(CreateThreadParams {
                thread_id,
                session_id,
                forked_from_id: None,
                parent_thread_id: None,
                source: SessionSource::Unknown,
                thread_source: Some(ThreadSource::Resumed),
                cwd: self.config.cwd.clone(),
                originator: "session-migration".to_string(),
                cli_version: env!("CARGO_PKG_VERSION").to_string(),
                base_instructions: None,
                dynamic_tools: Vec::new(),
                model_provider: Some(self.config.model_provider_id.clone()),
                generate_memories: self.config.generate_memories,
                multi_agent_version: None,
            })
            .await?;

        let messages = self.read_messages(v1_conn, &session.session_id)?;
        let items: Vec<RolloutItem> = messages.iter().map(message_to_item).collect();
        let message_count = items.len();

        if !items.is_empty() {
            store
                .append_items(AppendThreadItemsParams { thread_id, items })
                .await?;
        }

        if let Some(title) = infer_title(&messages) {
            store
                .update_thread_metadata(UpdateThreadMetadataParams {
                    thread_id,
                    patch: clarity_thread_store::ThreadMetadataPatch {
                        title: Some(title),
                        archived: None,
                        extra: std::collections::HashMap::new(),
                    },
                })
                .await?;
        }

        Ok(message_count)
    }

    fn read_messages(
        &self,
        conn: &Connection,
        session_id: &str,
    ) -> Result<Vec<V1Message>, rusqlite::Error> {
        let mut stmt = conn.prepare(
            "SELECT role, content, tool_calls, tool_call_id, created_at \
             FROM session_messages WHERE session_id = ?1 ORDER BY id ASC",
        )?;
        let rows: Vec<V1Message> = stmt
            .query_map([session_id], |row| {
                Ok(V1Message {
                    role: row.get(0)?,
                    content: row.get(1)?,
                    tool_calls: row.get(2)?,
                    tool_call_id: row.get(3)?,
                    created_at: row.get::<_, String>(4).unwrap_or_default(),
                })
            })?
            .collect::<Result<_, _>>()?;
        drop(stmt);
        Ok(rows)
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Convert a V1 message into a durable rollout item.
fn message_to_item(msg: &V1Message) -> RolloutItem {
    let payload = serde_json::json!({
        "role": msg.role,
        "content": msg.content,
        "tool_calls": msg.tool_calls,
        "tool_call_id": msg.tool_call_id,
        "created_at": msg.created_at,
    });
    RolloutItem::ResponseItem(RolloutResponseItem::Other(payload))
}

/// Derive a thread title from the first user message.
fn infer_title(messages: &[V1Message]) -> Option<String> {
    messages.iter().find(|m| m.role == "user").map(|m| {
        let trimmed = m.content.trim();
        if trimmed.chars().count() > 40 {
            format!("{}...", trimmed.chars().take(40).collect::<String>())
        } else {
            trimmed.to_string()
        }
    })
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
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE session_messages(id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE, role TEXT NOT NULL, content TEXT NOT NULL, tool_calls TEXT, tool_call_id TEXT, created_at TEXT NOT NULL)",
            [],
        )
        .unwrap();

        let sid = uuid::Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO sessions VALUES(?1,'2026-01-01T00:00:00Z','2026-01-01T01:00:00Z')",
            [&sid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO session_messages VALUES(1,?1,'user','hello world',NULL,NULL,'2026-01-01T00:00:01Z')",
            [&sid],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO session_messages VALUES(2,?1,'assistant','hi there',NULL,NULL,'2026-01-01T00:00:02Z')",
            [&sid],
        )
        .unwrap();
    }

    fn test_config(clarity_home: &Path) -> RolloutConfig {
        RolloutConfig {
            clarity_home: clarity_home.to_path_buf(),
            sqlite_home: clarity_home.to_path_buf(),
            cwd: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            model_provider_id: "default".to_string(),
            generate_memories: false,
        }
    }

    #[tokio::test]
    async fn test_dry_run() {
        let v1 = tempfile::NamedTempFile::new().unwrap();
        create_v1_db(v1.path());

        let tmp = tempfile::tempdir().unwrap();
        let migrator = ThreadMigrator::new(v1.path(), test_config(tmp.path())).unwrap();

        let stats = migrator.dry_run().unwrap();
        assert_eq!(stats.sessions, 1);
        assert_eq!(stats.messages, 2);
    }

    #[tokio::test]
    async fn test_migrate_session_to_thread() {
        let v1 = tempfile::NamedTempFile::new().unwrap();
        create_v1_db(v1.path());

        let tmp = tempfile::tempdir().unwrap();
        let migrator = ThreadMigrator::new(v1.path(), test_config(tmp.path())).unwrap();

        let report = migrator.migrate().await.unwrap();
        assert_eq!(report.sessions_migrated, 1);
        assert_eq!(report.messages_migrated, 2);
        assert!(report.errors.is_empty());

        let state_db = LocalThreadStore::default_state_db_path(&test_config(tmp.path()));
        let store = LocalThreadStore::new(test_config(tmp.path()), state_db).unwrap();
        let page = store
            .list_threads(clarity_thread_store::ListThreadsParams {
                limit: 10,
                cursor: None,
                include_archived: false,
            })
            .await
            .unwrap();
        assert_eq!(page.data.len(), 1);
        assert!(
            page.data[0]
                .title
                .as_ref()
                .unwrap()
                .starts_with("hello world")
        );
    }

    #[tokio::test]
    async fn test_migrate_empty_v1() {
        let v1 = tempfile::NamedTempFile::new().unwrap();
        let conn = Connection::open(v1.path()).unwrap();
        conn.execute(
            "CREATE TABLE sessions(session_id TEXT PRIMARY KEY, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE session_messages(id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE, role TEXT NOT NULL, content TEXT NOT NULL, tool_calls TEXT, tool_call_id TEXT, created_at TEXT NOT NULL)",
            [],
        )
        .unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let migrator = ThreadMigrator::new(v1.path(), test_config(tmp.path())).unwrap();

        let stats = migrator.dry_run().unwrap();
        assert_eq!(stats.sessions, 0);
        assert_eq!(stats.messages, 0);

        let report = migrator.migrate().await.unwrap();
        assert_eq!(report.sessions_migrated, 0);
        assert!(report.errors.is_empty());
    }

    #[tokio::test]
    async fn test_migrate_skips_invalid_session_id() {
        let v1 = tempfile::NamedTempFile::new().unwrap();
        let conn = Connection::open(v1.path()).unwrap();
        conn.execute(
            "CREATE TABLE sessions(session_id TEXT PRIMARY KEY, created_at TEXT NOT NULL, updated_at TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "CREATE TABLE session_messages(id INTEGER PRIMARY KEY AUTOINCREMENT, session_id TEXT NOT NULL REFERENCES sessions(session_id) ON DELETE CASCADE, role TEXT NOT NULL, content TEXT NOT NULL, tool_calls TEXT, tool_call_id TEXT, created_at TEXT NOT NULL)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO sessions VALUES('not-a-uuid','2026-01-01T00:00:00Z','2026-01-01T01:00:00Z')",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO session_messages VALUES(1,'not-a-uuid','user','hello',NULL,NULL,'2026-01-01T00:00:01Z')",
            [],
        )
        .unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let migrator = ThreadMigrator::new(v1.path(), test_config(tmp.path())).unwrap();

        let report = migrator.migrate().await.unwrap();
        assert_eq!(report.sessions_migrated, 0);
        assert_eq!(report.messages_migrated, 0);
        assert_eq!(report.errors.len(), 1);
        assert!(report.errors[0].contains("not-a-uuid"));
    }

    #[test]
    fn test_migrate_rejects_missing_v1_table() {
        let v1 = tempfile::NamedTempFile::new().unwrap();
        let conn = Connection::open(v1.path()).unwrap();
        conn.execute("CREATE TABLE other(id INTEGER PRIMARY KEY)", [])
            .unwrap();

        let tmp = tempfile::tempdir().unwrap();
        let result = ThreadMigrator::new(v1.path(), test_config(tmp.path()));
        assert!(result.is_err());
    }
}
