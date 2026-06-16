//! Storage-neutral thread persistence boundary.
//!
//! Modeled after `codex_thread_store::ThreadStore` from the OpenAI Codex
//! project, licensed under Apache-2.0. See `NOTICES.md` for attribution.

use std::any::Any;
use std::future::Future;
use std::pin::Pin;

use crate::error::ThreadStoreResult;
use crate::types::{
    AppendThreadItemsParams, ArchiveThreadParams, CreateThreadParams, DeleteThreadParams,
    ForkThreadParams, ListThreadsParams, LoadThreadHistoryParams, ReadThreadParams,
    ResumeThreadParams, StoredThread, StoredThreadHistory, ThreadPage, UpdateThreadMetadataParams,
};
use clarity_contract::ThreadId;

/// Future returned by [`ThreadStore`] operations.
pub type ThreadStoreFuture<'a, T> = Pin<Box<dyn Future<Output = ThreadStoreResult<T>> + Send + 'a>>;

/// Storage-neutral thread persistence boundary.
///
/// Implementations are responsible for durable replay history (typically JSONL
/// rollouts) and queryable metadata (typically SQLite). The trait is designed
/// to be object-safe so that `clarity-core` can hold `Arc<dyn ThreadStore>`.
pub trait ThreadStore: Any + Send + Sync {
    /// Return this store as [`Any`] for implementation-owned escape hatches.
    fn as_any(&self) -> &dyn Any;

    /// Creates a new thread and its durable history.
    fn create_thread(&self, params: CreateThreadParams) -> ThreadStoreFuture<'_, ()>;

    /// Reopens an existing thread for live appends.
    fn resume_thread(&self, params: ResumeThreadParams) -> ThreadStoreFuture<'_, StoredThread>;

    /// Appends rollout items to a live thread.
    fn append_items(&self, params: AppendThreadItemsParams) -> ThreadStoreFuture<'_, ()>;

    /// Materializes the thread if persistence is lazy, then persists all queued items.
    fn persist_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()>;

    /// Flushes all queued items and returns once they are durable/readable.
    fn flush_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()>;

    /// Flushes pending items and closes the live thread writer.
    fn shutdown_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()>;

    /// Discards the live thread writer without forcing pending in-memory items to become durable.
    ///
    /// Core calls this when session initialization fails after a live writer has
    /// been created. Implementations should release any live writer resources
    /// while preserving already-durable thread data.
    fn discard_thread(&self, thread_id: ThreadId) -> ThreadStoreFuture<'_, ()>;

    /// Loads persisted history for resume, fork, rollback, and memory jobs.
    fn load_history(
        &self,
        params: LoadThreadHistoryParams,
    ) -> ThreadStoreFuture<'_, StoredThreadHistory>;

    /// Reads a thread summary and optionally its persisted history.
    fn read_thread(&self, params: ReadThreadParams) -> ThreadStoreFuture<'_, StoredThread>;

    /// Lists stored threads matching the supplied filters.
    fn list_threads(&self, params: ListThreadsParams) -> ThreadStoreFuture<'_, ThreadPage>;

    /// Updates thread metadata and returns the updated thread summary.
    fn update_thread_metadata(
        &self,
        params: UpdateThreadMetadataParams,
    ) -> ThreadStoreFuture<'_, StoredThread>;

    /// Archives a thread.
    fn archive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreFuture<'_, ()>;

    /// Unarchives a thread and returns its updated metadata.
    fn unarchive_thread(&self, params: ArchiveThreadParams) -> ThreadStoreFuture<'_, StoredThread>;

    /// Deletes a thread's persisted data and associated metadata.
    fn delete_thread(&self, params: DeleteThreadParams) -> ThreadStoreFuture<'_, ()>;

    /// Forks a thread from a persisted snapshot.
    fn fork_thread(&self, params: ForkThreadParams) -> ThreadStoreFuture<'_, ThreadId>;
}
