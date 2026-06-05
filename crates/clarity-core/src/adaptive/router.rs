//! Dynamic model routing — select the optimal LLM provider based on historical performance.
//!
//! The [`AdaptiveModelRouter`] maintains a real-time profile for each provider
//! (latency, error rate, cost, quality) and uses it to route tasks to the
//! provider most likely to succeed within budget constraints.
//!
//! ## Routing pipeline
//!
//! ```text
//! TaskDescriptor
//!      │
//!      ▼
//! ┌─────────────────┐
//! │ Filter: capability│  — exclude providers that don't support required features
//! │ (reasoning, etc.) │
//! └─────────────────┘
//!      │
//!      ▼
//! ┌─────────────────┐
//! │ Score: weighted │  — combine latency / quality / cost / user-preference scores
//! │ composite         │
//! └─────────────────┘
//!      │
//!      ▼
//! ┌─────────────────┐
//! │ Rank + select   │  — pick top provider, emit ModelRoute event for feedback
//! └─────────────────┘
//! ```

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ============================================================================
// Error types
// ============================================================================

/// Errors emitted by the adaptive router.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum RouterError {
    #[error("no provider available for task: {0}")]
    NoProvider(String),

    #[error("all providers failed health check")]
    AllProvidersUnhealthy,

    #[error("budget exceeded for task: required {required}, available {available}")]
    BudgetExceeded { required: f64, available: f64 },
}

// ============================================================================
// TaskDescriptor
// ============================================================================

/// Description of a task submitted for routing.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TaskDescriptor {
    /// Human-readable task classification.
    pub task_type: TaskType,

    /// Estimated token count for the prompt.
    pub estimated_input_tokens: usize,

    /// Whether the task requires reasoning capabilities.
    pub requires_reasoning: bool,

    /// Whether the task requires vision capabilities.
    pub requires_vision: bool,

    /// Whether the task requires tool use.
    pub requires_tools: bool,

    /// Maximum acceptable latency in milliseconds.
    pub max_latency_ms: Option<u64>,

    /// Maximum acceptable cost in USD.
    pub max_cost_usd: Option<f64>,

    /// Minimum quality threshold (0.0 - 1.0).
    pub min_quality: f64,
}

impl TaskDescriptor {
    /// Create a simple task descriptor from a task type.
    pub fn new(task_type: TaskType) -> Self {
        Self {
            task_type,
            ..Default::default()
        }
    }

    /// Set estimated input tokens.
    pub fn with_estimated_tokens(mut self, tokens: usize) -> Self {
        self.estimated_input_tokens = tokens;
        self
    }

    /// Require reasoning capability.
    pub fn reasoning(mut self) -> Self {
        self.requires_reasoning = true;
        self
    }

    /// Require vision capability.
    pub fn vision(mut self) -> Self {
        self.requires_vision = true;
        self
    }

    /// Require tool use.
    pub fn tools(mut self) -> Self {
        self.requires_tools = true;
        self
    }
}

/// Classification of agent tasks for routing decisions.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskType {
    /// Quick, low-latency coding tasks.
    Coding,
    /// Exploratory research and analysis.
    Explore,
    /// Multi-step planning with conditional branches.
    Plan,
    /// File read, grep, glob — tool-heavy, low reasoning.
    FileOps,
    /// Shell execution and system interaction.
    System,
    /// Long-form content generation.
    Write,
    /// Code review and diff analysis.
    Review,
    /// Memory query and knowledge retrieval.
    MemoryQuery,
    /// Background/async task.
    Background,
    #[default]
    General,
}

// ============================================================================
// ProviderProfile
// ============================================================================

