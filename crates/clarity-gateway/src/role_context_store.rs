//! Persistent SQLite-backed store for Claw Mesh role-context events.
//!
//! Replaces the in-memory `HashMap<String, Vec<ClawContextEvent>>` placeholder
//! in `AppState` so Gateway role contexts survive restarts and can serve as an
//! online fallback for offline devices.

use chrono::Utc;
use clarity_contract::ClawContextEvent;
use parking_lot::Mutex;
use rusqlite::{Connection, params};
use std::path::Path;
use std::sync::Arc;
use tracing::{debug, info, instrument};

/// Persistent store for role-context events and device presence.
#[derive(Debug, Clone)]
pub struct RoleContextStore {
    conn: Arc<Mutex<Connection>>,
}

/// Errors that can occur when interacting with the role-context store.
#[derive(Debug, thiserror::Error)]
pub enum RoleContextStoreError {
    /// Database (SQLite) error.
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error.
    #[error("Serialization error: {0}")]
    Serialization(String),
}

/// Shorthand result type for role-context store operations.
pub type Result<T> = std::result::Result<T, RoleContextStoreError>;

/// Device is considered offline if its last heartbeat is older than this.
const PRESENCE_TTL_SECONDS: i64 = 5 * 60;

impl RoleContextStore {
    /// Open a persistent SQLite-backed role-context store.
    #[instrument(skip(db_path))]
    pub async fn new(db_path: impl AsRef<Path> + std::fmt::Debug) -> Result<Self> {
        let db_path = db_path.as_ref().to_path_buf();
        let conn = tokio::task::spawn_blocking(move || -> Result<Connection> {
            let conn = Connection::open(&db_path).map_err(RoleContextStoreError::Database)?;
            conn.pragma_update(None, "journal_mode", "WAL")
                .map_err(RoleContextStoreError::Database)?;
            Ok(conn)
        })
        .await
        .map_err(|e| RoleContextStoreError::Io(std::io::Error::other(e.to_string())))??;

        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        info!("RoleContextStore initialized");
        Ok(store)
    }

