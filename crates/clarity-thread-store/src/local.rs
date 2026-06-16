//! Local disk-backed [`ThreadStore`] implementation.
//!
//! Combines JSONL rollout files (via `clarity_rollout::RolloutRecorder`) with a
//! SQLite metadata index. This is the Clarity adaptation of Codex's
//! `codex_thread_store::LocalThreadStore`. Codex is licensed under Apache-2.0;
//! see `NOTICES.md` for attribution.

use std::any::Any;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::Arc;

use chrono::Utc;
use clarity_contract::{
    CreateRolloutParams, ResumeRolloutParams, RolloutEventMsg, RolloutItem, RolloutResponseItem,
    SessionId, ThreadId,
};
use clarity_rollout::{RolloutConfig, RolloutConfigView, RolloutRecorder, load_rollout_items};
use parking_lot::Mutex;
use rusqlite::{Connection, OptionalExtension};
use tokio::sync::Mutex as AsyncMutex;

use crate::error::{ThreadStoreError, ThreadStoreResult};
use crate::store::{ThreadStore, ThreadStoreFuture};
use crate::types::{
    AppendThreadItemsParams, ArchiveThreadParams, CreateThreadParams, DeleteThreadParams,
    ForkSnapshot, ForkThreadParams, ListThreadsParams, LoadThreadHistoryParams, ReadThreadParams,
    ResumeThreadParams, StoredThread, StoredThreadHistory, ThreadPage, ThreadSummary,
    UpdateThreadMetadataParams,
};

/// Directory names used under the Clarity home directory.
const SESSIONS_SUBDIR: &str = "sessions";
const ARCHIVED_SESSIONS_SUBDIR: &str = "archived_sessions";

/// A thread store that persists rollout files to disk and keeps a SQLite index
/// for fast listing and metadata queries.
pub struct LocalThreadStore {
    config: RolloutConfig,
    conn: Mutex<Connection>,
    live_writers: AsyncMutex<HashMap<ThreadId, RolloutRecorder>>,
}

impl LocalThreadStore {
    /// Open or create a local thread store.
    pub fn new(config: RolloutConfig, state_db_path: impl AsRef<Path>) -> ThreadStoreResult<Self> {
        let path = state_db_path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        let _ = conn.execute_batch("PRAGMA journal_mode=WAL;");
        let _ = conn.execute_batch("PRAGMA foreign_keys=ON;");
        init_schema(&conn)?;
        Ok(Self {
            config,
            conn: Mutex::new(conn),
            live_writers: AsyncMutex::new(HashMap::new()),
        })
    }

    /// Default state DB path under the Clarity home directory.
    pub fn default_state_db_path(config: &impl RolloutConfigView) -> PathBuf {
        config.clarity_home().join("state.db")
    }

    fn rollout_path(&self, thread_id: ThreadId) -> PathBuf {
        self.config
            .clarity_home()
            .join(SESSIONS_SUBDIR)
            .join(format!("rollout-{}.jsonl", thread_id))
    }

    fn ensure_sessions_dir(&self) -> ThreadStoreResult<()> {
        std::fs::create_dir_all(self.config.clarity_home().join(SESSIONS_SUBDIR))?;
        std::fs::create_dir_all(self.config.clarity_home().join(ARCHIVED_SESSIONS_SUBDIR))?;
        Ok(())
    }

