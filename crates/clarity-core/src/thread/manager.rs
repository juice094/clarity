//! High-level thread orchestrator.
//!
//! [`ThreadManager`] bridges the storage-neutral [`ThreadStore`] trait with
//! Clarity's runtime by converting durable rollout items into LLM messages and
//! vice-versa.

use std::path::Path;
use std::sync::Arc;

use chrono::Utc;
use clarity_contract::{
    Message, MessageRole, RolloutEventMsg, RolloutItem, RolloutResponseItem, SessionId,
    SessionMeta, SessionSource, ThreadId, ThreadSource,
};
use clarity_thread_store::{
    AppendThreadItemsParams, CreateThreadParams, DeleteThreadParams, ForkSnapshot,
    ForkThreadParams, InMemoryThreadStore, ListThreadsParams, LoadThreadHistoryParams,
    ReadThreadParams, ResumeThreadParams, ThreadStore, ThreadStoreError, ThreadSummary,
    UpdateThreadMetadataParams,
};
use clarity_wire::{Wire, WireMessage};
use thiserror::Error;

/// Errors that can occur inside [`ThreadManager`].
#[derive(Debug, Error)]
pub enum ThreadManagerError {
    /// Underlying thread store error.
    #[error("thread store error: {0}")]
    ThreadStore(#[from] ThreadStoreError),

    /// Failed to serialize or deserialize a rollout payload.
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// The thread exists but contains no session metadata line.
    #[error("thread {0} has no session metadata")]
    MissingSessionMeta(ThreadId),

    /// A rollout item could not be mapped to an LLM message.
    #[error("invalid rollout item for thread {0}: {1}")]
    InvalidRolloutItem(ThreadId, String),
}

/// Result type alias for thread manager operations.
pub type ThreadManagerResult<T> = Result<T, ThreadManagerError>;

/// High-level thread lifecycle manager.
///
/// Owns a [`ThreadStore`] implementation and exposes runtime-friendly helpers
/// for creating threads, loading LLM-ready history, and appending turn results.
#[derive(Clone)]
pub struct ThreadManager {
    store: Arc<dyn ThreadStore>,
    cli_version: String,
    default_source: SessionSource,
    wire: Option<Wire>,
}

impl ThreadManager {
    /// Create a manager wrapping the given store.
    pub fn new(store: Arc<dyn ThreadStore>) -> Self {
        Self {
            store,
            cli_version: crate::VERSION.to_string(),
            default_source: SessionSource::Unknown,
            wire: None,
        }
    }

    /// Create a manager with a default in-memory store, useful for tests.
    #[must_use]
    pub fn new_in_memory() -> Self {
        Self::new(Arc::new(InMemoryThreadStore::new()))
    }

    /// Set the default runtime source for newly created threads.
    #[must_use]
    pub fn with_source(mut self, source: SessionSource) -> Self {
        self.default_source = source;
        self
    }

    /// Set the CLI version recorded in new thread metadata.
    #[must_use]
    pub fn with_cli_version(mut self, version: impl Into<String>) -> Self {
        self.cli_version = version.into();
        self
    }

    /// Attach a [`Wire`] so thread lifecycle events are broadcast to UI consumers.
    #[must_use]
    pub fn with_wire(mut self, wire: Wire) -> Self {
        self.wire = Some(wire);
        self
    }

    fn broadcast(&self, message: WireMessage) {
        if let Some(wire) = &self.wire {
            let _ = wire.soul_side().send(message);
        }
    }

