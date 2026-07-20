//! Jumpy World Model contract.
//!
//! `JumpyState` is a compact, comparable fingerprint of the current session
//! state. `OutcomePredictor` implementations live in `clarity-core::agent::jumpy`
//! and are injected into `clarity-subagents` via trait objects.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A lightweight fingerprint of the current session / workspace state.
///
/// This is the "observation" that the Jumpy World Model predicts over.
/// Unlike raw message histories (which grow unbounded), `JumpyState`
/// is designed to be compact and comparable.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct JumpyState {
    /// Semantic tags describing the current situation
    /// (e.g. ["git-dirty", "build-failed", "tests-passing"]).
    pub tags: Vec<String>,

    /// Key-value working memory produced by previous skills.
    /// Keys are well-known identifiers (e.g. "last_error", "refactored_file").
    pub memory: HashMap<String, String>,

    /// Set of file paths that are currently "active" / modified.
    pub active_files: Vec<String>,

    /// High-level summary of the conversation so far (≤ 200 chars).
    pub context_summary: String,

    /// Estimated progress toward the current goal [0.0, 1.0].
    pub progress: f32,
}

impl JumpyState {
    /// Create a state from a raw user query (initial state).
    pub fn from_query(query: &str) -> Self {
        Self {
            context_summary: query.chars().take(200).collect(),
            ..Default::default()
        }
    }

    /// Compute a simple distance metric between two states.
    /// Returns value in [0, 1] where 0 = identical, 1 = completely different.
    pub fn distance(&self, other: &Self) -> f32 {
        let tag_overlap = if self.tags.is_empty() && other.tags.is_empty() {
            1.0
        } else {
            let union: std::collections::HashSet<_> =
                self.tags.iter().chain(other.tags.iter()).collect();
            let intersection: std::collections::HashSet<_> = self
                .tags
                .iter()
                .filter(|t| other.tags.contains(t))
                .collect();
            intersection.len() as f32 / union.len().max(1) as f32
        };

        let mem_overlap = if self.memory.is_empty() && other.memory.is_empty() {
            1.0
        } else {
            let union_keys: std::collections::HashSet<_> =
                self.memory.keys().chain(other.memory.keys()).collect();
            let matching_keys = self
                .memory
                .keys()
                .filter(|k| other.memory.get(*k) == self.memory.get(*k))
                .count();
            matching_keys as f32 / union_keys.len().max(1) as f32
        };

        let progress_diff = (self.progress - other.progress).abs();

        // Weighted combination: lower score = more similar
        1.0 - (tag_overlap * 0.4 + mem_overlap * 0.4 + (1.0 - progress_diff) * 0.2)
    }

    /// Check if this state satisfies a goal condition (simple tag check).
    pub fn satisfies(&self, goal_tags: &[String]) -> bool {
        goal_tags.iter().all(|g| self.tags.contains(g))
    }

    /// Merge another state's memory into this one (later overwrites earlier).
    pub fn merge(&mut self, other: &Self) {
        for (k, v) in &other.memory {
            self.memory.insert(k.clone(), v.clone());
        }
        for tag in &other.tags {
            if !self.tags.contains(tag) {
                self.tags.push(tag.clone());
            }
        }
        if other.progress > self.progress {
            self.progress = other.progress;
        }
        self.context_summary = other.context_summary.clone();
    }
}

/// Trait for outcome prediction — can be backed by history, LLM, or hybrid.
#[async_trait]
pub trait OutcomePredictor: Send + Sync {
    /// Predict the state after executing `skill_id` with `params` from `current` state.
    /// `commitment` ∈ [0, 1] maps to the RL discount γ:
    ///   - 0.0 = single action, immediate effect only
    ///   - 0.9 = long-horizon, predict end-state after full skill execution
    async fn predict(
        &self,
        skill_id: &str,
        params: &str,
        current: &JumpyState,
        commitment: f32,
    ) -> Result<JumpyState, String>;
}
