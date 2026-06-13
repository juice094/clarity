//! SubAgent Store
//!
//! parallel batch progress from Gateway

use crate::ui::types::*;
use std::time::Instant;

/// Holds sub agent UI state.
pub struct SubAgentStore {
    pub parallel_batches: Vec<SubAgentProgress>,
    pub last_parallel_poll: Instant,
    /// Live single-agent progress tracked via channel (IS-1 Sprint 30).
    pub running_agents: std::collections::HashMap<String, SingleSubagentProgress>,
    /// Last Gateway health check poll time.
    pub last_gateway_health_poll: Instant,
    /// Subagent result view modal open state.
    pub subagent_view_modal_open: bool,
    /// ID of the subagent whose output is being viewed.
    pub viewing_subagent_id: Option<String>,
}
