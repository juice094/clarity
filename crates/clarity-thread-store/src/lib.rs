//! `clarity-thread-store` — Thread persistence abstraction for Clarity.
//!
//! This crate provides a storage-neutral `ThreadStore` trait and several
//! implementations. The design is ported from the OpenAI Codex
//! `codex_thread_store` crate and adapted for Clarity. Codex is licensed under
//! Apache-2.0; see `NOTICES.md` for attribution.
//!
//! ## Crate topology
//!
//! `clarity-contract` ← `clarity-thread-store` → used by `clarity-core`,
//! `clarity-memory`, `clarity-gateway`, and `clarity-claw`.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod error;
pub mod in_memory;
pub mod live_thread;
pub mod local;
pub mod store;
pub mod types;

pub use clarity_rollout::{RolloutConfig, RolloutConfigView};
pub use error::{ThreadStoreError, ThreadStoreResult};
pub use in_memory::InMemoryThreadStore;
pub use live_thread::{LiveThread, LiveThreadInitGuard, SharedLiveThread};
pub use local::LocalThreadStore;
pub use store::{ThreadStore, ThreadStoreFuture};
pub use types::{
    AppendThreadItemsParams, ArchiveThreadParams, CreateThreadParams, DeleteThreadParams,
    ForkSnapshot, ForkThreadParams, ListThreadsParams, LoadThreadHistoryParams, ReadThreadParams,
    ResumeThreadParams, StoredThread, StoredThreadHistory, ThreadMetadataPatch, ThreadPage,
    ThreadSummary, UpdateThreadMetadataParams,
};
