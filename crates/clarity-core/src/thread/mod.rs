//! Thread lifecycle management for Clarity.
//!
//! Provides [`ThreadManager`], a high-level orchestrator over the storage-neutral
//! `ThreadStore` trait from `clarity-thread-store`.

pub mod manager;

pub use manager::{ThreadManager, ThreadManagerError};
