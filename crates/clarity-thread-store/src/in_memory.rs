//! In-memory implementation of [`ThreadStore`] for tests and lightweight deployments.
//!
//! Modeled after `codex_thread_store::InMemoryThreadStore` from the OpenAI Codex
//! project, licensed under Apache-2.0. See `NOTICES.md` for attribution.

use std::any::Any;
use std::collections::HashMap;

use chrono::Utc;
use clarity_contract::{RolloutEventMsg, RolloutItem, RolloutResponseItem, SessionId, ThreadId};
use parking_lot::RwLock;

use crate::error::ThreadStoreError;
use crate::store::{ThreadStore, ThreadStoreFuture};
use crate::types::{
    AppendThreadItemsParams, ArchiveThreadParams, CreateThreadParams, DeleteThreadParams,
    ForkSnapshot, ForkThreadParams, ListThreadsParams, LoadThreadHistoryParams, ReadThreadParams,
    ResumeThreadParams, StoredThread, StoredThreadHistory, ThreadPage, ThreadSummary,
    UpdateThreadMetadataParams,
};

/// In-memory thread record.
#[derive(Debug, Clone)]
struct InMemoryThread {
    session_id: SessionId,
    items: Vec<RolloutItem>,
    title: Option<String>,
    archived: bool,
    created_at: chrono::DateTime<Utc>,
    updated_at: chrono::DateTime<Utc>,
    parent_thread_id: Option<ThreadId>,
    forked_from_id: Option<ThreadId>,
}

impl InMemoryThread {
    fn summary(&self, thread_id: ThreadId) -> ThreadSummary {
        ThreadSummary {
            thread_id,
            session_id: self.session_id,
            title: self.title.clone(),
            created_at: self.created_at,
            updated_at: self.updated_at,
            archived: self.archived,
            parent_thread_id: self.parent_thread_id,
            forked_from_id: self.forked_from_id,
        }
    }

    fn stored(&self, thread_id: ThreadId, include_history: bool) -> StoredThread {
        StoredThread {
            thread_id,
            session_id: self.session_id,
            title: self.title.clone(),
            rollout_path: None,
            created_at: self.created_at,
            updated_at: self.updated_at,
            archived: self.archived,
            parent_thread_id: self.parent_thread_id,
            forked_from_id: self.forked_from_id,
            history: include_history.then(|| StoredThreadHistory {
                items: self.items.clone(),
            }),
        }
    }
}

/// In-memory [`ThreadStore`] implementation.
#[derive(Debug, Default)]
pub struct InMemoryThreadStore {
    threads: RwLock<HashMap<ThreadId, InMemoryThread>>,
}