    #[allow(dead_code)]
    /// Create a new root thread and return its identifier.
    ///
    /// The `session_id` is set to the same UUID as the thread, matching Codex's
    /// root-session semantics.
    pub async fn create_thread(
        &self,
        cwd: impl AsRef<Path>,
        originator: impl Into<String>,
        source: impl Into<Option<SessionSource>>,
    ) -> ThreadManagerResult<ThreadId> {
        let thread_id = ThreadId::new();
        let session_id = SessionId::from(thread_id);
        let source = source.into().unwrap_or_else(|| self.default_source.clone());
        let meta = SessionMeta {
            id: thread_id,
            timestamp: Utc::now().to_rfc3339(),
            cwd: cwd.as_ref().to_path_buf(),
            originator: originator.into(),
            cli_version: self.cli_version.clone(),
            source,
            thread_source: Some(ThreadSource::New),
            ..SessionMeta::default()
        };

        self.store
            .create_thread(CreateThreadParams {
                thread_id,
                session_id,
                forked_from_id: None,
                parent_thread_id: None,
                source: meta.source.clone(),
                thread_source: meta.thread_source.clone(),
                cwd: meta.cwd.clone(),
                originator: meta.originator.clone(),
                cli_version: meta.cli_version.clone(),
                base_instructions: meta.base_instructions.clone(),
                dynamic_tools: meta.dynamic_tools.clone().unwrap_or_default(),
                model_provider: meta.model_provider.clone(),
                generate_memories: false,
                multi_agent_version: meta.multi_agent_version.clone(),
            })
            .await?;

        self.broadcast(WireMessage::ThreadCreated {
            thread_id: thread_id.to_string(),
            title: None,
        });
        self.broadcast(WireMessage::ThreadActive {
            thread_id: thread_id.to_string(),
            title: None,
        });

        Ok(thread_id)
    }

    /// Resume an existing thread.
    pub async fn resume_thread(
        &self,
        thread_id: ThreadId,
    ) -> ThreadManagerResult<clarity_thread_store::StoredThread> {
        let session_id = SessionId::from(thread_id);
        self.store
            .resume_thread(ResumeThreadParams {
                thread_id,
                session_id,
            })
            .await
            .map_err(Into::into)
    }

    /// Load the persisted rollout history as LLM messages.
    ///
    /// Only `ResponseItem` variants are converted; other rollout atoms are
    /// skipped. Compacted replacement histories are included when present.
    pub async fn load_llm_history(&self, thread_id: ThreadId) -> ThreadManagerResult<Vec<Message>> {
        let history = self
            .store
            .load_history(LoadThreadHistoryParams {
                thread_id,
                before_turn: None,
                include_compacted: true,
            })
            .await?;

        let mut messages = Vec::new();
        for item in history.items {
            if let RolloutItem::ResponseItem(response) = item {
                if let Some(msg) = response_item_to_message(thread_id, &response)? {
                    messages.push(msg);
                }
            }
        }
        Ok(messages)
    }

    /// Append a completed user/assistant turn to the thread.
    pub async fn append_turn(
        &self,
        thread_id: ThreadId,
        user_prompt: impl Into<String>,
        assistant_response: impl Into<String>,
    ) -> ThreadManagerResult<()> {
        let user_prompt = user_prompt.into();
        let assistant_response = assistant_response.into();

        let items = vec![
            RolloutItem::ResponseItem(RolloutResponseItem::Message {
                role: "user".to_string(),
                content: user_prompt,
            }),
            RolloutItem::EventMsg(RolloutEventMsg::TurnComplete { turn_id: None }),
            RolloutItem::ResponseItem(RolloutResponseItem::Message {
                role: "assistant".to_string(),
                content: assistant_response,
            }),
        ];

        self.store
            .append_items(AppendThreadItemsParams { thread_id, items })
            .await?;
        self.store.persist_thread(thread_id).await?;
        self.broadcast(WireMessage::ThreadUpdated {
            thread_id: thread_id.to_string(),
            title: None,
            archived: None,
        });
        Ok(())
    }

    /// Append raw rollout items and persist the thread.
    pub async fn append_items(
        &self,
        thread_id: ThreadId,
        items: Vec<RolloutItem>,
    ) -> ThreadManagerResult<()> {
        self.store
            .append_items(AppendThreadItemsParams { thread_id, items })
            .await?;
        self.store.persist_thread(thread_id).await?;
        Ok(())
    }

    /// Flush all pending items for a thread.
    pub async fn flush(&self, thread_id: ThreadId) -> ThreadManagerResult<()> {
        self.store.flush_thread(thread_id).await?;
        Ok(())
    }

    /// Flush and close the live writer for a thread.
    pub async fn shutdown(&self, thread_id: ThreadId) -> ThreadManagerResult<()> {
        self.store.shutdown_thread(thread_id).await?;
        Ok(())
    }

    /// Archive a thread.
    pub async fn archive(&self, thread_id: ThreadId) -> ThreadManagerResult<()> {
        self.store
            .archive_thread(clarity_thread_store::ArchiveThreadParams { thread_id })
            .await?;
        Ok(())
    }

