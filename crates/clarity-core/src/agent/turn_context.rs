//! Turn-level mutable state extracted from AgentInner.
//!
//! This struct holds all fields that are reset at the beginning of each turn
//! and do not persist across turns.

use super::enhanced::TokenUsage;
use super::lifecycle::RunState;
use super::loop_detector::LoopDetector;
use clarity_contract::IdentityContext;
use std::collections::HashMap;

#[allow(dead_code)] // team_id, org_id consumed by Phase 7-8
pub(crate) struct TurnContext {
    /// Unique identifier for the current turn (ADR-007).
    pub turn_id: String,
    pub session_usage: TokenUsage,
    pub snapshotted_skill: Option<String>,
    pub recoverable_failure_counts: HashMap<String, u32>,
    pub loop_detector: LoopDetector,
    /// Current lifecycle state of the turn.
    pub run_state: RunState,
    /// User identifier for this turn.
    pub user_id: Option<String>,
    /// Team identifier for this turn.
    pub team_id: Option<String>,
    /// Organization identifier for this turn.
    pub org_id: Option<String>,
}

impl TurnContext {
    /// Create a new `TurnContext`.
    pub fn new(
        turn_id: String,
        active_skill: Option<String>,
        max_repetitions: usize,
        identity: IdentityContext,
    ) -> Self {
        Self {
            turn_id,
            session_usage: TokenUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            snapshotted_skill: active_skill,
            recoverable_failure_counts: HashMap::new(),
            loop_detector: LoopDetector::new(max_repetitions),
            run_state: RunState::Idle,
            user_id: identity.user_id,
            team_id: identity.team_id,
            org_id: identity.org_id,
        }
    }
}