impl InMemoryThreadStore {
    /// Create a new empty in-memory store.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ThreadStore for InMemoryThreadStore {
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn create_thread(&self, params: CreateThreadParams) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let mut threads = self.threads.write();
            if threads.contains_key(&params.thread_id) {
                return Err(ThreadStoreError::duplicate(params.thread_id.to_string()));
            }
            let now = Utc::now();
            threads.insert(
                params.thread_id,
                InMemoryThread {
                    session_id: params.session_id,
                    items: Vec::new(),
                    title: None,
                    archived: false,
                    created_at: now,
                    updated_at: now,
                    parent_thread_id: params.parent_thread_id,
                    forked_from_id: params.forked_from_id,
                },
            );
            Ok(())
        })
    }

    fn resume_thread(&self, params: ResumeThreadParams) -> ThreadStoreFuture<'_, StoredThread> {
        Box::pin(async move {
            let threads = self.threads.read();
            let thread = threads
                .get(&params.thread_id)
                .ok_or_else(|| ThreadStoreError::not_found(params.thread_id.to_string()))?;
            Ok(thread.stored(params.thread_id, false))
        })
    }

    fn append_items(&self, params: AppendThreadItemsParams) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let mut threads = self.threads.write();
            let thread = threads
                .get_mut(&params.thread_id)
                .ok_or_else(|| ThreadStoreError::not_found(params.thread_id.to_string()))?;
            thread.items.extend(params.items);
            thread.updated_at = Utc::now();
            Ok(())
        })
    }

    fn persist_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let _ = self
                .threads
                .read()
                .get(&thread_id)
                .ok_or_else(|| ThreadStoreError::not_found(thread_id.to_string()))?;
            Ok(())
        })
    }

    fn flush_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let _ = self
                .threads
                .read()
                .get(&thread_id)
                .ok_or_else(|| ThreadStoreError::not_found(thread_id.to_string()))?;
            Ok(())
        })
    }

    fn shutdown_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move { self.flush_thread(thread_id).await })
    }

    fn discard_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let _ = self
                .threads
                .read()
                .get(&thread_id)
                .ok_or_else(|| ThreadStoreError::not_found(thread_id.to_string()))?;
            Ok(())
        })
    }

    fn load_history(
        &self,
        params: LoadThreadHistoryParams,
    ) -> ThreadStoreFuture<'_, StoredThreadHistory> {
        Box::pin(async move {
            let threads = self.threads.read();
            let thread = threads
                .get(&params.thread_id)
                .ok_or_else(|| ThreadStoreError::not_found(params.thread_id.to_string()))?;
            let items: Vec<RolloutItem> = if params.include_compacted {
                thread.items.clone()
            } else {
                thread
                    .items
                    .iter()
                    .filter(|item| !matches!(item, RolloutItem::Compacted(_)))
                    .cloned()
                    .collect()
            };
            let items = if let Some(before_turn) = params.before_turn {
                // Count user-message boundaries and drop items at or after the nth.
                let mut user_count = 0;
                items
                    .into_iter()
                    .take_while(|item| {
                        if matches!(item, RolloutItem::EventMsg(RolloutEventMsg::UserMessage(_))) {
                            user_count += 1;
                        }
                        user_count < before_turn
                    })
                    .collect()
            } else {
                items
            };
            Ok(StoredThreadHistory { items })
        })
    }

    fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreFuture<'_, StoredThread> {
        Box::pin(async move {
            let threads = self.threads.read();
            let thread = threads
                .get(&params.thread_id)
                .ok_or_else(|| ThreadStoreError::not_found(params.thread_id.to_string()))?;
            Ok(thread.stored(params.thread_id, params.include_history))
        })
    }

    fn list_threads(&self, params: ListThreadsParams) -> ThreadStoreFuture<'_, ThreadPage> {
        Box::pin(async move {
            let threads = self.threads.read();
            let mut data: Vec<ThreadSummary> = threads
                .iter()
                .filter(|(_, t)| params.include_archived || !t.archived)
                .map(|(id, t)| t.summary(*id))
                .collect();
            data.sort_by_key(|item| std::cmp::Reverse(item.updated_at));
            let data = data.into_iter().take(params.limit).collect();
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
            let mut threads = self.threads.write();
            let thread = threads
                .get_mut(&params.thread_id)
                .ok_or_else(|| ThreadStoreError::not_found(params.thread_id.to_string()))?;
            if let Some(title) = params.patch.title {
                thread.title = Some(title);
            }
            if let Some(archived) = params.patch.archived {
                thread.archived = archived;
            }
            thread.updated_at = Utc::now();
            Ok(thread.stored(params.thread_id, false))
        })
    }

    fn archive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let mut threads = self.threads.write();
            let thread = threads
                .get_mut(&params.thread_id)
                .ok_or_else(|| ThreadStoreError::not_found(params.thread_id.to_string()))?;
            thread.archived = true;
            thread.updated_at = Utc::now();
            Ok(())
        })
    }

    fn unarchive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreFuture<'_, StoredThread> {
        Box::pin(async move {
            let mut threads = self.threads.write();
            let thread = threads
                .get_mut(&params.thread_id)
                .ok_or_else(|| ThreadStoreError::not_found(params.thread_id.to_string()))?;
            thread.archived = false;
            thread.updated_at = Utc::now();
            Ok(thread.stored(params.thread_id, false))
        })
    }

    fn delete_thread(&self, params: DeleteThreadParams) -> ThreadStoreFuture<'_, ()> {
        Box::pin(async move {
            let mut threads = self.threads.write();
            threads
                .remove(&params.thread_id)
                .ok_or_else(|| ThreadStoreError::not_found(params.thread_id.to_string()))?;
            Ok(())
        })
    }

    fn fork_thread(&self, params: ForkThreadParams) -> ThreadStoreFuture<'_, ThreadId> {
        Box::pin(async move {
            let source_items = {
                let threads = self.threads.read();
                let source = threads.get(&params.source_thread_id).ok_or_else(|| {
                    ThreadStoreError::not_found(params.source_thread_id.to_string())
                })?;
                source.items.clone()
            };

            let new_items: Vec<RolloutItem> = match params.snapshot {
                ForkSnapshot::TruncateBeforeNthUserMessage(n) => {
                    let mut user_count = 0;
                    source_items
                        .into_iter()
                        .take_while(|item| {
                            if is_user_message_item(item) {
                                user_count += 1;
                            }
                            user_count < n
                        })
                        .collect()
                }
                ForkSnapshot::Interrupted => source_items,
            };

            let new_thread_id = params.new_thread_id.unwrap_or_default();
            let now = Utc::now();
            let session_id = {
                let threads = self.threads.read();
                threads
                    .get(&params.source_thread_id)
                    .map(|t| t.session_id)
                    .unwrap_or_default()
            };
            let mut threads = self.threads.write();
            threads.insert(
                new_thread_id,
                InMemoryThread {
                    session_id,
                    items: new_items,
                    title: None,
                    archived: false,
                    created_at: now,
                    updated_at: now,
                    parent_thread_id: Some(params.source_thread_id),
                    forked_from_id: Some(params.source_thread_id),
                },
            );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ThreadMetadataPatch;
    use clarity_contract::{RolloutResponseItem, SessionSource};

    #[tokio::test]
    async fn create_and_list_thread() {
        let store = InMemoryThreadStore::new();
        let params = CreateThreadParams {
            thread_id: ThreadId::new(),
            session_id: SessionId::new(),
            source: SessionSource::Cli,
            ..Default::default()
        };
        store.create_thread(params.clone()).await.unwrap();
        let page = store
            .list_threads(ListThreadsParams::default())
            .await
            .unwrap();
        assert_eq!(page.data.len(), 1);
        assert_eq!(page.data[0].thread_id, params.thread_id);
    }

    #[tokio::test]
    async fn append_and_read_history() {
        let store = InMemoryThreadStore::new();
        let thread_id = ThreadId::new();
        let session_id = SessionId::new();
        store
            .create_thread(CreateThreadParams {
                thread_id,
                session_id,
                source: SessionSource::Cli,
                ..Default::default()
            })
            .await
            .unwrap();
        store
            .append_items(AppendThreadItemsParams {
                thread_id,
                items: vec![RolloutItem::ResponseItem(RolloutResponseItem::Message {
                    role: "user".into(),
                    content: "hello".into(),
                })],
            })
            .await
            .unwrap();
        let history = store
            .load_history(LoadThreadHistoryParams {
                thread_id,
                include_compacted: true,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(history.items.len(), 1);
    }

    #[tokio::test]
    async fn duplicate_create_fails() {
        let store = InMemoryThreadStore::new();
        let params = CreateThreadParams {
            thread_id: ThreadId::new(),
            session_id: SessionId::new(),
            source: SessionSource::Cli,
            ..Default::default()
        };
        store.create_thread(params.clone()).await.unwrap();
        let err = store.create_thread(params.clone()).await.unwrap_err();
        assert!(
            matches!(err, ThreadStoreError::Duplicate { ref thread_id } if thread_id == &params.thread_id.to_string()),
            "expected Duplicate error, got {err:?}"
        );
    }

    #[tokio::test]
    async fn resume_and_read_thread() {
        let store = InMemoryThreadStore::new();
        let thread_id = ThreadId::new();
        let session_id = SessionId::new();
        store
            .create_thread(CreateThreadParams {
                thread_id,
                session_id,
                source: SessionSource::Cli,
                ..Default::default()
            })
            .await
            .unwrap();

        let resumed = store
            .resume_thread(ResumeThreadParams {
                thread_id,
                session_id,
            })
            .await
            .unwrap();
        assert_eq!(resumed.thread_id, thread_id);
        assert!(resumed.history.is_none());

        store
            .append_items(AppendThreadItemsParams {
                thread_id,
                items: vec![RolloutItem::ResponseItem(RolloutResponseItem::Message {
                    role: "user".into(),
                    content: "hello again".into(),
                })],
            })
            .await
            .unwrap();

        let read = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_history: true,
            })
            .await
            .unwrap();
        assert_eq!(read.history.unwrap().items.len(), 1);
    }

    #[tokio::test]
    async fn update_metadata_and_archive() {
        let store = InMemoryThreadStore::new();
        let thread_id = ThreadId::new();
        let session_id = SessionId::new();
        store
            .create_thread(CreateThreadParams {
                thread_id,
                session_id,
                source: SessionSource::Cli,
                ..Default::default()
            })
            .await
            .unwrap();

        let updated = store
            .update_thread_metadata(UpdateThreadMetadataParams {
                thread_id,
                patch: ThreadMetadataPatch {
                    title: Some("my title".into()),
                    archived: Some(true),
                    ..Default::default()
                },
            })
            .await
            .unwrap();
        assert_eq!(updated.title.as_deref(), Some("my title"));
        assert!(updated.archived);

        let listed = store
            .list_threads(ListThreadsParams {
                include_archived: false,
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(listed.data.is_empty());

        let unarchived = store
            .unarchive_thread(ArchiveThreadParams { thread_id })
            .await
            .unwrap();
        assert!(!unarchived.archived);
    }

    #[tokio::test]
    async fn delete_thread_removes_it() {
        let store = InMemoryThreadStore::new();
        let thread_id = ThreadId::new();
        let session_id = SessionId::new();
        store
            .create_thread(CreateThreadParams {
                thread_id,
                session_id,
                source: SessionSource::Cli,
                ..Default::default()
            })
            .await
            .unwrap();

        store
            .delete_thread(DeleteThreadParams { thread_id })
            .await
            .unwrap();
        let err = store
            .read_thread(ReadThreadParams {
                thread_id,
                include_history: false,
            })
            .await
            .unwrap_err();
        assert!(
            matches!(err, ThreadStoreError::NotFound { .. }),
            "expected NotFound error, got {err:?}"
        );
    }

    #[tokio::test]
    async fn fork_thread_interrupt_and_truncate() {
        let store = InMemoryThreadStore::new();
        let source_thread_id = ThreadId::new();
        let session_id = SessionId::new();
        store
            .create_thread(CreateThreadParams {
                thread_id: source_thread_id,
                session_id,
                source: SessionSource::Cli,
                ..Default::default()
            })
            .await
            .unwrap();

        let items: Vec<RolloutItem> = (0..3)
            .flat_map(|i| {
                vec![
                    RolloutItem::EventMsg(RolloutEventMsg::UserMessage(format!("user-{i}"))),
                    RolloutItem::ResponseItem(RolloutResponseItem::Message {
                        role: "assistant".into(),
                        content: format!("assistant-{i}"),
                    }),
                ]
            })
            .collect();
        store
            .append_items(AppendThreadItemsParams {
                thread_id: source_thread_id,
                items,
            })
            .await
            .unwrap();

        let interrupted_fork_id = ThreadId::new();
        store
            .fork_thread(ForkThreadParams {
                source_thread_id,
                snapshot: ForkSnapshot::Interrupted,
                new_thread_id: Some(interrupted_fork_id),
            })
            .await
            .unwrap();
        let interrupted = store
            .read_thread(ReadThreadParams {
                thread_id: interrupted_fork_id,
                include_history: true,
            })
            .await
            .unwrap();
        assert_eq!(interrupted.history.unwrap().items.len(), 6);

        let truncated_fork_id = ThreadId::new();
        store
            .fork_thread(ForkThreadParams {
                source_thread_id,
                snapshot: ForkSnapshot::TruncateBeforeNthUserMessage(2),
                new_thread_id: Some(truncated_fork_id),
            })
            .await
            .unwrap();
        let truncated = store
            .read_thread(ReadThreadParams {
                thread_id: truncated_fork_id,
                include_history: true,
            })
            .await
            .unwrap();
        assert_eq!(truncated.history.unwrap().items.len(), 2);
    }

    #[tokio::test]
    async fn load_history_filters_compacted_and_before_turn() {
        let store = InMemoryThreadStore::new();
        let thread_id = ThreadId::new();
        let session_id = SessionId::new();
        store
            .create_thread(CreateThreadParams {
                thread_id,
                session_id,
                source: SessionSource::Cli,
                ..Default::default()
            })
            .await
            .unwrap();

        store
            .append_items(AppendThreadItemsParams {
                thread_id,
                items: vec![
                    RolloutItem::EventMsg(RolloutEventMsg::UserMessage("first".into())),
                    RolloutItem::Compacted(clarity_contract::CompactedItem {
                        message: "compacted summary".into(),
                        replacement_history: None,
                        window_id: None,
                    }),
                    RolloutItem::EventMsg(RolloutEventMsg::UserMessage("second".into())),
                ],
            })
            .await
            .unwrap();

        let with_compacted = store
            .load_history(LoadThreadHistoryParams {
                thread_id,
                include_compacted: true,
                before_turn: None,
            })
            .await
            .unwrap();
        assert_eq!(with_compacted.items.len(), 3);

        let without_compacted = store
            .load_history(LoadThreadHistoryParams {
                thread_id,
                include_compacted: false,
                before_turn: None,
            })
            .await
            .unwrap();
        assert_eq!(without_compacted.items.len(), 2);

        let before_turn = store
            .load_history(LoadThreadHistoryParams {
                thread_id,
                include_compacted: true,
                before_turn: Some(2),
            })
            .await
            .unwrap();
        assert_eq!(before_turn.items.len(), 2);
    }
}
