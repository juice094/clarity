// SPDX-License-Identifier: AGPL-3.0
// Copyright (C) 2026 juice094 and contributors

//! Hierarchical Planner — Compositional planning over skills.
//!
//! Translates the CompPlan algorithm from the Jumpy World Models paper
//! into the workflow domain:
//!   - "Policies"      → Skills (parameterized by intent / subgoal)
//!   - "GHM sampling"  → OutcomePredictor::predict()
//!   - "Random shooting" → Generate candidate skill sequences, evaluate, pick best
//!   - "Value estimator" → Distance(predicted_final_state, goal_state)

use super::predictor::OutcomePredictor;
use super::state::JumpyState;
use rand::SeedableRng;
use std::sync::Arc;

/// A goal specification for planning.
#[derive(Debug, Clone)]
pub struct Goal {
    /// Desired state tags (all must be present).
    pub required_tags: Vec<String>,
    /// Optional: specific memory keys that should be populated.
    pub required_memory_keys: Vec<String>,
    /// Target progress [0.0, 1.0].
    pub target_progress: f32,
}

impl Goal {
    pub fn new(required_tags: Vec<String>) -> Self {
        Self {
            required_tags,
            required_memory_keys: Vec::new(),
            target_progress: 1.0,
        }
    }

    /// Compute a "reward" for a given state: higher = closer to goal.
    pub fn evaluate(&self, state: &JumpyState) -> f32 {
        let mut score = 0.0f32;

        // Tag coverage
        if !self.required_tags.is_empty() {
            let covered = self
                .required_tags
                .iter()
                .filter(|t| state.tags.contains(t))
                .count();
            score += (covered as f32 / self.required_tags.len() as f32) * 0.5;
        } else {
            score += 0.5;
        }

        // Memory coverage
        if !self.required_memory_keys.is_empty() {
            let covered = self
                .required_memory_keys
                .iter()
                .filter(|k| state.memory.contains_key(*k))
                .count();
            score += (covered as f32 / self.required_memory_keys.len() as f32) * 0.3;
        } else {
            score += 0.3;
        }

        // Progress alignment
        let progress_bonus = 1.0 - (state.progress - self.target_progress).abs();
        score += progress_bonus * 0.2;

        score
    }
}

/// A single skill invocation in a planned sequence.
#[derive(Debug, Clone)]
pub struct SkillProposal {
    pub skill_id: String,
    /// Parameters / subgoal description (the "z" in π_z).
    pub params: String,
    /// Commitment level ∈ [0, 1] — how long to let this skill run before reconsidering.
    /// Maps to the RL "switching probability" α:  commitment = 1 - α.
    pub commitment: f32,
}

/// A sequence of skills (a "policy composition").
pub type SkillSequence = Vec<SkillProposal>;

/// Planner configuration.
#[derive(Debug, Clone)]
pub struct PlannerConfig {
    /// Number of candidate sequences to generate (random shooting samples).
    pub num_candidates: usize,
    /// Max length of a sequence.
    pub max_sequence_len: usize,
    /// Available skills (skill_id, default_params).
    pub available_skills: Vec<(String, String)>,
    /// Optional RNG seed for deterministic planning (useful in tests).
    pub rng_seed: Option<u64>,
}

impl Default for PlannerConfig {
    fn default() -> Self {
        Self {
            num_candidates: 16,
            max_sequence_len: 5,
            available_skills: Vec::new(),
            rng_seed: None,
        }
    }
}

/// The hierarchical planner.
pub struct HierarchicalPlanner<P: OutcomePredictor + ?Sized> {
    predictor: Arc<P>,
    config: PlannerConfig,
    rng: rand::rngs::StdRng,
}

impl<P: OutcomePredictor + ?Sized> HierarchicalPlanner<P> {
    pub fn new(predictor: Arc<P>, config: PlannerConfig) -> Self {
        let rng = match config.rng_seed {
            Some(seed) => rand::rngs::StdRng::seed_from_u64(seed),
            None => rand::rngs::StdRng::from_rng(&mut rand::rng()),
        };
        Self {
            predictor,
            config,
            rng,
        }
    }

    /// Predict the outcome of executing a skill from a given state.
    /// Exposed so the composer can check predictions before execution.
    pub async fn predict(
        &self,
        skill_id: &str,
        params: &str,
        current: &JumpyState,
        commitment: f32,
    ) -> Result<JumpyState, String> {
        self.predictor
            .predict(skill_id, params, current, commitment)
            .await
    }