    fn db_upsert_thread(
        &self,
        thread_id: ThreadId,
        session_id: SessionId,
        rollout_path: &Path,
        parent: Option<ThreadId>,
        forked: Option<ThreadId>,
    ) -> ThreadStoreResult<()> {
        let now = Utc::now().timestamp_millis();
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO threads (id, session_id, rollout_path, created_at, updated_at, archived, parent_thread_id, forked_from_id)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
                 session_id=excluded.session_id,
                 rollout_path=excluded.rollout_path,
                 updated_at=excluded.updated_at,
                 parent_thread_id=excluded.parent_thread_id,
                 forked_from_id=excluded.forked_from_id",
            rusqlite::params![
                thread_id.to_string(),
                session_id.to_string(),
                rollout_path.to_string_lossy(),
                now,
                now,
                parent.map(|id| id.to_string()),
                forked.map(|id| id.to_string()),
            ],
        )?;
        Ok(())
    }

    fn db_update_timestamp(&self, thread_id: ThreadId) -> ThreadStoreResult<()> {
        let now = Utc::now().timestamp_millis();
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE threads SET updated_at = ?1 WHERE id = ?2",
            rusqlite::params![now, thread_id.to_string()],
        )?;
        Ok(())
    }

    fn db_thread_summary(&self, thread_id: ThreadId) -> ThreadStoreResult<Option<ThreadSummary>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT id, session_id, title, created_at, updated_at, archived, parent_thread_id, forked_from_id
             FROM threads WHERE id = ?1",
        )?;
        let row = stmt
            .query_row([thread_id.to_string()], |row| {
                let id_str: String = row.get(0)?;
                let session_str: String = row.get(1)?;
                let parent: Option<String> = row.get(6)?;
                let forked: Option<String> = row.get(7)?;
                Ok(ThreadSummary {
                    thread_id: ThreadId::from_string(&id_str).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?,
                    session_id: SessionId::from_string(&session_str).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?,
                    title: row.get(2)?,
                    created_at: chrono::DateTime::from_timestamp_millis(row.get(3)?)
                        .unwrap_or_else(Utc::now),
                    updated_at: chrono::DateTime::from_timestamp_millis(row.get(4)?)
                        .unwrap_or_else(Utc::now),
                    archived: row.get(5)?,
                    parent_thread_id: parent.and_then(|s| ThreadId::from_string(&s).ok()),
                    forked_from_id: forked.and_then(|s| ThreadId::from_string(&s).ok()),
                })
            })
            .optional()?;
        Ok(row)
    }
}

impl ThreadStore for LocalThreadStore {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn create_thread(&self, params: CreateThreadParams) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            self.ensure_sessions_dir()?;
            if self.db_thread_summary(params.thread_id)?.is_some() {
                return Err(ThreadStoreError::duplicate(params.thread_id.to_string()));
            }

            let rollout_path = self.rollout_path(params.thread_id);
            let recorder = RolloutRecorder::create(
                &self.config,
                CreateRolloutParams {
                    thread_id: params.thread_id,
                    session_id: params.session_id,
                    forked_from_id: params.forked_from_id,
                    parent_thread_id: params.parent_thread_id,
                    source: params.source,
                    thread_source: params.thread_source,
                    cwd: params.cwd,
                    originator: params.originator,
                    cli_version: params.cli_version,
                    base_instructions: params.base_instructions,
                    dynamic_tools: params.dynamic_tools,
                    model_provider: params.model_provider,
                    multi_agent_version: params.multi_agent_version,
                    skip_initial_meta: false,
                },
            )
            .await?;

            self.db_upsert_thread(
                params.thread_id,
                params.session_id,
                &rollout_path,
                params.parent_thread_id,
                params.forked_from_id,
            )?;

