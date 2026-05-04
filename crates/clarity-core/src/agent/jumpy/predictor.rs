// SPDX-License-Identifier: AGPL-3.0
// Copyright (C) 2026 juice094 and contributors

//! Skill Outcome Predictor — The "Jumpy World Model" for workflow orchestration.
//!
//! Predicts the distribution (or point estimate) of `JumpyState` after executing
//! a parameterized skill, without actually running it.
//!
//! Analogous to the GHM (Geometric Horizon Model) in RL:
//!   m^π_γ(· | s, a)  →  predict(skill_id, params, current_state) → predicted_state
//!
//! Two modes of operation:
//! 1. **Historical** — lookup from past executions (offline learning).
//! 2. **LLM-augmented** — when no history exists, ask the LLM to simulate the outcome.

use super::state::JumpyState;
use std::collections::HashMap;

/// A single observed transition: (skill, params, before, after).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillObservation {
    pub skill_id: String,
    pub params: String, // JSON or human-readable parameterization
    pub before: JumpyState,
    pub after: JumpyState,
}

/// Trait for outcome prediction — can be backed by history, LLM, or hybrid.
#[async_trait::async_trait]
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

/// Simple historical predictor that learns from observed transitions.
///
/// No neural networks, no flow matching — pure nearest-neighbor over a
/// compact state embedding. This is the "MVP" world model.
pub struct HistoricalPredictor {
    /// Observations grouped by (skill_id, params) key.
    observations: HashMap<String, Vec<SkillObservation>>,
    /// Minimum similarity threshold to trust a historical match [0, 1].
    similarity_threshold: f32,
}

impl Default for HistoricalPredictor {
    fn default() -> Self {
        Self::new()
    }
}

impl HistoricalPredictor {
    pub fn new() -> Self {
        Self {
            observations: HashMap::new(),
            similarity_threshold: 0.3,
        }
    }

    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.similarity_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Ingest a new observation (offline learning).
    pub fn observe(&mut self, obs: SkillObservation) {
        let key = format!("{}:{}", obs.skill_id, obs.params);
        self.observations.entry(key).or_default().push(obs);
    }

    /// Batch ingest from a session log.
    pub fn observe_batch(&mut self, observations: Vec<SkillObservation>) {
        for obs in observations {
            self.observe(obs);
        }
    }

    /// Find the k-nearest historical observations for the given query state.
    fn nearest_neighbors(
        &self,
        skill_id: &str,
        params: &str,
        current: &JumpyState,
        k: usize,
    ) -> Vec<(f32, &SkillObservation)> {
        let key = format!("{}:{}", skill_id, params);
        let candidates = match self.observations.get(&key) {
            Some(v) => v,
            None => return Vec::new(),
        };

        let mut scored: Vec<(f32, &SkillObservation)> = candidates
            .iter()
            .map(|obs| (current.distance(&obs.before), obs))
            .collect();

        scored.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.into_iter().take(k).collect()
    }
}

#[async_trait::async_trait]
impl OutcomePredictor for HistoricalPredictor {
    async fn predict(
        &self,
        skill_id: &str,
        params: &str,
        current: &JumpyState,
        _commitment: f32,
    ) -> Result<JumpyState, String> {
        let neighbors = self.nearest_neighbors(skill_id, params, current, 3);

        if neighbors.is_empty() {
            return Err(format!(
                "No historical observations for skill '{}:{}'",
                skill_id, params
            ));
        }

        // If best match is too dissimilar, don't trust it.
        let (best_dist, _) = neighbors[0];
        if best_dist > self.similarity_threshold {
            return Err(format!(
                "Best historical match distance {} exceeds threshold {} for skill '{}:{}'",
                best_dist, self.similarity_threshold, skill_id, params
            ));
        }

        // Weighted average of neighbor outcomes (inverse distance weighting).
        let mut merged = JumpyState::default();
        let mut total_weight = 0.0f32;

        for (dist, obs) in &neighbors {
            let weight = 1.0 / (dist + 0.01);
            total_weight += weight;

            // Merge tags (union)
            for tag in &obs.after.tags {
                if !merged.tags.contains(tag) {
                    merged.tags.push(tag.clone());
                }
            }
            // Merge memory (later overwrites)
            for (k, v) in &obs.after.memory {
                merged.memory.insert(k.clone(), v.clone());
            }
            // Weighted progress
            merged.progress += obs.after.progress * weight;
        }

        merged.progress /= total_weight;
        merged.context_summary = neighbors[0].1.after.context_summary.clone();
        merged.active_files = neighbors[0].1.after.active_files.clone();

        Ok(merged)
    }
}

/// Horizon Consistency wrapper — enforces that predictions at different
/// commitment levels are coherent.
///
/// Implements the key insight from the paper:
///   long-horizon prediction should be reachable by chaining short-horizon ones.
pub struct ConsistentPredictor<P: OutcomePredictor> {
    inner: P,
    /// Short-horizon commitment level (e.g. 0.5)
    short_commitment: f32,
    /// Long-horizon commitment level (e.g. 0.9)
    long_commitment: f32,
}

impl<P: OutcomePredictor> ConsistentPredictor<P> {
    pub fn new(inner: P) -> Self {
        Self {
            inner,
            short_commitment: 0.5,
            long_commitment: 0.9,
        }
    }

    pub fn with_horizons(mut self, short: f32, long: f32) -> Self {
        self.short_commitment = short.clamp(0.0, 1.0);
        self.long_commitment = long.clamp(0.0, 1.0);
        self
    }

    /// Verify consistency: predict long directly vs chain two shorts.
    /// Returns the inconsistency score (lower = more consistent).
    pub async fn check_consistency(
        &self,
        skill_id: &str,
        params: &str,
        current: &JumpyState,
    ) -> Result<f32, String> {
        let direct_long = self
            .inner
            .predict(skill_id, params, current, self.long_commitment)
            .await?;

        let mid = self
            .inner
            .predict(skill_id, params, current, self.short_commitment)
            .await?;
        let chained_long = self
            .inner
            .predict(skill_id, params, &mid, self.short_commitment)
            .await?;

        Ok(direct_long.distance(&chained_long))
    }
}

#[async_trait::async_trait]
impl<P: OutcomePredictor> OutcomePredictor for ConsistentPredictor<P> {
    async fn predict(
        &self,
        skill_id: &str,
        params: &str,
        current: &JumpyState,
        commitment: f32,
    ) -> Result<JumpyState, String> {
        // If requesting long horizon, first try short then extend.
        // This "bootstraps" from more reliable short-horizon predictions.
        if commitment >= self.long_commitment {
            let short = self
                .inner
                .predict(skill_id, params, current, self.short_commitment)
                .await?;
            self.inner
                .predict(skill_id, params, &short, commitment)
                .await
        } else {
            self.inner
                .predict(skill_id, params, current, commitment)
                .await
        }
    }
}
