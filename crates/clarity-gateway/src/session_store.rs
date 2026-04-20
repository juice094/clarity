use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::{Arc, Mutex};
use tracing::{debug, info, instrument};

/// A chat message stored in a session
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    pub created_at: DateTime<Utc>,
}

impl SessionMessage {
    pub fn new(role: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: role.into(),
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
            created_at: Utc::now(),
        }
    }

    pub fn with_tool_calls(mut self, tool_calls: impl Into<String>) -> Self {
        self.tool_calls = Some(tool_calls.into());
        self
    }

    pub fn with_tool_call_id(mut self, tool_call_id: impl Into<String>) -> Self {
        self.tool_call_id = Some(tool_call_id.into());
        self
    }
}

/// Persistent session store using SQLite
#[derive(Debug, Clone)]
pub struct PersistentSessionStore {
    conn: Arc<Mutex<Connection>>,
}

#[derive(Debug, thiserror::Error)]
pub enum SessionStoreError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Session not found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, SessionStoreError>;

impl PersistentSessionStore {
    #[instrument(skip(db_path))]
    pub async fn new(db_path: impl AsRef<Path> + std::fmt::Debug) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let conn = tokio::task::spawn_blocking(move || {
            Connection::open(&db_path).map_err(SessionStoreError::Database)
        })
        .await
        .map_err(|e| {
            SessionStoreError::Io(std::io::Error::other(e.to_string()))
        })??;

        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        info!("PersistentSessionStore initialized");
        Ok(store)
    }

    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(SessionStoreError::Database)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS sessions (
                session_id TEXT PRIMARY KEY,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS session_messages (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                tool_calls TEXT,
                tool_call_id TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(session_id) ON DELETE CASCADE
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_session_messages_session ON session_messages(session_id)",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS stats (
                key TEXT PRIMARY KEY,
                value INTEGER NOT NULL DEFAULT 0
            )",
            [],
        )?;

        conn.execute(
            "INSERT OR IGNORE INTO stats (key, value) VALUES ('total_requests', 0)",
            [],
        )?;

        debug!("Session store schema initialized");
        Ok(())
    }

    /// Create a new session
    pub async fn create_session(&self, session_id: &str) -> Result<()> {
        let sid = session_id.to_string();
        let conn = self.conn.clone();
        let now = Utc::now().to_rfc3339();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO sessions (session_id, created_at, updated_at) VALUES (?1, ?2, ?3)",
                params![sid, now.clone(), now],
            )?;
            Ok::<_, SessionStoreError>(())
        })
        .await
        .map_err(|e| {
            SessionStoreError::Io(std::io::Error::other(e.to_string()))
        })??;

        info!("Session created: {}", session_id);
        Ok(())
    }

    /// Save multiple messages for a session (upsert semantics: delete old, insert new)
    pub async fn save_session(&self, session_id: &str, messages: &[SessionMessage]) -> Result<()> {
        let session_id = session_id.to_string();
        let messages: Vec<_> = messages.to_vec();
        let conn = self.conn.clone();
        let now = Utc::now().to_rfc3339();

        tokio::task::spawn_blocking(move || {
            let mut conn = conn.lock().unwrap();
            let tx = conn.transaction()?;

            // Ensure session exists
            tx.execute(
                "INSERT OR IGNORE INTO sessions (session_id, created_at, updated_at) VALUES (?1, ?2, ?3)",
                params![session_id, now.clone(), now.clone()],
            )?;

            // Delete existing messages for this session
            tx.execute(
                "DELETE FROM session_messages WHERE session_id = ?",
                [&session_id],
            )?;

            // Insert new messages
            for msg in messages {
                tx.execute(
                    "INSERT INTO session_messages (session_id, role, content, tool_calls, tool_call_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        session_id,
                        msg.role,
                        msg.content,
                        msg.tool_calls,
                        msg.tool_call_id,
                        msg.created_at.to_rfc3339()
                    ],
                )?;
            }

            // Update session timestamp
            tx.execute(
                "UPDATE sessions SET updated_at = ?1 WHERE session_id = ?2",
                params![now, session_id],
            )?;

            tx.commit()?;
            Ok::<_, SessionStoreError>(())
        })
        .await
        .map_err(|e| {
            SessionStoreError::Io(std::io::Error::other(e.to_string()))
        })??;

        Ok(())
    }

    /// Append a single message to a session
    pub async fn append_message(&self, session_id: &str, message: &SessionMessage) -> Result<()> {
        let session_id = session_id.to_string();
        let message = message.clone();
        let conn = self.conn.clone();
        let now = Utc::now().to_rfc3339();

        tokio::task::spawn_blocking(move || {
            let mut conn = conn.lock().unwrap();
            let tx = conn.transaction()?;

            // Ensure session exists
            tx.execute(
                "INSERT OR IGNORE INTO sessions (session_id, created_at, updated_at) VALUES (?1, ?2, ?3)",
                params![session_id, now.clone(), now.clone()],
            )?;

            tx.execute(
                "INSERT INTO session_messages (session_id, role, content, tool_calls, tool_call_id, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    session_id,
                    message.role,
                    message.content,
                    message.tool_calls,
                    message.tool_call_id,
                    message.created_at.to_rfc3339()
                ],
            )?;

            tx.execute(
                "UPDATE sessions SET updated_at = ?1 WHERE session_id = ?2",
                params![now, session_id],
            )?;

            tx.commit()?;
            Ok::<_, SessionStoreError>(())
        })
        .await
        .map_err(|e| {
            SessionStoreError::Io(std::io::Error::other(e.to_string()))
        })??;

        Ok(())
    }

    /// Load all messages for a session
    pub async fn load_session(&self, session_id: &str) -> Result<Vec<SessionMessage>> {
        let session_id = session_id.to_string();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT role, content, tool_calls, tool_call_id, created_at 
                 FROM session_messages 
                 WHERE session_id = ? 
                 ORDER BY id ASC"
            )?;
            let rows = stmt.query_map([session_id], |row| {
                let created_at_str: String = row.get(4)?;
                let created_at = DateTime::parse_from_rfc3339(&created_at_str)
                    .map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            4,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?
                    .with_timezone(&Utc);
                Ok(SessionMessage {
                    role: row.get(0)?,
                    content: row.get(1)?,
                    tool_calls: row.get(2)?,
                    tool_call_id: row.get(3)?,
                    created_at,
                })
            })?;

            let mut messages = Vec::new();
            for row in rows {
                messages.push(row?);
            }
            Ok::<_, SessionStoreError>(messages)
        })
        .await
        .map_err(|e| {
            SessionStoreError::Io(std::io::Error::other(e.to_string()))
        })?
    }

    /// List all session IDs ordered by most recently updated
    pub async fn list_sessions(&self) -> Result<Vec<String>> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt =
                conn.prepare("SELECT session_id FROM sessions ORDER BY updated_at DESC")?;
            let rows = stmt.query_map([], |row| {
                let id: String = row.get(0)?;
                Ok(id)
            })?;

            let mut sessions = Vec::new();
            for row in rows {
                sessions.push(row?);
            }
            Ok::<_, SessionStoreError>(sessions)
        })
        .await
        .map_err(|e| {
            SessionStoreError::Io(std::io::Error::other(e.to_string()))
        })?
    }

    /// Delete a session and all its messages. Returns true if the session existed.
    pub async fn delete_session(&self, session_id: &str) -> Result<bool> {
        let session_id = session_id.to_string();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let rows = conn.execute(
                "DELETE FROM sessions WHERE session_id = ?",
                [session_id],
            )?;
            Ok::<_, SessionStoreError>(rows > 0)
        })
        .await
        .map_err(|e| {
            SessionStoreError::Io(std::io::Error::other(e.to_string()))
        })?
    }

    /// Check if a session exists
    pub async fn session_exists(&self, session_id: &str) -> Result<bool> {
        let session_id = session_id.to_string();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let count: i64 = conn.query_row(
                "SELECT COUNT(*) FROM sessions WHERE session_id = ?",
                [session_id],
                |row| row.get(0),
            )?;
            Ok::<_, SessionStoreError>(count > 0)
        })
        .await
        .map_err(|e| {
            SessionStoreError::Io(std::io::Error::other(e.to_string()))
        })?
    }

    /// Count total sessions
    pub async fn session_count(&self) -> Result<usize> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let count: i64 = conn.query_row("SELECT COUNT(*) FROM sessions", [], |row| {
                row.get(0)
            })?;
            Ok::<_, SessionStoreError>(count as usize)
        })
        .await
        .map_err(|e| {
            SessionStoreError::Io(std::io::Error::other(e.to_string()))
        })?
    }

    /// Record a request in stats
    pub async fn record_request(&self) -> Result<()> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "UPDATE stats SET value = value + 1 WHERE key = 'total_requests'",
                [],
            )?;
            Ok::<_, SessionStoreError>(())
        })
        .await
        .map_err(|e| {
            SessionStoreError::Io(std::io::Error::other(e.to_string()))
        })??;

        Ok(())
    }

    /// Get total request count
    pub async fn total_requests(&self) -> Result<u64> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let count: i64 = conn.query_row(
                "SELECT value FROM stats WHERE key = 'total_requests'",
                [],
                |row| row.get(0),
            )?;
            Ok::<_, SessionStoreError>(count as u64)
        })
        .await
        .map_err(|e| {
            SessionStoreError::Io(std::io::Error::other(e.to_string()))
        })?
    }

    /// Delete sessions that have been idle longer than max_idle_minutes
    pub async fn cleanup_expired(&self, max_idle_minutes: i64) -> Result<usize> {
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let cutoff =
                (Utc::now() - chrono::Duration::minutes(max_idle_minutes)).to_rfc3339();
            let rows = conn.execute(
                "DELETE FROM sessions WHERE updated_at < ?",
                [cutoff],
            )?;
            if rows > 0 {
                info!("Cleaned up {} expired sessions", rows);
            }
            Ok::<_, SessionStoreError>(rows)
        })
        .await
        .map_err(|e| {
            SessionStoreError::Io(std::io::Error::other(e.to_string()))
        })?
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_store() -> PersistentSessionStore {
        PersistentSessionStore::new_in_memory().unwrap()
    }

    #[tokio::test]
    async fn test_create_and_list_sessions() {
        let store = create_test_store();

        store.create_session("session-a").await.unwrap();
        store.create_session("session-b").await.unwrap();

        let sessions = store.list_sessions().await.unwrap();
        assert_eq!(sessions.len(), 2);
        assert!(sessions.contains(&"session-a".to_string()));
        assert!(sessions.contains(&"session-b".to_string()));
    }

    #[tokio::test]
    async fn test_save_and_load_messages() {
        let store = create_test_store();

        let messages = vec![
            SessionMessage::new("user", "Hello"),
            SessionMessage::new("assistant", "Hi there!"),
            SessionMessage::new("user", "How are you?"),
        ];

        store.save_session("session-1", &messages).await.unwrap();
        let loaded = store.load_session("session-1").await.unwrap();

        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].role, "user");
        assert_eq!(loaded[0].content, "Hello");
        assert_eq!(loaded[1].role, "assistant");
        assert_eq!(loaded[1].content, "Hi there!");
        assert_eq!(loaded[2].role, "user");
        assert_eq!(loaded[2].content, "How are you?");
    }

    #[tokio::test]
    async fn test_append_message() {
        let store = create_test_store();

        store
            .append_message("session-1", &SessionMessage::new("user", "First"))
            .await
            .unwrap();
        store
            .append_message("session-1", &SessionMessage::new("assistant", "Second"))
            .await
            .unwrap();

        let loaded = store.load_session("session-1").await.unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded[0].content, "First");
        assert_eq!(loaded[1].content, "Second");
    }

    #[tokio::test]
    async fn test_save_session_overwrites() {
        let store = create_test_store();

        store
            .append_message("session-1", &SessionMessage::new("user", "Old"))
            .await
            .unwrap();

        let new_messages = vec![SessionMessage::new("user", "New")];
        store.save_session("session-1", &new_messages).await.unwrap();

        let loaded = store.load_session("session-1").await.unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].content, "New");
    }

    #[tokio::test]
    async fn test_delete_session() {
        let store = create_test_store();

        store.create_session("to-delete").await.unwrap();
        store
            .append_message("to-delete", &SessionMessage::new("user", "bye"))
            .await
            .unwrap();

        assert!(store.session_exists("to-delete").await.unwrap());
        assert!(store.delete_session("to-delete").await.unwrap());
        assert!(!store.session_exists("to-delete").await.unwrap());

        // Deleting non-existent returns false
        assert!(!store.delete_session("missing").await.unwrap());
    }

    #[tokio::test]
    async fn test_empty_session_messages() {
        let store = create_test_store();

        store.create_session("empty").await.unwrap();
        let loaded = store.load_session("empty").await.unwrap();
        assert!(loaded.is_empty());
    }

    #[tokio::test]
    async fn test_load_nonexistent_session() {
        let store = create_test_store();

        let loaded = store.load_session("no-such-session").await.unwrap();
        assert!(loaded.is_empty());
    }

    #[tokio::test]
    async fn test_session_count() {
        let store = create_test_store();

        assert_eq!(store.session_count().await.unwrap(), 0);

        store.create_session("s1").await.unwrap();
        assert_eq!(store.session_count().await.unwrap(), 1);

        store.create_session("s2").await.unwrap();
        assert_eq!(store.session_count().await.unwrap(), 2);

        store.delete_session("s1").await.unwrap();
        assert_eq!(store.session_count().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_record_and_total_requests() {
        let store = create_test_store();

        assert_eq!(store.total_requests().await.unwrap(), 0);

        store.record_request().await.unwrap();
        store.record_request().await.unwrap();
        assert_eq!(store.total_requests().await.unwrap(), 2);
    }

    #[tokio::test]
    async fn test_tool_fields() {
        let store = create_test_store();

        let msg = SessionMessage::new("assistant", "Using tool")
            .with_tool_calls(r#"[{"name":"test"}]"#.to_string())
            .with_tool_call_id("call_123".to_string());

        store.append_message("session-t", &msg).await.unwrap();
        let loaded = store.load_session("session-t").await.unwrap();

        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].tool_calls, Some(r#"[{"name":"test"}]"#.to_string()));
        assert_eq!(loaded[0].tool_call_id, Some("call_123".to_string()));
    }

    #[tokio::test]
    async fn test_cleanup_expired() {
        let store = create_test_store();

        store.create_session("old").await.unwrap();
        store.create_session("recent").await.unwrap();

        // Manually set old session's updated_at to a time in the past
        {
            let conn = store.conn.lock().unwrap();
            let old_time = (Utc::now() - chrono::Duration::minutes(100)).to_rfc3339();
            conn.execute(
                "UPDATE sessions SET updated_at = ?1 WHERE session_id = 'old'",
                [old_time],
            )
            .unwrap();
        }

        let cleaned = store.cleanup_expired(60).await.unwrap();
        assert_eq!(cleaned, 1);

        assert!(!store.session_exists("old").await.unwrap());
        assert!(store.session_exists("recent").await.unwrap());
    }
}