    /// Create an in-memory role-context store, useful for tests and fallbacks.
    pub fn new_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory().map_err(RoleContextStoreError::Database)?;
        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "CREATE TABLE IF NOT EXISTS role_contexts (
                role_id TEXT PRIMARY KEY,
                updated_at TEXT NOT NULL
            )",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS role_context_events (
                role_id TEXT NOT NULL,
                event_id TEXT NOT NULL,
                origin_clock INTEGER NOT NULL,
                event_json TEXT NOT NULL,
                recorded_at TEXT NOT NULL,
                PRIMARY KEY (role_id, event_id)
            )",
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_role_events_clock
             ON role_context_events(role_id, origin_clock)",
            [],
        )?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS role_context_devices (
                role_id TEXT NOT NULL,
                device_id TEXT NOT NULL,
                last_seen TEXT NOT NULL,
                PRIMARY KEY (role_id, device_id)
            )",
            [],
        )?;

        debug!("Role context store schema initialized");
        Ok(())
    }

    /// Append an event to a role context.
    ///
    /// Duplicate `event_id`s for the same role are ignored, making the operation
    /// idempotent.
    pub async fn append_event(&self, role_id: &str, event: &ClawContextEvent) -> Result<()> {
        let role_id = role_id.to_string();
        let event_id = event.event_id.clone();
        let origin_clock = event.origin_clock as i64;
        let event_json = serde_json::to_string(event)
            .map_err(|e| RoleContextStoreError::Serialization(e.to_string()))?;
        let conn = self.conn.clone();
        let now = Utc::now().to_rfc3339();

        tokio::task::spawn_blocking(move || {
            let mut conn = conn.lock();
            let tx = conn.transaction()?;

            tx.execute(
                "INSERT OR IGNORE INTO role_contexts (role_id, updated_at) VALUES (?1, ?2)",
                params![role_id, now.clone()],
            )?;

            tx.execute(
                "INSERT OR IGNORE INTO role_context_events
                 (role_id, event_id, origin_clock, event_json, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![role_id, event_id, origin_clock, event_json, now],
            )?;

            tx.execute(
                "UPDATE role_contexts SET updated_at = ?1 WHERE role_id = ?2",
                params![Utc::now().to_rfc3339(), role_id],
            )?;

            tx.commit()?;
            Ok::<_, RoleContextStoreError>(())
        })
        .await
        .map_err(|e| RoleContextStoreError::Io(std::io::Error::other(e.to_string())))??;

        Ok(())
    }

    /// List events for a role, optionally starting after `since_event_id`.
    ///
    /// Events are returned ordered by `(origin_clock, event_id)` so the client
    /// can merge them deterministically.
    pub async fn list_events(
        &self,
        role_id: &str,
        since_event_id: Option<&str>,
    ) -> Result<Vec<ClawContextEvent>> {
        let role_id = role_id.to_string();
        let since_event_id = since_event_id.map(|s| s.to_string());
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock();
            let mut events = Vec::new();

            // ponytail: load all events for the role and filter/sort in Rust.
            // Replace with an indexed SQL range query if roles grow beyond ~1k
            // events.
            let mut stmt = conn.prepare(
                "SELECT event_json FROM role_context_events WHERE role_id = ?",
            )?;
            let rows = stmt.query_map(params![role_id], |row| {
                let json: String = row.get(0)?;
                Ok(json)
            })?;

            for row in rows {
                let json = row?;
                match serde_json::from_str::<ClawContextEvent>(&json) {
                    Ok(event) => events.push(event),
                    Err(e) => {
                        tracing::warn!(error = %e, "skipping malformed role context event in store");
                    }
                }
            }

            events.sort_by(|a, b| {
                a.origin_clock
                    .cmp(&b.origin_clock)
                    .then_with(|| a.event_id.cmp(&b.event_id))
            });

            if let Some(since) = since_event_id {
                let since_idx = events.iter().position(|e| e.event_id == since);
                if let Some(idx) = since_idx {
                    events = events.split_off(idx + 1);
                }
            }

            Ok::<_, RoleContextStoreError>(events)
        })
        .await
        .map_err(|e| RoleContextStoreError::Io(std::io::Error::other(e.to_string())))?
    }

    /// Record that `device_id` is currently participating in `role_id`.
    pub async fn record_device_presence(&self, role_id: &str, device_id: &str) -> Result<()> {
        if device_id.is_empty() {
            return Ok(());
        }
        let role_id = role_id.to_string();
        let device_id = device_id.to_string();
        let conn = self.conn.clone();
        let now = Utc::now().to_rfc3339();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock();
            conn.execute(
                "INSERT INTO role_context_devices (role_id, device_id, last_seen)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(role_id, device_id) DO UPDATE SET last_seen = excluded.last_seen",
                params![role_id, device_id, now],
            )?;
            Ok::<_, RoleContextStoreError>(())
        })
        .await
        .map_err(|e| RoleContextStoreError::Io(std::io::Error::other(e.to_string())))??;

        Ok(())
    }

    /// List devices currently considered online for `role_id`.
    pub async fn online_devices(&self, role_id: &str) -> Result<Vec<String>> {
        let role_id = role_id.to_string();
        let conn = self.conn.clone();
        let cutoff = (Utc::now() - chrono::Duration::seconds(PRESENCE_TTL_SECONDS)).to_rfc3339();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock();
            let mut stmt = conn.prepare(
                "SELECT device_id FROM role_context_devices
                 WHERE role_id = ? AND last_seen > ?
                 ORDER BY device_id",
            )?;
            let rows = stmt.query_map(params![role_id, cutoff], |row| {
                let id: String = row.get(0)?;
                Ok(id)
            })?;

            let mut devices = Vec::new();
            for row in rows {
                devices.push(row?);
            }
            Ok::<_, RoleContextStoreError>(devices)
        })
        .await
        .map_err(|e| RoleContextStoreError::Io(std::io::Error::other(e.to_string())))?
    }

    /// Remove a device's presence record for a role.
    pub async fn remove_device_presence(&self, role_id: &str, device_id: &str) -> Result<()> {
        let role_id = role_id.to_string();
        let device_id = device_id.to_string();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock();
            conn.execute(
                "DELETE FROM role_context_devices WHERE role_id = ? AND device_id = ?",
                params![role_id, device_id],
            )?;
            Ok::<_, RoleContextStoreError>(())
        })
        .await
        .map_err(|e| RoleContextStoreError::Io(std::io::Error::other(e.to_string())))??;

        Ok(())
    }

    /// Delete all events and presence records for a role.
    pub async fn delete_role(&self, role_id: &str) -> Result<()> {
        let role_id = role_id.to_string();
        let conn = self.conn.clone();

        tokio::task::spawn_blocking(move || {
            let conn = conn.lock();
            conn.execute(
                "DELETE FROM role_context_events WHERE role_id = ?",
                params![role_id],
            )?;
            conn.execute(
                "DELETE FROM role_context_devices WHERE role_id = ?",
                params![role_id],
            )?;
            conn.execute(
                "DELETE FROM role_contexts WHERE role_id = ?",
                params![role_id],
            )?;
            Ok::<_, RoleContextStoreError>(())
        })
        .await
        .map_err(|e| RoleContextStoreError::Io(std::io::Error::other(e.to_string())))??;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::ContextEventKind;

    fn sample_event(event_id: &str, clock: u64) -> ClawContextEvent {
        ClawContextEvent {
            event_id: event_id.to_string(),
            origin_device: "device-a".to_string(),
            origin_clock: clock,
            kind: ContextEventKind::AppendMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn test_append_and_list_events() {
        let store = RoleContextStore::new_in_memory().unwrap();
        let role = "operator";

        store
            .append_event(role, &sample_event("ev-1", 1))
            .await
            .unwrap();
        store
            .append_event(role, &sample_event("ev-2", 2))
            .await
            .unwrap();

        let events = store.list_events(role, None).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_id, "ev-1");
        assert_eq!(events[1].event_id, "ev-2");
    }

    #[tokio::test]
    async fn test_duplicate_event_is_ignored() {
        let store = RoleContextStore::new_in_memory().unwrap();
        let role = "operator";

        let event = sample_event("ev-1", 1);
        store.append_event(role, &event).await.unwrap();
        store.append_event(role, &event).await.unwrap();

        let events = store.list_events(role, None).await.unwrap();
        assert_eq!(events.len(), 1);
    }

    #[tokio::test]
    async fn test_list_events_since_event_id() {
        let store = RoleContextStore::new_in_memory().unwrap();
        let role = "operator";

        store
            .append_event(role, &sample_event("ev-1", 1))
            .await
            .unwrap();
        store
            .append_event(role, &sample_event("ev-2", 2))
            .await
            .unwrap();
        store
            .append_event(role, &sample_event("ev-3", 3))
            .await
            .unwrap();

        let events = store.list_events(role, Some("ev-1")).await.unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_id, "ev-2");
        assert_eq!(events[1].event_id, "ev-3");
    }

    #[tokio::test]
    async fn test_device_presence() {
        let store = RoleContextStore::new_in_memory().unwrap();
        let role = "operator";

        assert!(store.online_devices(role).await.unwrap().is_empty());

        store.record_device_presence(role, "dev-1").await.unwrap();
        let devices = store.online_devices(role).await.unwrap();
        assert_eq!(devices, vec!["dev-1"]);

        store.remove_device_presence(role, "dev-1").await.unwrap();
        assert!(store.online_devices(role).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_delete_role() {
        let store = RoleContextStore::new_in_memory().unwrap();
        store
            .append_event("operator", &sample_event("ev-1", 1))
            .await
            .unwrap();
        store
            .record_device_presence("operator", "dev-1")
            .await
            .unwrap();

        store.delete_role("operator").await.unwrap();

        assert!(
            store
                .list_events("operator", None)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(store.online_devices("operator").await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_empty_device_id_is_noop() {
        let store = RoleContextStore::new_in_memory().unwrap();
        store.record_device_presence("operator", "").await.unwrap();
        assert!(store.online_devices("operator").await.unwrap().is_empty());
    }
}