    /// Plan a skill sequence to reach `goal` from `initial_state`.
    ///
    /// Returns the best sequence and its estimated value.
    pub async fn plan(
        &mut self,
        goal: &Goal,
        initial_state: &JumpyState,
    ) -> Result<(SkillSequence, f32), String> {
        if self.config.available_skills.is_empty() {
            return Err("No available skills for planning".to_string());
        }

        let mut best_sequence: SkillSequence = Vec::new();
        let mut best_value = f32::NEG_INFINITY;

        for _ in 0..self.config.num_candidates {
            let seq = self.generate_candidate(initial_state);
            let value = self.evaluate_sequence(&seq, goal, initial_state).await;

            if let Ok(v) = value {
                if v > best_value {
                    best_value = v;
                    best_sequence = seq;
                }
            }
        }

        if best_sequence.is_empty() {
            return Err("Failed to generate any valid plan".to_string());
        }

        Ok((best_sequence, best_value))
    }

    /// Generate a random candidate sequence (proposal distribution).
    ///
    /// Uses a simple heuristic: prefer skills whose historical observations
    /// show progress toward goals with similar tags.
    fn generate_candidate(&mut self, _initial_state: &JumpyState) -> SkillSequence {
        use rand::RngExt;
        let len = self.rng.random_range(1..=self.config.max_sequence_len);
        let mut seq = Vec::with_capacity(len);

        for _ in 0..len {
            let idx = self.rng.random_range(0..self.config.available_skills.len());
            let (skill_id, default_params) = self.config.available_skills[idx].clone();

            // Commitment: geometric decay across sequence position
            // First skill gets high commitment, later ones lower.
            let commitment = 0.95f32.powi(seq.len() as i32).clamp(0.3, 0.95);

            seq.push(SkillProposal {
                skill_id,
                params: default_params,
                commitment,
            });
        }

        seq
    }

    /// Evaluate a candidate sequence using the predictor (Lemma 1 estimator).
    ///
    /// Chains predictions: state_{k+1} = predict(skill_k, state_k).
    /// Final value = goal.evaluate(state_n).
    async fn evaluate_sequence(
        &self,
        seq: &SkillSequence,
        goal: &Goal,
        initial_state: &JumpyState,
    ) -> Result<f32, String> {
        let mut state = initial_state.clone();

        for proposal in seq {
            match self
                .predictor
                .predict(
                    &proposal.skill_id,
                    &proposal.params,
                    &state,
                    proposal.commitment,
                )
                .await
            {
                Ok(next_state) => state = next_state,
                Err(e) => {
                    // If prediction fails for one step, penalize heavily but continue
                    // (this mirrors the "unreliable long-horizon" issue the paper solves).
                    tracing::warn!(
                        "Prediction failed for {}: {}. Penalizing candidate.",
                        proposal.skill_id,
                        e
                    );
                    state.progress *= 0.5; // penalty
                }
            }
        }

        Ok(goal.evaluate(&state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyPredictor;

    #[async_trait::async_trait]
    impl OutcomePredictor for DummyPredictor {
        async fn predict(
            &self,
            _skill_id: &str,
            _params: &str,
            current: &JumpyState,
            _commitment: f32,
        ) -> Result<JumpyState, String> {
            let mut next = current.clone();
            next.progress = (next.progress + 0.2).min(1.0);
            next.tags.push("progressed".to_string());
            Ok(next)
        }
    }

    #[tokio::test]
    async fn test_planner_finds_sequence() {
        let predictor = Arc::new(DummyPredictor);
        let config = PlannerConfig {
            num_candidates: 8,
            max_sequence_len: 3,
            available_skills: vec![
                ("explore".to_string(), "default".to_string()),
                ("code".to_string(), "default".to_string()),
            ],
            rng_seed: None,
        };

        let mut planner = HierarchicalPlanner::new(predictor, config);
        let goal = Goal::new(vec!["progressed".to_string()]);
        let initial = JumpyState::from_query("test task");

        let (seq, value) = planner.plan(&goal, &initial).await.unwrap();
        assert!(!seq.is_empty());
        assert!(value > 0.0);
    }
}