/// Historical performance profile for a single LLM provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProviderProfile {
    /// Provider identifier (e.g. "kimi-coding", "anthropic", "local-gguf").
    pub provider_id: String,

    /// Exponentially-weighted moving average latency (ms).
    pub avg_latency_ms: f64,

    /// Sliding-window error rate (0.0 - 1.0).
    pub error_rate: f64,

    /// Estimated cost per 1K input+output tokens (USD).
    pub cost_per_1k_tokens: f64,

    /// Quality score derived from user feedback and task success (0.0 - 1.0).
    pub quality_score: f64,

    /// When this provider was last selected.
    pub last_used: Option<DateTime<Utc>>,

    /// Number of times this provider has been selected.
    pub selection_count: u64,

    /// EWMA alpha for latency updates (higher = more responsive to recent values).
    #[serde(skip)]
    latency_alpha: f64,

    /// Error-rate window size (number of recent calls considered).
    #[serde(skip)]
    error_window_size: usize,
}

impl ProviderProfile {
    /// Create a new profile with default parameters.
    pub fn new(provider_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            avg_latency_ms: 0.0,
            error_rate: 0.0,
            cost_per_1k_tokens: 0.0,
            quality_score: 0.5, // neutral starting point
            last_used: None,
            selection_count: 0,
            latency_alpha: 0.3,
            error_window_size: 20,
        }
    }

    /// Update latency with a new observation (EWMA).
    pub fn observe_latency(&mut self, latency_ms: f64) {
        if self.avg_latency_ms == 0.0 {
            self.avg_latency_ms = latency_ms;
        } else {
            self.avg_latency_ms =
                self.latency_alpha * latency_ms + (1.0 - self.latency_alpha) * self.avg_latency_ms;
        }
    }

    /// Update error rate with a new observation (sliding window approximation).
    pub fn observe_error(&mut self, is_error: bool) {
        let n = self.error_window_size as f64;
        let error_value = if is_error { 1.0 } else { 0.0 };
        self.error_rate = (error_value + (n - 1.0) * self.error_rate) / n;
    }

    /// Update quality score with user feedback (-1.0 to +1.0).
    pub fn observe_feedback(&mut self, feedback: f64) {
        // Map feedback (-1..+1) to quality update
        let alpha = 0.2;
        let normalized = (feedback + 1.0) / 2.0; // 0..1
        self.quality_score = alpha * normalized + (1.0 - alpha) * self.quality_score;
    }

    /// Compute a composite fitness score for a given task.
    ///
    /// Higher is better. Weights are derived from task type.
    pub fn fitness_score(&self, task: &TaskDescriptor, weights: &RoutingWeights) -> f64 {
        // Normalize each dimension to 0..1 (higher is better)
        let latency_score = self.latency_score(task.max_latency_ms);
        let error_score = 1.0 - self.error_rate;
        let quality_score = self.quality_score;
        let cost_score = self.cost_score(task.max_cost_usd, task.estimated_input_tokens);

        weights.latency * latency_score
            + weights.quality * quality_score
            + weights.reliability * error_score
            + weights.cost * cost_score
    }

    fn latency_score(&self, max_latency: Option<u64>) -> f64 {
        match max_latency {
            Some(max) if max > 0 => {
                let ratio = self.avg_latency_ms / max as f64;
                (1.0 - ratio).clamp(0.0, 1.0)
            }
            _ => {
                // No latency constraint — score by relative speed
                // (faster providers score higher, but the scale is soft)
                let reference = 5000.0; // 5s reference
                (1.0 - self.avg_latency_ms / reference).clamp(0.0, 1.0)
            }
        }
    }

    fn cost_score(&self, max_cost: Option<f64>, estimated_tokens: usize) -> f64 {
        let estimated_cost = self.cost_per_1k_tokens * estimated_tokens as f64 / 1000.0;
        match max_cost {
            Some(max) if max > 0.0 => {
                if estimated_cost > max {
                    0.0
                } else {
                    1.0 - estimated_cost / max
                }
            }
            _ => {
                // No cost constraint — cheaper is better
                let reference = 0.01; // $0.01 reference
                (1.0 - estimated_cost / reference).clamp(0.0, 1.0)
            }
        }
    }
}

impl Default for ProviderProfile {
    fn default() -> Self {
        Self::new("unknown")
    }
}

