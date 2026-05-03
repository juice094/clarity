//! Agent Loop orchestration for the Claw runtime.
//!
//! Phase 1: Skeleton only. The actual agent loop lives in `clarity-core`.
//! This module will eventually wrap it with federal capabilities
//! (multi-agent dispatch, SOP templates, cron triggers).

use clarity_contract::AgentError;

/// Placeholder for a federal agent session.
///
/// In Phase 1 this is a no-op struct. Phase 2+ will hold the
/// `clarity_core::Agent` reference and coordinate turn execution.
pub struct FederalAgentSession {
    session_id: String,
}

impl FederalAgentSession {
    /// Create a new session placeholder.
    pub fn new(session_id: impl Into<String>) -> Self {
        Self {
            session_id: session_id.into(),
        }
    }

    /// Run a single turn (placeholder).
    ///
    /// Phase 1: returns an error indicating not yet implemented.
    /// Phase 2: will delegate to `clarity_core::Agent::run()`.
    pub async fn run_turn(&self, _input: &str) -> Result<String, AgentError> {
        tracing::warn!(
            session_id = %self.session_id,
            "FederalAgentSession::run_turn called — not yet implemented in Phase 1"
        );
        Err(AgentError::registry(
            "Federal agent turn not yet implemented (Phase 1 skeleton)",
        ))
    }

    /// Session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}