    /// Unarchive a thread and return its updated metadata.
    pub async fn unarchive(
        &self,
        thread_id: ThreadId,
    ) -> ThreadManagerResult<clarity_thread_store::StoredThread> {
        self.store
            .unarchive_thread(clarity_thread_store::ArchiveThreadParams { thread_id })
            .await
            .map_err(Into::into)
    }

    /// Delete a thread.
    pub async fn delete(&self, thread_id: ThreadId) -> ThreadManagerResult<()> {
        self.store
            .delete_thread(DeleteThreadParams { thread_id })
            .await?;
        Ok(())
    }

    /// List threads with optional pagination.
    pub async fn list_threads(
        &self,
        limit: usize,
        include_archived: bool,
        cursor: Option<String>,
    ) -> ThreadManagerResult<Vec<ThreadSummary>> {
        let page = self
            .store
            .list_threads(ListThreadsParams {
                limit,
                cursor,
                include_archived,
            })
            .await?;
        Ok(page.data)
    }

    /// Read a thread summary and optionally its full history.
    pub async fn read_thread(
        &self,
        thread_id: ThreadId,
        include_history: bool,
    ) -> ThreadManagerResult<clarity_thread_store::StoredThread> {
        self.store
            .read_thread(ReadThreadParams {
                thread_id,
                include_history,
            })
            .await
            .map_err(Into::into)
    }

    /// Update thread metadata.
    pub async fn update_metadata(
        &self,
        thread_id: ThreadId,
        patch: clarity_thread_store::ThreadMetadataPatch,
    ) -> ThreadManagerResult<clarity_thread_store::StoredThread> {
        let title = patch.title.clone();
        let archived = patch.archived;
        let stored = self
            .store
            .update_thread_metadata(UpdateThreadMetadataParams { thread_id, patch })
            .await?;
        self.broadcast(WireMessage::ThreadUpdated {
            thread_id: thread_id.to_string(),
            title,
            archived,
        });
        Ok(stored)
    }