            self.live_writers
                .lock()
                .await
                .insert(params.thread_id, recorder);
            Ok(())
        })
    }

    fn resume_thread(&self, params: ResumeThreadParams) -> ThreadStoreFuture<'_, StoredThread> {
        Box::pin(async move {
            let summary = self
                .db_thread_summary(params.thread_id)?
                .ok_or_else(|| ThreadStoreError::not_found(params.thread_id.to_string()))?;
            let rollout_path = self.rollout_path(params.thread_id);
            let recorder =
                RolloutRecorder::resume(&self.config, ResumeRolloutParams { path: rollout_path })
                    .await?;
            self.live_writers
                .lock()
                .await
                .insert(params.thread_id, recorder);
            Ok(StoredThread {
                thread_id: summary.thread_id,
                session_id: summary.session_id,
                title: summary.title,
                rollout_path: Some(self.rollout_path(params.thread_id)),
                created_at: summary.created_at,
                updated_at: summary.updated_at,
                archived: summary.archived,
                parent_thread_id: summary.parent_thread_id,
                forked_from_id: summary.forked_from_id,
                history: None,
            })
        })
    }

    fn append_items(&self, params: AppendThreadItemsParams) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let writers = self.live_writers.lock().await;
            let recorder = writers
                .get(&params.thread_id)
                .ok_or_else(|| ThreadStoreError::not_found(params.thread_id.to_string()))?
                .clone();
            drop(writers);
            recorder.add_items(params.items).await?;
            self.db_update_timestamp(params.thread_id)?;
            Ok(())
        })
    }

    fn persist_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let recorder = self
                .live_writers
                .lock()
                .await
                .get(&thread_id)
                .ok_or_else(|| ThreadStoreError::not_found(thread_id.to_string()))?
                .clone();
            recorder.persist().await.map_err(ThreadStoreError::Io)
        })
    }

    fn flush_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let recorder = self
                .live_writers
                .lock()
                .await
                .get(&thread_id)
                .ok_or_else(|| ThreadStoreError::not_found(thread_id.to_string()))?
                .clone();
            recorder.flush().await.map_err(ThreadStoreError::Io)
        })
    }

    fn shutdown_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let recorder = self
                .live_writers
                .lock()
                .await
                .remove(&thread_id)
                .ok_or_else(|| ThreadStoreError::not_found(thread_id.to_string()))?;
            recorder.shutdown().await.map_err(ThreadStoreError::Io)
        })
    }

    fn discard_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            self.live_writers.lock().await.remove(&thread_id);
            Ok(())
        })
    }

    fn load_history(
        &self,
        params: LoadThreadHistoryParams,
    ) -> ThreadStoreFuture<'_, StoredThreadHistory> {
        Box::pin(async move {
            let path = self.rollout_path(params.thread_id);
            let mut items = load_rollout_items(&path).await?;
            if !params.include_compacted {
                items.retain(|item| !matches!(item, RolloutItem::Compacted(_)));
            }
            if let Some(before_turn) = params.before_turn {
                let mut user_count = 0;
                items = items
                    .into_iter()
                    .take_while(|item| {
                        if matches!(item, RolloutItem::EventMsg(RolloutEventMsg::UserMessage(_))) {
                            user_count += 1;
                        }
                        user_count < before_turn
                    })
                    .collect();
            }
            Ok(StoredThreadHistory { items })
        })
    }

    fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreFuture<'_, StoredThread> {
        Box::pin(async move {
            let summary = self
                .db_thread_summary(params.thread_id)?
                .ok_or_else(|| ThreadStoreError::not_found(params.thread_id.to_string()))?;
            let history = if params.include_history {
                Some(
                    self.load_history(LoadThreadHistoryParams {
                        thread_id: params.thread_id,
                        before_turn: None,
                        include_compacted: true,
                    })
                    .await?,
                )
            } else {
                None
            };
            Ok(StoredThread {
                thread_id: summary.thread_id,
                session_id: summary.session_id,
                title: summary.title,
                rollout_path: Some(self.rollout_path(params.thread_id)),
                created_at: summary.created_at,
                updated_at: summary.updated_at,
                archived: summary.archived,
                parent_thread_id: summary.parent_thread_id,
                forked_from_id: summary.forked_from_id,
                history,
            })
        })
    }

    fn list_threads(&self, params: ListThreadsParams) -> ThreadStoreFuture<'_, ThreadPage> {
        Box::pin(async move {
            let conn = self.conn.lock();
            let archived_clause = if params.include_archived {
                ""
            } else {
                "WHERE archived = 0"
            };
            let sql = format!(
                "SELECT id, session_id, title, created_at, updated_at, archived, parent_thread_id, forked_from_id
                 FROM threads {}
                 ORDER BY updated_at DESC
                 LIMIT ?1",
                archived_clause
            );
            let limit = params.limit as i64;
            let mut stmt = conn.prepare(&sql)?;
            let rows = stmt.query_map([limit], |row| {
                let id_str: String = row.get(0)?;
                let session_str: String = row.get(1)?;
                let parent: Option<String> = row.get(6)?;
                let forked: Option<String> = row.get(7)?;
                Ok(ThreadSummary {
                    thread_id: ThreadId::from_string(&id_str).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?,
                    session_id: SessionId::from_string(&session_str).map_err(|e| {
                        rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        )
                    })?,
                    title: row.get(2)?,
                    created_at: chrono::DateTime::from_timestamp_millis(row.get(3)?)
                        .unwrap_or_else(Utc::now),
                    updated_at: chrono::DateTime::from_timestamp_millis(row.get(4)?)
                        .unwrap_or_else(Utc::now),
                    archived: row.get(5)?,
                    parent_thread_id: parent.and_then(|s| ThreadId::from_string(&s).ok()),
                    forked_from_id: forked.and_then(|s| ThreadId::from_string(&s).ok()),
                })
            })?;
            let mut data = Vec::new();
            for row in rows {
                data.push(row?);
            }
            Ok(ThreadPage {
                data,
                next_cursor: None,
            })
        })
    }

    fn update_thread_metadata(
        &self,
        params: UpdateThreadMetadataParams,
    ) -> ThreadStoreFuture<'_, StoredThread> {
        Box::pin(async move {
            {
                let conn = self.conn.lock();
                if let Some(title) = &params.patch.title {
                    conn.execute(
                        "UPDATE threads SET title = ?1, updated_at = ?2 WHERE id = ?3",
                        rusqlite::params![
                            title,
                            Utc::now().timestamp_millis(),
                            params.thread_id.to_string()
                        ],
                    )?;
                }
                if let Some(archived) = params.patch.archived {
                    conn.execute(
                        "UPDATE threads SET archived = ?1, updated_at = ?2 WHERE id = ?3",
                        rusqlite::params![
                            archived as i32,
                            Utc::now().timestamp_millis(),
                            params.thread_id.to_string()
                        ],
                    )?;
                }
            }
            self.read_thread(ReadThreadParams {
                thread_id: params.thread_id,
                include_history: false,
            })
            .await
        })
    }

    fn archive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let old_path = self.rollout_path(params.thread_id);
            let new_path = self
                .config
                .clarity_home()
                .join(ARCHIVED_SESSIONS_SUBDIR)
                .join(format!("rollout-{}.jsonl", params.thread_id));
            {
                let conn = self.conn.lock();
                let rows = conn.execute(
                    "UPDATE threads SET archived = 1, updated_at = ?1 WHERE id = ?2",
                    rusqlite::params![Utc::now().timestamp_millis(), params.thread_id.to_string()],
                )?;
                if rows == 0 {
                    return Err(ThreadStoreError::not_found(params.thread_id.to_string()));
                }
            }
            if old_path.exists() {
                tokio::fs::rename(&old_path, &new_path).await?;
            }
            Ok(())
        })
    }

    fn unarchive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreFuture<'_, StoredThread> {
        Box::pin(async move {
            let old_path = self
                .config
                .clarity_home()
                .join(ARCHIVED_SESSIONS_SUBDIR)
                .join(format!("rollout-{}.jsonl", params.thread_id));
            let new_path = self.rollout_path(params.thread_id);
            {
                let conn = self.conn.lock();
                conn.execute(
                    "UPDATE threads SET archived = 0, updated_at = ?1 WHERE id = ?2",
                    rusqlite::params![Utc::now().timestamp_millis(), params.thread_id.to_string()],
                )?;
            }
            if old_path.exists() {
                tokio::fs::rename(&old_path, &new_path).await?;
            }
            self.read_thread(ReadThreadParams {
                thread_id: params.thread_id,
                include_history: false,
            })
            .await
        })
    }

    fn delete_thread(&self, params: DeleteThreadParams) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            self.live_writers.lock().await.remove(&params.thread_id);
            {
                let conn = self.conn.lock();
                conn.execute(
                    "DELETE FROM thread_spawn_edges WHERE parent_thread_id = ?1 OR child_thread_id = ?1",
                    [params.thread_id.to_string()],
                )?;
                conn.execute(
                    "DELETE FROM threads WHERE id = ?1",
                    [params.thread_id.to_string()],
                )?;
            }
            let active = self.rollout_path(params.thread_id);
            let archived = self
                .config
                .clarity_home()
                .join(ARCHIVED_SESSIONS_SUBDIR)
                .join(format!("rollout-{}.jsonl", params.thread_id));
            for path in [active, archived] {
                if path.exists() {
                    tokio::fs::remove_file(&path).await?;
                }
            }
            Ok(())
        })
    }

    fn fork_thread(&self, params: ForkThreadParams) -> ThreadStoreFuture<'_, ThreadId> {
        Box::pin(async move {
            let source_items = self
                .load_history(LoadThreadHistoryParams {
                    thread_id: params.source_thread_id,
                    before_turn: None,
                    include_compacted: true,
                })
                .await?;

            let new_items: Vec<RolloutItem> = match params.snapshot {
                ForkSnapshot::TruncateBeforeNthUserMessage(n) => {
                    let mut user_count = 0;
                    source_items
                        .items
                        .into_iter()
                        .take_while(|item| {
                            if is_user_message_item(item) {
                                user_count += 1;
                            }
                            user_count < n
                        })
                        .collect()
                }
                ForkSnapshot::Interrupted => source_items.items,
            };

            let new_thread_id = params.new_thread_id.unwrap_or_default();
            let session_id = {
                self.db_thread_summary(params.source_thread_id)?
                    .map(|s| s.session_id)
                    .unwrap_or_default()
            };
            // Create the new thread without an initial SessionMeta line; the forked
            // history already contains the source SessionMeta, and writing a new
            // meta here would duplicate it.
            let rollout_path = self.rollout_path(new_thread_id);
            let recorder = RolloutRecorder::create(
                &self.config,
                CreateRolloutParams {
                    thread_id: new_thread_id,
                    session_id,
                    forked_from_id: Some(params.source_thread_id),
                    parent_thread_id: Some(params.source_thread_id),
                    source: clarity_contract::SessionSource::Cli,
                    thread_source: Some(clarity_contract::ThreadSource::Forked),
                    cwd: self.config.cwd.clone(),
                    originator: "clarity-thread-store".into(),
                    cli_version: env!("CARGO_PKG_VERSION").into(),
                    base_instructions: None,
                    dynamic_tools: Vec::new(),
                    model_provider: Some(self.config.model_provider_id.clone()),
                    multi_agent_version: None,
                    skip_initial_meta: true,
                },
            )
            .await?;

            self.db_upsert_thread(
                new_thread_id,
                session_id,
                &rollout_path,
                Some(params.source_thread_id),
                Some(params.source_thread_id),
            )?;

            self.live_writers
                .lock()
                .await
                .insert(new_thread_id, recorder);

            if !new_items.is_empty() {
                self.append_items(AppendThreadItemsParams {
                    thread_id: new_thread_id,
                    items: new_items,
                })
                .await?;
            }

            // Record spawn edge.
            {
                let conn = self.conn.lock();
                conn.execute(
                    "INSERT INTO thread_spawn_edges (parent_thread_id, child_thread_id, created_at)
                     VALUES (?1, ?2, ?3)
                     ON CONFLICT(parent_thread_id, child_thread_id) DO NOTHING",
                    rusqlite::params![
                        params.source_thread_id.to_string(),
                        new_thread_id.to_string(),
                        Utc::now().timestamp_millis()
                    ],
                )?;
            }

            self.persist_thread(new_thread_id).await?;

            Ok(new_thread_id)
        })
    }
}

