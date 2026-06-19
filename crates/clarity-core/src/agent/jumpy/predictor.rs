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
use clarity_contract::AgentError;
use std::collections::HashMap;
use std::sync::Arc;

/// Simplified LLM provider interface for outcome prediction.
#[async_trait::async_trait]
pub trait LlmProvider: Send + Sync {
    /// Send a prompt to the LLM and get the response text.
    async fn complete(&self, prompt: &str, model: &str) -> Result<String, AgentError>;
}

/// Adapter that bridges the full `clarity_contract::LlmProvider` (agent-level)
/// to the simplified `jumpy::predictor::LlmProvider`.
///
/// This allows headless CLI / gateway code to reuse existing provider
/// construction logic (OpenAI, Ollama, etc.) without duplicating setup.
pub struct LlmAdapter {
    inner: Arc<dyn clarity_contract::LlmProvider>,
}

impl LlmAdapter {
    /// Wrap an existing LLM provider.
    pub fn new(inner: Arc<dyn clarity_contract::LlmProvider>) -> Self {
        Self { inner }
    }
}

#[async_trait::async_trait]
impl LlmProvider for LlmAdapter {
    async fn complete(&self, prompt: &str, _model: &str) -> Result<String, AgentError> {
        use clarity_contract::{Message, MessageRole};
        let messages = vec![Message {
            role: MessageRole::User,
            content: prompt.to_string(),
            tool_calls: None,
            tool_call_id: None,
        }];
        let response = self
            .inner
            .complete(&messages, &serde_json::Value::Null)
            .await?;
        Ok(response.content)
    }
}

/// LLM-augmented predictor that asks an LLM to simulate the outcome of a skill.
pub struct LlmAugmentedPredictor {
    llm: Arc<dyn LlmProvider>,
    system_prompt: String,
}

impl LlmAugmentedPredictor {
    /// Create a new predictor with the given LLM provider.
    pub fn new(llm: Arc<dyn LlmProvider>) -> Self {
        Self {
            llm,
            system_prompt: "You are a world model for an AI agent. Predict the state changes caused by executing a skill.".to_string(),
        }
    }

    /// Override the default system prompt.
    pub fn with_system_prompt(mut self, prompt: String) -> Self {
        self.system_prompt = prompt;
        self
    }

    fn build_prompt(&self, skill_id: &str, params: &str, current: &JumpyState) -> String {
        let memory_json =
            serde_json::to_string(&current.memory).unwrap_or_else(|_| "{}".to_string());
        format!(
            "{}\n\nCurrent State:\n- Tags: {:?}\n- Memory: {}\n- Active Files: {:?}\n- Progress: {}\n- Context: {}\n\nSkill to Execute: {}\nParameters: {}\n\nPredict the resulting state after execution. Output valid JSON with fields: tags, memory, active_files, context_summary, progress.",
            self.system_prompt,
            current.tags,
            memory_json,
            current.active_files,
            current.progress,
            current.context_summary,
            skill_id,
            params
        )
    }
}

#[async_trait::async_trait]
impl OutcomePredictor for LlmAugmentedPredictor {
    async fn predict(
        &self,
        skill_id: &str,
        params: &str,
        current: &JumpyState,
        _commitment: f32,
    ) -> Result<JumpyState, String> {
        let prompt = self.build_prompt(skill_id, params, current);
        let response = self
            .llm
            .complete(&prompt, "default")
            .await
            .map_err(|e| format!("LLM completion failed: {}", e))?;

        let predicted: JumpyState = serde_json::from_str(&response)
            .map_err(|e| format!("Failed to parse LLM response as JumpyState: {}", e))?;

        Ok(predicted)
    }
}

/// Hybrid predictor that tries historical first, then falls back to LLM.
pub struct HybridPredictor {
    historical: HistoricalPredictor,
    llm: LlmAugmentedPredictor,
    confidence_threshold: f32,
    cache_synthetic: bool,
}

impl HybridPredictor {
    /// Create a new hybrid predictor.
    pub fn new(historical: HistoricalPredictor, llm: LlmAugmentedPredictor) -> Self {
        Self {
            historical,
            llm,
            confidence_threshold: 0.5,
            cache_synthetic: false,
        }
    }

    /// Set the confidence threshold for trusting historical predictions.
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.confidence_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Enable or disable caching of synthetic (LLM-generated) predictions.
    pub fn with_caching(mut self, enable: bool) -> Self {
        self.cache_synthetic = enable;
        self
    }
}

#[async_trait::async_trait]
impl OutcomePredictor for HybridPredictor {
    async fn predict(
        &self,
        skill_id: &str,
        params: &str,
        current: &JumpyState,
        commitment: f32,
    ) -> Result<JumpyState, String> {
        // 1. Try historical first
        match self
            .historical
            .predict(skill_id, params, current, commitment)
            .await
        {
            Ok(state) => Ok(state),
            Err(_) => {
                // 2. If Err(_) → fallback to llm.predict()
                self.llm
                    .predict(skill_id, params, current, commitment)
                    .await
            }
        }
    }
}

