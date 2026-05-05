// SPDX-License-Identifier: AGPL-3.0
// Copyright (C) 2026 juice094 and contributors

//! Skill Composer — Executes a planned skill sequence with replanning.
//!
//! This is the runtime counterpart to the planner.
//!
//! Key behaviors (MPC-style):
//! 1. Execute skills sequentially.
//! 2. After each skill, compare actual outcome to predicted outcome.
//! 3. If deviation exceeds threshold → trigger **replanning** from current state.
//! 4. Continue until goal is satisfied or max iterations reached.
//!
//! Analogous to RL: execute first action of plan, then replan at next state.

use super::planner::{Goal, HierarchicalPlanner, PlannerConfig};
use super::predictor::{OutcomePredictor, SkillObservation};
use super::state::JumpyState;
use std::sync::Arc;

/// Result of executing one skill in the sequence.
#[derive(Debug, Clone)]
pub struct ExecutionStep {
    pub skill_id: String,
    pub params: String,
    pub predicted_state: JumpyState,
    pub actual_state: JumpyState,
    pub deviation: f32,
    pub replanned: bool,
}

/// Configuration for the composer.
#[derive(Debug, Clone)]
pub struct ComposerConfig {
    /// Deviation threshold that triggers replanning [0, 1].
    pub replan_threshold: f32,
    /// Maximum number of skills to execute before giving up.
    pub max_steps: usize,
    /// Whether to record observations for future learning.
    pub record_observations: bool,
}

impl Default for ComposerConfig {
    fn default() -> Self {
        Self {
            replan_threshold: 0.4,
            max_steps: 20,
            record_observations: true,
        }
    }
}

/// The skill composer runtime.
pub struct SkillComposer<P: OutcomePredictor + ?Sized> {
    planner: HierarchicalPlanner<P>,
    config: ComposerConfig,
    /// Recorded observations for offline learning.
    observations: Vec<SkillObservation>,
}

impl<P: OutcomePredictor + ?Sized> SkillComposer<P> {
    pub fn new(planner: HierarchicalPlanner<P>, config: ComposerConfig) -> Self {
        Self {
            planner,
            config,
            observations: Vec::new(),
        }
    }

    /// Execute skills toward the goal, with online replanning.
    ///
    /// `execute_skill_fn` is a callback that actually runs the skill and returns
    /// the resulting `JumpyState`. This decouples the composer from the Agent internals.
    pub async fn compose<F, Fut>(
        &mut self,
        goal: &Goal,
        initial_state: &JumpyState,
        mut execute_skill_fn: F,
    ) -> Result<CompositionResult, String>
    where
        F: FnMut(&str, &str) -> Fut,
        Fut: std::future::Future<Output = Result<JumpyState, String>>,
    {
        let mut current_state = initial_state.clone();
        let mut history: Vec<ExecutionStep> = Vec::new();
        let mut total_steps = 0usize;

        while total_steps < self.config.max_steps {
            // Check if goal already satisfied
            if goal.evaluate(&current_state) >= 0.95 {
                break;
            }

            // Plan from current state
            let (sequence, _value) = self
                .planner
                .plan(goal, &current_state)
                .await
                .map_err(|e| format!("Planning failed: {}", e))?;

            if sequence.is_empty() {
                return Err("Planner returned empty sequence".to_string());
            }

            // Execute the first skill of the plan (MPC: only commit to one step)
            let proposal = &sequence[0];
            let predicted = self
                .planner
                .predict(
                    &proposal.skill_id,
                    &proposal.params,
                    &current_state,
                    proposal.commitment,
                )
                .await
                .unwrap_or_else(|_| JumpyState::default());

            let actual = execute_skill_fn(&proposal.skill_id, &proposal.params).await?;
            let deviation = predicted.distance(&actual);

            let step = ExecutionStep {
                skill_id: proposal.skill_id.clone(),
                params: proposal.params.clone(),
                predicted_state: predicted.clone(),
                actual_state: actual.clone(),
                deviation,
                replanned: false,
            };

            history.push(step.clone());

            // Record observation for future learning
            if self.config.record_observations {
                self.observations.push(SkillObservation {
                    skill_id: proposal.skill_id.clone(),
                    params: proposal.params.clone(),
                    before: current_state.clone(),
                    after: actual.clone(),
                });
            }

            current_state = actual;
            total_steps += 1;

            // Trigger replan if deviation is high
            if deviation > self.config.replan_threshold {
                tracing::info!(
                    "Deviation {} > threshold {} — replanning from new state",
                    deviation,
                    self.config.replan_threshold
                );
                if let Some(last) = history.last_mut() {
                    last.replanned = true;
                }
                // Loop continues: next iteration will replan from current_state
            }
        }

        let success = goal.evaluate(&current_state) >= 0.95;

        Ok(CompositionResult {
            success,
            final_state: current_state,
            steps: history,
            total_steps,
        })
    }

    /// Drain recorded observations for ingestion into a predictor.
    pub fn drain_observations(&mut self) -> Vec<SkillObservation> {
        std::mem::take(&mut self.observations)
    }
}

/// The result of a full composition run.
#[derive(Debug, Clone)]
pub struct CompositionResult {
    pub success: bool,
    pub final_state: JumpyState,
    pub steps: Vec<ExecutionStep>,
    pub total_steps: usize,
}

/// Convenience builder to construct a composer with common defaults.
pub struct ComposerBuilder<P: OutcomePredictor + ?Sized> {
    predictor: Option<Arc<P>>,
    planner_config: PlannerConfig,
    composer_config: ComposerConfig,
}

impl<P: OutcomePredictor + ?Sized> ComposerBuilder<P> {
    pub fn new() -> Self {
        Self {
            predictor: None,
            planner_config: PlannerConfig::default(),
            composer_config: ComposerConfig::default(),
        }
    }

    pub fn with_predictor(mut self, predictor: Arc<P>) -> Self {
        self.predictor = Some(predictor);
        self
    }

    pub fn with_skills(mut self, skills: Vec<(String, String)>) -> Self {
        self.planner_config.available_skills = skills;
        self
    }

    pub fn with_replan_threshold(mut self, threshold: f32) -> Self {
        self.composer_config.replan_threshold = threshold;
        self
    }

    pub fn with_num_candidates(mut self, n: usize) -> Self {
        self.planner_config.num_candidates = n;
        self
    }

    pub fn with_rng_seed(mut self, seed: u64) -> Self {
        self.planner_config.rng_seed = Some(seed);
        self
    }

    pub fn build(self) -> Result<SkillComposer<P>, String> {
        let predictor = self.predictor.ok_or("Predictor required")?;
        let planner = HierarchicalPlanner::new(predictor, self.planner_config);
        Ok(SkillComposer::new(planner, self.composer_config))
    }
}

impl<P: OutcomePredictor + ?Sized> Default for ComposerBuilder<P> {
    fn default() -> Self {
        Self::new()
    }
}