fn is_user_message_item(item: &RolloutItem) -> bool {
    match item {
        RolloutItem::EventMsg(RolloutEventMsg::UserMessage(_)) => true,
        RolloutItem::ResponseItem(RolloutResponseItem::Message { role, .. }) => role == "user",
        _ => false,
    }
}

fn init_schema(conn: &Connection) -> ThreadStoreResult<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS threads (
            id TEXT PRIMARY KEY,
            session_id TEXT NOT NULL,
            rollout_path TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            title TEXT,
            archived INTEGER NOT NULL DEFAULT 0,
            parent_thread_id TEXT,
            forked_from_id TEXT
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_threads_updated_at ON threads(updated_at DESC)",
        [],
    )?;
    conn.execute(
        "CREATE TABLE IF NOT EXISTS thread_spawn_edges (
            parent_thread_id TEXT NOT NULL,
            child_thread_id TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            PRIMARY KEY (parent_thread_id, child_thread_id)
        )",
        [],
    )?;
    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_thread_spawn_edges_parent ON thread_spawn_edges(parent_thread_id)",
        [],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::{RolloutResponseItem, SessionSource};
    use tempfile::TempDir;

    struct TestConfig {
        home: PathBuf,
    }

    impl RolloutConfigView for TestConfig {
        fn clarity_home(&self) -> &Path {
            &self.home
        }
        fn sqlite_home(&self) -> &Path {
            &self.home
        }
        fn cwd(&self) -> &Path {
            &self.home
        }
        fn model_provider_id(&self) -> &str {
            "test"
        }
        fn generate_memories(&self) -> bool {
            false
        }
    }

    #[tokio::test]
    async fn create_and_list_local_thread() {
        let dir = TempDir::new().unwrap();
        let config = RolloutConfig::from_view(&TestConfig {
            home: dir.path().to_path_buf(),
        });
        let store = LocalThreadStore::new(config, dir.path().join("state.db")).unwrap();
        let thread_id = ThreadId::new();
        store
            .create_thread(CreateThreadParams {
                thread_id,
                session_id: SessionId::from(thread_id),
                source: SessionSource::Cli,
                ..Default::default()
            })
            .await
            .unwrap();
        let page = store
            .list_threads(ListThreadsParams::default())
            .await
            .unwrap();
        assert_eq!(page.data.len(), 1);
    }

    #[tokio::test]
    async fn archive_and_unarchive_local_thread() {
        let dir = TempDir::new().unwrap();
        let config = RolloutConfig::from_view(&TestConfig {
            home: dir.path().to_path_buf(),
        });
        let store = LocalThreadStore::new(config, dir.path().join("state.db")).unwrap();
        let thread_id = ThreadId::new();
        store
            .create_thread(CreateThreadParams {
                thread_id,
                session_id: SessionId::from(thread_id),
                source: SessionSource::Cli,
                ..Default::default()
            })
            .await
            .unwrap();
        store
            .archive_thread(ArchiveThreadParams { thread_id })
            .await
            .unwrap();
        let page = store
            .list_threads(ListThreadsParams {
                include_archived: false,
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(page.data.is_empty());
    }

    #[tokio::test]
    async fn concurrent_append_to_same_thread() {
        let dir = TempDir::new().unwrap();
        let config = RolloutConfig::from_view(&TestConfig {
            home: dir.path().to_path_buf(),
        });
        let store = LocalThreadStore::new(config, dir.path().join("state.db")).unwrap();
        let thread_id = ThreadId::new();
        store
            .create_thread(CreateThreadParams {
                thread_id,
                session_id: SessionId::from(thread_id),
                source: SessionSource::Cli,
                ..Default::default()
            })
            .await
            .unwrap();

        let store = Arc::new(store);
        let mut handles = Vec::new();
        for batch in 0..5 {
            let store = Arc::clone(&store);
            handles.push(tokio::spawn(async move {
                for i in 0..10 {
                    store
                        .append_items(AppendThreadItemsParams {
                            thread_id,
                            items: vec![RolloutItem::ResponseItem(RolloutResponseItem::Message {
                                role: "user".to_string(),
                                content: format!("batch {batch} message {i}"),
                            })],
                        })
                        .await
                        .unwrap();
                }
            }));
        }

        for h in handles {
            h.await.unwrap();
        }
        store.persist_thread(thread_id).await.unwrap();

        let history = store
            .load_history(LoadThreadHistoryParams {
                thread_id,
                include_compacted: false,
                before_turn: None,
            })
            .await
            .unwrap();
        // create_thread wrote a SessionMeta item, so total is 50 user messages + 1 meta.
        assert_eq!(history.items.len(), 51);
    }

    #[tokio::test]
    async fn fork_thread_persists_history() {
        let dir = TempDir::new().unwrap();
        let config = RolloutConfig::from_view(&TestConfig {
            home: dir.path().to_path_buf(),
        });
        let store = LocalThreadStore::new(config, dir.path().join("state.db")).unwrap();
        let source_id = ThreadId::new();
        store
            .create_thread(CreateThreadParams {
                thread_id: source_id,
                session_id: SessionId::from(source_id),
                source: SessionSource::Cli,
                ..Default::default()
            })
            .await
            .unwrap();

        for i in 0..5 {
            store
                .append_items(AppendThreadItemsParams {
                    thread_id: source_id,
                    items: vec![RolloutItem::EventMsg(RolloutEventMsg::UserMessage(
                        format!("msg {i}"),
                    ))],
                })
                .await
                .unwrap();
        }
        store.persist_thread(source_id).await.unwrap();

        let new_id = store
            .fork_thread(ForkThreadParams {
                source_thread_id: source_id,
                snapshot: ForkSnapshot::Interrupted,
                new_thread_id: None,
            })
            .await
            .unwrap();
        store.persist_thread(new_id).await.unwrap();

        let source_history = store
            .load_history(LoadThreadHistoryParams {
                thread_id: source_id,
                include_compacted: false,
                before_turn: None,
            })
            .await
            .unwrap();
        let fork_history = store
            .load_history(LoadThreadHistoryParams {
                thread_id: new_id,
                include_compacted: false,
                before_turn: None,
            })
            .await
            .unwrap();

        // Source: one SessionMeta + 5 user messages.
        assert_eq!(source_history.items.len(), 6);
        // Fork copies the source history verbatim (no extra SessionMeta).
        assert_eq!(fork_history.items.len(), 6);
        assert_ne!(source_id, new_id);
    }

    #[tokio::test]
    async fn fork_truncate_before_nth_user_message() {
        let dir = TempDir::new().unwrap();
        let config = RolloutConfig::from_view(&TestConfig {
            home: dir.path().to_path_buf(),
        });
        let store = LocalThreadStore::new(config, dir.path().join("state.db")).unwrap();
        let source_id = ThreadId::new();
        store
            .create_thread(CreateThreadParams {
                thread_id: source_id,
                session_id: SessionId::from(source_id),
                source: SessionSource::Cli,
                ..Default::default()
            })
            .await
            .unwrap();

        for i in 0..3 {
            store
                .append_items(AppendThreadItemsParams {
                    thread_id: source_id,
                    items: vec![
                        RolloutItem::EventMsg(RolloutEventMsg::UserMessage(format!("user {i}"))),
                        RolloutItem::ResponseItem(RolloutResponseItem::Message {
                            role: "assistant".to_string(),
                            content: format!("assistant {i}"),
                        }),
                    ],
                })
                .await
                .unwrap();
        }
        store.persist_thread(source_id).await.unwrap();

        let new_id = store
            .fork_thread(ForkThreadParams {
                source_thread_id: source_id,
                snapshot: ForkSnapshot::TruncateBeforeNthUserMessage(2),
                new_thread_id: None,
            })
            .await
            .unwrap();
        store.persist_thread(new_id).await.unwrap();

        let fork_history = store
            .load_history(LoadThreadHistoryParams {
                thread_id: new_id,
                include_compacted: false,
                before_turn: None,
            })
            .await
            .unwrap();

        // Fork truncates before the 2nd user message, keeping source SessionMeta +
        // user 0 + assistant 0 (no extra SessionMeta).
        assert_eq!(fork_history.items.len(), 3);
    }
}
