//! `clarity-rollout` — JSONL rollout persistence for Clarity threads.
//!
//! This crate writes canonical, append-only JSONL event logs that form the
//! durable replay history of a thread. The concept is inspired by the OpenAI
//! Codex `codex_rollout` crate (Apache-2.0); the implementation is original to
//! Clarity. See `NOTICES.md` for attribution.

#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]

pub mod config;
pub mod policy;
pub mod recorder;

pub use config::{RolloutConfig, RolloutConfigView};
pub use policy::{
    is_persisted_event_msg, is_persisted_response_item, is_persisted_response_item_for_memories,
    is_persisted_rollout_item, persisted_rollout_items,
};
pub use recorder::{RolloutRecorder, load_rollout_items};
