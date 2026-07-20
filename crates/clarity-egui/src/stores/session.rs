//! Session Store
//!
//! session list, active session, drafts

use crate::ui::types::*;
use std::collections::HashMap;

/// Cached `AgentTurn` per aggregated unit index, keyed by session id.
pub type AgentTurnCache = Vec<Option<crate::components::agent_turn::AgentTurn>>;

/// Holds session UI state.
pub struct SessionStore {
    pub sessions: Vec<Session>,
    pub active_session_id: String,
    /// Per-session draft buffer. Key = session_id.
    pub drafts: HashMap<String, String>,
    /// Per-session cache of constructed `AgentTurn`s, keyed by session id.
    /// Cleared for the last (possibly streaming) unit each frame.
    pub turn_cache: HashMap<String, AgentTurnCache>,
}

impl SessionStore {
    /// Returns the active session.
    #[allow(dead_code)]
    pub fn active_session(&self) -> Option<&Session> {
        self.sessions
            .iter()
            .find(|s| s.id == self.active_session_id)
    }

    /// Returns the active session.
    pub fn active_session_mut(&mut self) -> Option<&mut Session> {
        self.sessions
            .iter_mut()
            .find(|s| s.id == self.active_session_id)
    }

    /// Returns a mutable reference to a session by id, regardless of whether it
    /// is currently active. Used by stream handlers so switching sessions does
    /// not redirect chunks into the wrong conversation.
    pub fn session_mut(&mut self, id: &str) -> Option<&mut Session> {
        self.sessions.iter_mut().find(|s| s.id == id)
    }
}