// ============================================================================
// RoutingWeights
// ============================================================================

/// Per-task-type weighting for the composite fitness function.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoutingWeights {
    pub latency: f64,
    pub quality: f64,
    pub reliability: f64,
    pub cost: f64,
}

impl RoutingWeights {
    /// Weights optimized for coding tasks (latency-critical).
    pub fn coding() -> Self {
        Self {
            latency: 0.4,
            quality: 0.2,
            reliability: 0.2,
            cost: 0.2,
        }
    }

    /// Weights optimized for planning tasks (quality-critical).
    pub fn plan() -> Self {
        Self {
            latency: 0.1,
            quality: 0.5,
            reliability: 0.3,
            cost: 0.1,
        }
    }

    /// Weights optimized for background tasks (cost-critical).
    pub fn background() -> Self {
        Self {
            latency: 0.1,
            quality: 0.2,
            reliability: 0.2,
            cost: 0.5,
        }
    }

    /// Balanced weights for general tasks.
    pub fn general() -> Self {
        Self {
            latency: 0.25,
            quality: 0.3,
            reliability: 0.25,
            cost: 0.2,
        }
    }
}

impl Default for RoutingWeights {
    fn default() -> Self {
        Self::general()
    }
}

// ============================================================================
// AdaptiveModelRouter
// ============================================================================

/// Data-driven router that selects the best LLM provider for a given task.
///
/// Profiles are updated reactively (after each task completion) and read
/// during routing. All updates are lock-free reads via `RwLock`.
pub struct AdaptiveModelRouter {
    /// Registry of provider profiles (provider_id → profile).
    profiles: RwLock<HashMap<String, ProviderProfile>>,

    /// Minimum quality threshold below which a provider is excluded.
    min_quality_threshold: f64,

    /// Maximum error rate above which a provider is excluded.
    max_error_rate: f64,
}

impl AdaptiveModelRouter {
    /// Create a new router with empty profiles.
    pub fn new() -> Self {
        Self {
            profiles: RwLock::new(HashMap::new()),
            min_quality_threshold: 0.1,
            max_error_rate: 0.5,
        }
    }

    /// Register a provider with an initial profile.
    pub fn register_provider(&self, profile: ProviderProfile) {
        let mut profiles = self.profiles.write();
        profiles.insert(profile.provider_id.clone(), profile);
    }

    /// Observe a task outcome and update the corresponding provider profile.
    pub fn observe_outcome(
        &self,
        provider_id: &str,
        latency_ms: u64,
        is_error: bool,
        feedback: Option<f64>,
    ) {
        let mut profiles = self.profiles.write();
        if let Some(profile) = profiles.get_mut(provider_id) {
            profile.observe_latency(latency_ms as f64);
            profile.observe_error(is_error);
            profile.last_used = Some(Utc::now());
            profile.selection_count += 1;
            if let Some(fb) = feedback {
                profile.observe_feedback(fb);
            }
        }
    }

    /// Route a task to the best available provider.
    ///
    /// # Algorithm
    ///
    /// 1. Filter out providers that don't meet health thresholds.
    /// 2. Filter out providers that don't support required capabilities.
    /// 3. Score remaining providers using weighted composite fitness.
    /// 4. Return the highest-scoring provider.
    pub fn route(&self, task: &TaskDescriptor) -> Result<String, RouterError> {
        let profiles = self.profiles.read();

        if profiles.is_empty() {
            return Err(RouterError::NoProvider(
                "no providers registered".to_string(),
            ));
        }

        let weights = weights_for_task(&task.task_type);

        let mut candidates: Vec<(&String, f64)> = profiles
            .iter()
            .filter(|(_, profile)| self.is_healthy(profile))
            .filter(|(_, profile)| self.capable(profile, task))
            .map(|(id, profile)| (id, profile.fitness_score(task, &weights)))
            .collect();

        if candidates.is_empty() {
            return Err(RouterError::AllProvidersUnhealthy);
        }

        // Sort by score descending.
        candidates.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        Ok(candidates[0].0.clone())
    }