    /// Fork a thread from a persisted snapshot.
    pub async fn fork(
        &self,
        source_thread_id: ThreadId,
        snapshot: ForkSnapshot,
        new_thread_id: Option<ThreadId>,
    ) -> ThreadManagerResult<ThreadId> {
        self.store
            .fork_thread(ForkThreadParams {
                source_thread_id,
                snapshot,
                new_thread_id,
            })
            .await
            .map_err(Into::into)
    }
}

fn response_item_to_message(
    thread_id: ThreadId,
    item: &RolloutResponseItem,
) -> ThreadManagerResult<Option<Message>> {
    match item {
        RolloutResponseItem::Message { role, content } => {
            let role = parse_message_role(role).ok_or_else(|| {
                ThreadManagerError::InvalidRolloutItem(thread_id, format!("unknown role: {role}"))
            })?;
            Ok(Some(Message {
                role,
                content: content.clone(),
                tool_calls: None,
                tool_call_id: None,
            }))
        }
        RolloutResponseItem::FunctionCall { name, arguments } => {
            let tool_call = clarity_contract::ToolCall {
                id: format!("call-{}", uuid::Uuid::new_v4()),
                call_type: "function".to_string(),
                function: clarity_contract::FunctionCall {
                    name: name.clone(),
                    arguments: arguments.clone(),
                },
            };
            Ok(Some(Message {
                role: MessageRole::Assistant,
                content: String::new(),
                tool_calls: Some(vec![tool_call]),
                tool_call_id: None,
            }))
        }
        RolloutResponseItem::FunctionCallOutput { call_id, output } => Ok(Some(Message {
            role: MessageRole::Tool,
            content: output.clone(),
            tool_calls: None,
            tool_call_id: Some(call_id.clone()),
        })),
        RolloutResponseItem::Reasoning { content } => Ok(Some(Message {
            role: MessageRole::Assistant,
            content: format!("<thinking>{}</thinking>", content),
            tool_calls: None,
            tool_call_id: None,
        })),
        RolloutResponseItem::Compaction | RolloutResponseItem::ContextCompaction => Ok(None),
        RolloutResponseItem::Other(_) => Ok(None),
    }
}

fn parse_message_role(role: &str) -> Option<MessageRole> {
    match role.to_ascii_lowercase().as_str() {
        "system" => Some(MessageRole::System),
        "user" => Some(MessageRole::User),
        "assistant" => Some(MessageRole::Assistant),
        "tool" => Some(MessageRole::Tool),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[tokio::test]
    async fn test_manager_with_local_store_writes_rollout() {
        let tmp = tempfile::TempDir::new().expect("temp dir");
        let config = clarity_thread_store::RolloutConfig {
            clarity_home: tmp.path().to_path_buf(),
            sqlite_home: tmp.path().to_path_buf(),
            cwd: tmp.path().to_path_buf(),
            model_provider_id: "test".into(),
            generate_memories: false,
        };
        let state_db: PathBuf =
            clarity_thread_store::LocalThreadStore::default_state_db_path(&config);
        let store = tokio::task::spawn_blocking(move || {
            clarity_thread_store::LocalThreadStore::new(config, state_db).map(Arc::new)
        })
        .await
        .expect("spawn_blocking panicked")
        .expect("local thread store creation failed");

        let manager = ThreadManager::new(store);
        let thread_id = manager
            .create_thread(
                tmp.path(),
                "manager-rollout-test",
                Some(SessionSource::Test),
            )
            .await
            .unwrap();

        manager.append_turn(thread_id, "hello", "hi").await.unwrap();

        manager.shutdown(thread_id).await.unwrap();

        let stored = manager.read_thread(thread_id, false).await.unwrap();
        let rollout_path = stored.rollout_path.expect("rollout path set");

        assert!(rollout_path.exists());

        let items = clarity_rollout::load_rollout_items(&rollout_path)
            .await
            .unwrap();

        assert_eq!(items.len(), 4, "expected 4 rollout items, got {items:?}");
        assert!(
            matches!(&items[0], RolloutItem::SessionMeta(meta) if meta.meta.id == thread_id),
            "expected SessionMeta with thread id, got {:?}",
            items[0]
        );
        assert!(
            items.iter().any(|item| matches!(
                item,
                RolloutItem::ResponseItem(RolloutResponseItem::Message { role, content })
                    if role == "user" && content == "hello"
            )),
            "missing user message in rollout"
        );
        assert!(
            items.iter().any(|item| matches!(
                item,
                RolloutItem::ResponseItem(RolloutResponseItem::Message { role, content })
                    if role == "assistant" && content == "hi"
            )),
            "missing assistant message in rollout"
        );
    }

    #[tokio::test]
    async fn test_create_and_load_history() {
        let manager = ThreadManager::new_in_memory();
        let cwd = std::env::current_dir().unwrap_or_else(|_| Path::new(".").to_path_buf());
        let thread_id = manager
            .create_thread(&cwd, "test-originator", SessionSource::Test)
            .await
            .expect("create thread");

        manager
            .append_turn(thread_id, "hello", "hi there")
            .await
            .expect("append turn");

        let history = manager
            .load_llm_history(thread_id)
            .await
            .expect("load history");

        assert_eq!(history.len(), 2);
        assert_eq!(history[0].role, MessageRole::User);
        assert_eq!(history[0].content, "hello");
        assert_eq!(history[1].role, MessageRole::Assistant);
        assert_eq!(history[1].content, "hi there");
    }

    #[tokio::test]
    async fn test_load_history_skips_session_meta() {
        let manager = ThreadManager::new_in_memory();
        let thread_id = manager
            .create_thread(".", "test", None::<SessionSource>)
            .await
            .expect("create thread");

        let history = manager.load_llm_history(thread_id).await.expect("load");
        assert!(history.is_empty());
    }

    #[tokio::test]
    async fn test_create_thread_broadcasts_wire_events() {
        let wire = clarity_wire::Wire::new();
        let mut ui = wire.ui_side(false);
        let manager = ThreadManager::new_in_memory().with_wire(wire);

        let thread_id = manager
            .create_thread(".", "test", Some(SessionSource::Test))
            .await
            .expect("create thread");

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), ui.recv())
            .await
            .expect("no timeout")
            .expect("event");
        assert!(
            matches!(event, clarity_wire::WireMessage::ThreadCreated { thread_id: ref id, .. } if id == &thread_id.to_string()),
            "expected ThreadCreated, got {:?}",
            event
        );

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), ui.recv())
            .await
            .expect("no timeout")
            .expect("event");
        assert!(
            matches!(event, clarity_wire::WireMessage::ThreadActive { thread_id: ref id, .. } if id == &thread_id.to_string()),
            "expected ThreadActive, got {:?}",
            event
        );
    }

    #[tokio::test]
    async fn test_append_turn_broadcasts_thread_updated() {
        let wire = clarity_wire::Wire::new();
        let mut ui = wire.ui_side(false);
        let manager = ThreadManager::new_in_memory().with_wire(wire);

        let thread_id = manager
            .create_thread(".", "test", None::<SessionSource>)
            .await
            .expect("create thread");

        // Drain ThreadCreated + ThreadActive.
        let _ = ui.recv().await;
        let _ = ui.recv().await;

        manager
            .append_turn(thread_id, "hello", "hi")
            .await
            .expect("append turn");

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), ui.recv())
            .await
            .expect("no timeout")
            .expect("event");
        assert!(
            matches!(event, clarity_wire::WireMessage::ThreadUpdated { thread_id: ref id, .. } if id == &thread_id.to_string()),
            "expected ThreadUpdated, got {:?}",
            event
        );
    }

    #[tokio::test]
    async fn test_update_metadata_broadcasts_thread_updated() {
        let wire = clarity_wire::Wire::new();
        let mut ui = wire.ui_side(false);
        let manager = ThreadManager::new_in_memory().with_wire(wire);

        let thread_id = manager
            .create_thread(".", "test", None::<SessionSource>)
            .await
            .expect("create thread");

        // Drain ThreadCreated + ThreadActive.
        let _ = ui.recv().await;
        let _ = ui.recv().await;

        manager
            .update_metadata(
                thread_id,
                clarity_thread_store::ThreadMetadataPatch {
                    title: Some("new title".to_string()),
                    archived: None,
                    extra: std::collections::HashMap::new(),
                },
            )
            .await
            .expect("update metadata");

        let event = tokio::time::timeout(std::time::Duration::from_secs(1), ui.recv())
            .await
            .expect("no timeout")
            .expect("event");
        match event {
            clarity_wire::WireMessage::ThreadUpdated {
                thread_id: ref id,
                title: Some(ref t),
                ..
            } => {
                assert_eq!(id, &thread_id.to_string());
                assert_eq!(t, "new title");
            }
            other => panic!("expected ThreadUpdated with title, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_archive_unarchive_delete() {
        let manager = ThreadManager::new_in_memory();
        let thread_id = manager
            .create_thread(".", "test", None::<SessionSource>)
            .await
            .expect("create thread");

        manager.archive(thread_id).await.expect("archive");
        let archived = manager
            .list_threads(10, true, None)
            .await
            .expect("list archived");
        assert!(archived.iter().any(|t| t.thread_id == thread_id));

        manager.unarchive(thread_id).await.expect("unarchive");
        let active = manager
            .list_threads(10, false, None)
            .await
            .expect("list active");
        assert!(active.iter().any(|t| t.thread_id == thread_id));

        manager.delete(thread_id).await.expect("delete");
        let after = manager
            .list_threads(10, true, None)
            .await
            .expect("list after delete");
        assert!(!after.iter().any(|t| t.thread_id == thread_id));
    }

    #[tokio::test]
    async fn test_fork_thread() {
        let manager = ThreadManager::new_in_memory();
        let thread_id = manager
            .create_thread(".", "test", None::<SessionSource>)
            .await
            .expect("create thread");
        manager
            .append_turn(thread_id, "hello", "hi")
            .await
            .expect("append turn");

        let new_id = manager
            .fork(
                thread_id,
                clarity_thread_store::ForkSnapshot::Interrupted,
                None,
            )
            .await
            .expect("fork");
        assert_ne!(new_id, thread_id);

        let history = manager
            .load_llm_history(new_id)
            .await
            .expect("load fork history");
        assert_eq!(history.len(), 2);
    }

    #[test]
    fn test_builder_methods() {
        let manager = ThreadManager::new_in_memory()
            .with_source(SessionSource::Test)
            .with_cli_version("0.0.0");
        assert_eq!(manager.cli_version, "0.0.0");
        assert_eq!(manager.default_source, SessionSource::Test);
    }
}
