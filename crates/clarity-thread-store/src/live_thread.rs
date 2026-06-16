//! In-memory representation of a live thread writer.
//!
//! Modeled after `codex_thread_store::LiveThread` from the OpenAI Codex project,
//! licensed under Apache-2.0. See `NOTICES.md` for attribution.

use std::path::PathBuf;
use std::sync::Arc;

use clarity_contract::{SessionId, ThreadId};

/// A handle to a thread that is currently loaded in memory.
///
/// Live threads hold ephemeral writer state and are used by implementations to
/// batch or buffer rollout appends before flushing.
#[derive(Debug, Clone)]
pub struct LiveThread {
    /// Thread identifier.
    pub thread_id: ThreadId,
    /// Session identifier.
    pub session_id: SessionId,
    /// Path to the durable rollout file, if any.
    pub rollout_path: Option<PathBuf>,
}

/// RAII guard returned while a live thread is being initialized.
///
/// Dropping the guard without committing signals that initialization failed and
/// any partially-created durable state should be discarded.
pub struct LiveThreadInitGuard {
    /// Thread being initialized.
    pub thread_id: ThreadId,
    /// Path to any partially-created rollout file.
    pub rollout_path: Option<PathBuf>,
    /// Whether initialization was committed.
    committed: bool,
}

impl LiveThreadInitGuard {
    /// Create a new initialization guard.
    pub fn new(thread_id: ThreadId, rollout_path: Option<PathBuf>) -> Self {
        Self {
            thread_id,
            rollout_path,
            committed: false,
        }
    }

    /// Mark initialization as committed.
    pub fn commit(mut self) {
        self.committed = true;
    }
}

impl Drop for LiveThreadInitGuard {
    fn drop(&mut self) {
        if !self.committed {
            // Implementations may hook this to remove partial rollout files.
            // The default behavior is to leave already-durable data in place.
        }
    }
}

/// Shared owner for a live thread handle.
pub type SharedLiveThread = Arc<LiveThread>;