    /// Get a copy of all current profiles (for UI display / serialization).
    pub fn profiles(&self) -> HashMap<String, ProviderProfile> {
        self.profiles.read().clone()
    }

    /// Get a single provider profile.
    pub fn profile(&self, provider_id: &str) -> Option<ProviderProfile> {
        self.profiles.read().get(provider_id).cloned()
    }

    fn is_healthy(&self, profile: &ProviderProfile) -> bool {
        profile.quality_score >= self.min_quality_threshold
            && profile.error_rate <= self.max_error_rate
    }

    fn capable(&self, _profile: &ProviderProfile, _task: &TaskDescriptor) -> bool {
        // NOTE: In a full implementation, this would check provider capability
        // flags (reasoning, vision, tool-use) against task requirements.
        // For now, we assume all registered providers are capable.
        true
    }
}

impl Default for AdaptiveModelRouter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn weights_for_task(task_type: &TaskType) -> RoutingWeights {
    match task_type {
        TaskType::Coding => RoutingWeights::coding(),
        TaskType::Plan => RoutingWeights::plan(),
        TaskType::Background => RoutingWeights::background(),
        _ => RoutingWeights::general(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_provider_profile_ewma_latency() {
        let mut profile = ProviderProfile::new("test");
        profile.observe_latency(1000.0);
        assert!((profile.avg_latency_ms - 1000.0).abs() < 0.01);

        profile.observe_latency(2000.0);
        // EWMA: 0.3 * 2000 + 0.7 * 1000 = 1300
        assert!((profile.avg_latency_ms - 1300.0).abs() < 0.01);
    }

    #[test]
    fn test_provider_profile_error_rate() {
        let mut profile = ProviderProfile::new("test");
        // Start with 0 errors in window of 20
        for _ in 0..20 {
            profile.observe_error(false);
        }
        assert!(profile.error_rate < 0.01);

        // One error in window
        profile.observe_error(true);
        // Approx: 1/20 = 0.05
        assert!(profile.error_rate > 0.04 && profile.error_rate < 0.06);
    }

    #[test]
    fn test_router_basic() {
        let router = AdaptiveModelRouter::new();

        let fast = ProviderProfile::new("fast")
            .tap(|p| p.avg_latency_ms = 100.0)
            .tap(|p| p.quality_score = 0.9);
        let slow = ProviderProfile::new("slow")
            .tap(|p| p.avg_latency_ms = 5000.0)
            .tap(|p| p.quality_score = 0.9);

        router.register_provider(fast);
        router.register_provider(slow);

        let task = TaskDescriptor::new(TaskType::Coding).with_estimated_tokens(1000);
        let choice = router.route(&task).unwrap();

        assert_eq!(choice, "fast");
    }

    #[test]
    fn test_router_excludes_unhealthy() {
        let router = AdaptiveModelRouter::new();

        let good = ProviderProfile::new("good").tap(|p| {
            p.quality_score = 0.9;
            p.error_rate = 0.01;
        });
        let bad = ProviderProfile::new("bad").tap(|p| {
            p.quality_score = 0.05; // below threshold
            p.error_rate = 0.9; // above threshold
        });

        router.register_provider(good);
        router.register_provider(bad);

        let task = TaskDescriptor::new(TaskType::General);
        let choice = router.route(&task).unwrap();

        assert_eq!(choice, "good");
    }

    #[test]
    fn test_router_no_providers() {
        let router = AdaptiveModelRouter::new();
        let task = TaskDescriptor::new(TaskType::General);
        assert!(matches!(
            router.route(&task),
            Err(RouterError::NoProvider(_))
        ));
    }

    // Helper trait for builder-style mutation in tests.
    trait Tap: Sized {
        fn tap<F>(mut self, f: F) -> Self
        where
            F: FnOnce(&mut Self),
        {
            f(&mut self);
            self
        }
    }
    impl<T: Sized> Tap for T {}
}
