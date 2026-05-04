//! Turn-level mutable state extracted from AgentInner.
//!
//! This struct holds all fields that are reset at the beginning of each turn
//! and do not persist across turns.

use super::enhanced::TokenUsage;
use super::loop_detector::LoopDetector;
use std::collections::HashMap;

pub(crate) struct TurnContext {
    pub session_usage: TokenUsage,
    pub snapshotted_skill: Option<String>,
    pub recoverable_failure_counts: HashMap<String, u32>,
    pub loop_detector: LoopDetector,
}

impl TurnContext {
    pub fn new(active_skill: Option<String>, max_repetitions: usize) -> Self {
        Self {
            session_usage: TokenUsage {
                prompt_tokens: 0,
                completion_tokens: 0,
                total_tokens: 0,
            },
            snapshotted_skill: active_skill,
            recoverable_failure_counts: HashMap::new(),
            loop_detector: LoopDetector::new(max_repetitions),
        }
    }
}