/// A single observed transition: (skill, params, before, after).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SkillObservation {
    /// Skill identifier.
    pub skill_id: String,
    /// Serialized skill parameters.
    pub params: String, // JSON or human-readable parameterization
    /// State before the skill invocation.
    pub before: JumpyState,
    /// State after the skill invocation.
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
    /// Create a new `Default`.
    pub fn new() -> Self {
        Self {
            observations: HashMap::new(),
            similarity_threshold: 0.3,
        }
    }

    /// Set the threshold.
    pub fn with_threshold(mut self, threshold: f32) -> Self {
        self.similarity_threshold = threshold.clamp(0.0, 1.0);
        self
    }

    /// Ingest a new observation (offline learning).
    pub fn observe(&mut self, obs: SkillObservation) {
        let key = format!(
            "{}:{}",
            obs.skill_id,
            Self::canonicalize_params(&obs.params)
        );
        self.observations.entry(key).or_default().push(obs);
    }

    /// Normalize JSON params to a canonical compact form so that
    /// `{ "path" : "x" }` and `{"path":"x"}` map to the same key.
    fn canonicalize_params(params: &str) -> String {
        serde_json::from_str::<serde_json::Value>(params)
            .ok()
            .map(|v| {
                let mut buf = Vec::new();
                Self::write_canonical(&v, &mut buf);
                String::from_utf8(buf).unwrap_or_else(|_| params.to_string())
            })
            .unwrap_or_else(|| params.to_string())
    }

    fn write_canonical(v: &serde_json::Value, buf: &mut Vec<u8>) {
        match v {
            serde_json::Value::Null => buf.extend_from_slice(b"null"),
            serde_json::Value::Bool(b) => {
                buf.extend_from_slice(if *b { b"true" } else { b"false" })
            }
            serde_json::Value::Number(n) => buf.extend_from_slice(n.to_string().as_bytes()),
            serde_json::Value::String(s) => {
                buf.push(b'"');
                for ch in s.chars() {
                    match ch {
                        '"' => buf.extend_from_slice(b"\\\""),
                        '\\' => buf.extend_from_slice(b"\\\\"),
                        '\n' => buf.extend_from_slice(b"\\n"),
                        '\r' => buf.extend_from_slice(b"\\r"),
                        '\t' => buf.extend_from_slice(b"\\t"),
                        c if c.is_control() => {
                            buf.extend_from_slice(format!("\\u{:04x}", c as u32).as_bytes());
                        }
                        c => {
                            let mut b = [0; 4];
                            buf.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
                        }
                    }
                }
                buf.push(b'"');
            }
            serde_json::Value::Array(arr) => {
                buf.push(b'[');
                for (i, item) in arr.iter().enumerate() {
                    if i > 0 {
                        buf.push(b',');
                    }
                    Self::write_canonical(item, buf);
                }
                buf.push(b']');
            }
            serde_json::Value::Object(map) => {
                buf.push(b'{');
                let mut items: Vec<_> = map.iter().collect();
                items.sort_by(|a, b| a.0.cmp(b.0));
                for (i, (k, v)) in items.iter().enumerate() {
                    if i > 0 {
                        buf.push(b',');
                    }
                    Self::write_canonical(&serde_json::Value::String(k.to_string()), buf);
                    buf.push(b':');
                    Self::write_canonical(v, buf);
                }
                buf.push(b'}');
            }
        }
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
        let key = format!("{}:{}", skill_id, Self::canonicalize_params(params));
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
    /// Create a new instance.
    pub fn new(inner: P) -> Self {
        Self {
            inner,
            short_commitment: 0.5,
            long_commitment: 0.9,
        }
    }

    /// Set the horizons.
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

#[cfg(test)]
mod tests {
    use super::*;

    struct MockLlmProvider {
        response: String,
    }

    #[async_trait::async_trait]
    impl LlmProvider for MockLlmProvider {
        async fn complete(&self, _prompt: &str, _model: &str) -> Result<String, AgentError> {
            Ok(self.response.clone())
        }
    }

    #[tokio::test]
    async fn test_llm_predictor_valid_json() {
        let json_response = r#"{
            "tags": ["done"],
            "memory": {"key": "value"},
            "active_files": ["src/main.rs"],
            "context_summary": "summary",
            "progress": 0.8
        }"#;
        let llm = Arc::new(MockLlmProvider {
            response: json_response.to_string(),
        });
        let predictor = LlmAugmentedPredictor::new(llm);
        let current = JumpyState::default();
        let result = predictor.predict("test_skill", "{}", &current, 0.5).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.tags, vec!["done"]);
        assert_eq!(state.memory.get("key"), Some(&"value".to_string()));
        assert_eq!(state.active_files, vec!["src/main.rs"]);
        assert_eq!(state.context_summary, "summary");
        assert!((state.progress - 0.8).abs() < f32::EPSILON);
    }

    #[tokio::test]
    async fn test_llm_predictor_invalid_json() {
        let llm = Arc::new(MockLlmProvider {
            response: "not valid json".to_string(),
        });
        let predictor = LlmAugmentedPredictor::new(llm);
        let current = JumpyState::default();
        let result = predictor.predict("test_skill", "{}", &current, 0.5).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse"));
    }

    #[tokio::test]
    async fn test_hybrid_historical_hit() {
        let mut historical = HistoricalPredictor::new();
        let obs = SkillObservation {
            skill_id: "test_skill".to_string(),
            params: "{}".to_string(),
            before: JumpyState::default(),
            after: JumpyState {
                tags: vec!["historical_tag".to_string()],
                memory: HashMap::new(),
                active_files: vec![],
                context_summary: "historical".to_string(),
                progress: 0.5,
            },
        };
        historical.observe(obs);

        let llm = Arc::new(MockLlmProvider {
            response: r#"{"tags":["llm_tag"],"memory":{},"active_files":[],"context_summary":"llm","progress":0.9}"#.to_string(),
        });
        let llm_predictor = LlmAugmentedPredictor::new(llm);
        let hybrid = HybridPredictor::new(historical, llm_predictor);
        let current = JumpyState::default();
        let result = hybrid.predict("test_skill", "{}", &current, 0.5).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.tags, vec!["historical_tag"]);
    }

    #[test]
    fn test_trajectories_json_parseable() {
        // Verify J10 Phase 2 training data is valid SkillObservation JSON.
        let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
        let path = std::path::PathBuf::from(manifest_dir)
            .join("..")
            .join("..")
            .join("training-data")
            .join("trajectories")
            .join("observations.json");
        if !path.exists() {
            // Trajectories not generated yet — skip gracefully.
            return;
        }
        let data = std::fs::read_to_string(&path).expect("read trajectories");
        let observations: Vec<SkillObservation> =
            serde_json::from_str(&data).expect("parse trajectories as SkillObservation array");
        assert!(
            observations.len() >= 20,
            "Expected at least 20 trajectories, got {}",
            observations.len()
        );

        // Verify HistoricalPredictor can ingest them without panic.
        let mut predictor = HistoricalPredictor::new();
        predictor.observe_batch(observations);
    }

    #[test]
    fn test_canonicalize_params_json() {
        // Whitespace differences should collapse to the same canonical form.
        let a = HistoricalPredictor::canonicalize_params(r#"{"path":"x"}"#);
        let b = HistoricalPredictor::canonicalize_params(r#"{ "path" : "x" }"#);
        assert_eq!(a, b);
        assert_eq!(a, r#"{"path":"x"}"#);

        // Key order should be normalized (sorted).
        let c = HistoricalPredictor::canonicalize_params(r#"{"b":1,"a":2}"#);
        assert_eq!(c, r#"{"a":2,"b":1}"#);

        // Nested objects.
        let d = HistoricalPredictor::canonicalize_params(r#"{"outer":{"z":1,"a":2}}"#);
        assert_eq!(d, r#"{"outer":{"a":2,"z":1}}"#);

        // Non-JSON falls back to verbatim.
        let e = HistoricalPredictor::canonicalize_params("not json");
        assert_eq!(e, "not json");
    }

    #[test]
    fn test_canonicalize_params_observation_key_match() {
        let mut predictor = HistoricalPredictor::new();
        let obs = SkillObservation {
            skill_id: "file_read".to_string(),
            params: r#"{ "path" : "src/main.rs" }"#.to_string(),
            before: JumpyState::default(),
            after: JumpyState {
                tags: vec!["matched".to_string()],
                memory: HashMap::new(),
                active_files: vec![],
                context_summary: "ok".to_string(),
                progress: 1.0,
            },
        };
        predictor.observe(obs);

        // Query with different whitespace should find the observation.
        let result = tokio::runtime::Runtime::new()
            .unwrap()
            .block_on(predictor.predict(
                "file_read",
                r#"{"path":"src/main.rs"}"#,
                &JumpyState::default(),
                0.5,
            ));
        assert!(result.is_ok());
        assert_eq!(result.unwrap().tags, vec!["matched"]);
    }

    #[tokio::test]
    async fn test_hybrid_llm_fallback() {
        let historical = HistoricalPredictor::new();
        let json_response = r#"{
            "tags": ["llm_tag"],
            "memory": {},
            "active_files": [],
            "context_summary": "llm",
            "progress": 0.9
        }"#;
        let llm = Arc::new(MockLlmProvider {
            response: json_response.to_string(),
        });
        let llm_predictor = LlmAugmentedPredictor::new(llm);
        let hybrid = HybridPredictor::new(historical, llm_predictor);
        let current = JumpyState::default();
        let result = hybrid.predict("test_skill", "{}", &current, 0.5).await;
        assert!(result.is_ok());
        let state = result.unwrap();
        assert_eq!(state.tags, vec!["llm_tag"]);
    }
}
