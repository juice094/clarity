//! Agent Growth Profile — persistent character and capability evolution for Clarity souls.
//!
//! Each agent instance ("soul") maintains a [`AgentGrowthProfile`] that records:
//! - Skill mastery progression
//! - Tool use effectiveness
//! - Model preference evolution over time
//! - User interaction patterns
//! - Compression strategy outcomes
//!
//! Profiles are stored as human-readable JSON in `~/.clarity/profiles/` and
//! loaded on soul wake. They serve as the long-term memory of the agent's
//! learning trajectory, distinct from the episodic memory managed by
//! `clarity-memory`.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ============================================================================
// AgentGrowthProfile
// ============================================================================

/// Persistent growth state for a single agent soul.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AgentGrowthProfile {
    /// Unique soul identifier.
    pub soul_id: String,

    /// When this profile was first created.
    pub created_at: DateTime<Utc>,

    /// Profile schema version for migration compatibility.
    pub version: u32,

    /// Skill mastery levels (skill_id → level).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub skill_mastery: HashMap<String, MasteryLevel>,

    /// Tool use statistics (tool_name → stats).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub tool_effectiveness: HashMap<String, ToolStats>,

    /// Model preference evolution over time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub model_preference_evolution: Vec<ModelPreferenceSnapshot>,

    /// User interaction patterns.
    #[serde(default)]
    pub user_interaction_patterns: InteractionPatterns,

    /// Outcomes of past compression strategies.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub compression_history: Vec<CompressionOutcome>,
}

impl AgentGrowthProfile {
    /// Create a new profile for the given soul.
    pub fn new(soul_id: impl Into<String>) -> Self {
        Self {
            soul_id: soul_id.into(),
            created_at: Utc::now(),
            version: 1,
            skill_mastery: HashMap::new(),
            tool_effectiveness: HashMap::new(),
            model_preference_evolution: Vec::new(),
            user_interaction_patterns: InteractionPatterns::default(),
            compression_history: Vec::new(),
        }
    }

    /// Record a skill usage outcome, updating mastery.
    pub fn record_skill_use(&mut self, skill_id: &str, success: bool) {
        let entry = self.skill_mastery.entry(skill_id.to_string()).or_default();
        entry.total_uses += 1;
        if success {
            entry.successful_uses += 1;
        }
        // Update derived score
        entry.score = entry.successful_uses as f64 / entry.total_uses.max(1) as f64;
    }

    /// Record a tool use outcome, updating effectiveness stats.
    pub fn record_tool_use(
        &mut self,
        tool_name: &str,
        latency_ms: u64,
        success: bool,
        user_satisfaction: Option<f64>,
    ) {
        let entry = self
            .tool_effectiveness
            .entry(tool_name.to_string())
            .or_default();
        entry.total_calls += 1;
        if success {
            entry.successful_calls += 1;
        }
        // EWMA latency
        let alpha = 0.3;
        let latency_f = latency_ms as f64;
        if entry.avg_latency_ms == 0.0 {
            entry.avg_latency_ms = latency_f;
        } else {
            entry.avg_latency_ms = alpha * latency_f + (1.0 - alpha) * entry.avg_latency_ms;
        }
        if let Some(sat) = user_satisfaction {
            entry.user_satisfaction = Some(
                entry
                    .user_satisfaction
                    .map(|existing| 0.2 * sat + 0.8 * existing)
                    .unwrap_or(sat),
            );
        }
    }

    /// Record a model preference snapshot.
    pub fn record_model_preference(&mut self, provider_id: &str, weight: f64) {
        self.model_preference_evolution
            .push(ModelPreferenceSnapshot {
                timestamp: Utc::now(),
                provider_id: provider_id.to_string(),
                weight,
            });
        // Prune old snapshots — keep last 100.
        if self.model_preference_evolution.len() > 100 {
            self.model_preference_evolution
                .drain(0..self.model_preference_evolution.len() - 100);
        }
    }

    /// Get current model preferences (aggregated from evolution history).
    pub fn current_model_preferences(&self) -> HashMap<String, f64> {
        let mut prefs: HashMap<String, (f64, usize)> = HashMap::new();
        for snap in &self.model_preference_evolution {
            let (sum, count) = prefs.entry(snap.provider_id.clone()).or_insert((0.0, 0));
            *sum += snap.weight;
            *count += 1;
        }
        prefs
            .into_iter()
            .map(|(k, (sum, count))| (k, sum / count.max(1) as f64))
            .collect()
    }

    /// Record a compression strategy outcome.
    pub fn record_compression_outcome(
        &mut self,
        method: String,
        tokens_before: usize,
        tokens_after: usize,
        user_regenerated: bool,
    ) {
        self.compression_history.push(CompressionOutcome {
            timestamp: Utc::now(),
            method,
            tokens_before,
            tokens_after,
            user_regenerated,
        });
        // Prune old outcomes — keep last 50.
        if self.compression_history.len() > 50 {
            self.compression_history
                .drain(0..self.compression_history.len() - 50);
        }
    }

    /// Path where this profile should be persisted.
    pub fn profile_path(&self) -> std::path::PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".clarity")
            .join("profiles")
            .join(format!("{}.json", self.soul_id))
    }

    /// Save profile to disk.
    pub fn save(&self) -> Result<(), std::io::Error> {
        let path = self.profile_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, json)
    }

    /// Load profile from disk, or create a new one if it doesn't exist.
    pub fn load_or_create(soul_id: impl Into<String>) -> Self {
        let soul_id = soul_id.into();
        let path = dirs::home_dir()
            .unwrap_or_else(|| std::path::PathBuf::from("."))
            .join(".clarity")
            .join("profiles")
            .join(format!("{}.json", &soul_id));

        if let Ok(contents) = std::fs::read_to_string(&path) {
            if let Ok(profile) = serde_json::from_str::<Self>(&contents) {
                return profile;
            }
        }
        Self::new(soul_id)
    }
}

// ============================================================================
// Supporting types
// ============================================================================

/// Skill mastery progression.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct MasteryLevel {
    /// Total number of times this skill has been invoked.
    pub total_uses: u64,
    /// Number of successful invocations.
    pub successful_uses: u64,
    /// Normalized score (0.0 - 1.0).
    pub score: f64,
}

/// Statistics for a single tool.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ToolStats {
    /// Total number of calls.
    pub total_calls: u64,
    /// Number of successful calls.
    pub successful_calls: u64,
    /// EWMA average latency (ms).
    pub avg_latency_ms: f64,
    /// EWMA user satisfaction (-1.0 to +1.0), if feedback exists.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_satisfaction: Option<f64>,
}

/// A single point in the model preference evolution timeline.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelPreferenceSnapshot {
    /// Timestamp of the record.
    pub timestamp: DateTime<Utc>,
    /// Provider identifier.
    pub provider_id: String,
    /// Normalized weight (0.0 - 1.0) for this provider at this time.
    pub weight: f64,
}

/// User interaction patterns derived from telemetry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct InteractionPatterns {
    /// Preferred session length (number of turns).
    pub preferred_session_length: Option<usize>,
    /// Typical time of day for activity (hour of day, 0-23).
    pub typical_active_hour: Option<u8>,
    /// Ratio of plan mode vs react mode usage.
    pub plan_mode_ratio: f64,
    /// Frequency of explicit user feedback (per 100 messages).
    pub feedback_rate: f64,
}

/// Outcome of a compression strategy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CompressionOutcome {
    /// Timestamp of the record.
    pub timestamp: DateTime<Utc>,
    /// Compression method used.
    pub method: String,
    /// Token count before compression.
    pub tokens_before: usize,
    /// Token count after compression.
    pub tokens_after: usize,
    /// Whether the user regenerated after compaction (negative signal).
    pub user_regenerated: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_skill_mastery() {
        let mut profile = AgentGrowthProfile::new("test-soul");
        profile.record_skill_use("coding", true);
        profile.record_skill_use("coding", true);
        profile.record_skill_use("coding", false);

        let coding = profile.skill_mastery.get("coding").unwrap();
        assert_eq!(coding.total_uses, 3);
        assert_eq!(coding.successful_uses, 2);
        assert!((coding.score - 0.6667).abs() < 0.01);
    }

    #[test]
    fn test_profile_tool_stats() {
        let mut profile = AgentGrowthProfile::new("test-soul");
        profile.record_tool_use("bash", 500, true, Some(0.8));
        profile.record_tool_use("bash", 600, true, Some(0.9));

        let bash = profile.tool_effectiveness.get("bash").unwrap();
        assert_eq!(bash.total_calls, 2);
        assert_eq!(bash.successful_calls, 2);
        // EWMA: 0.3 * 600 + 0.7 * 500 = 530
        assert!((bash.avg_latency_ms - 530.0).abs() < 1.0);
        assert!(bash.user_satisfaction.unwrap() > 0.8);
    }

    #[test]
    fn test_model_preference_evolution() {
        let mut profile = AgentGrowthProfile::new("test-soul");
        profile.record_model_preference("kimi", 0.8);
        profile.record_model_preference("anthropic", 0.6);
        profile.record_model_preference("kimi", 0.9);

        let prefs = profile.current_model_preferences();
        assert_eq!(prefs.len(), 2);
        assert!(prefs["kimi"] > prefs["anthropic"]);
    }

    #[test]
    fn test_compression_history_pruning() {
        let mut profile = AgentGrowthProfile::new("test-soul");
        for i in 0..60 {
            profile.record_compression_outcome("tier1".to_string(), 1000 + i, 500 + i, i % 3 == 0);
        }
        assert_eq!(profile.compression_history.len(), 50);
    }

    #[test]
    fn test_profile_roundtrip() {
        let mut profile = AgentGrowthProfile::new("roundtrip-test");
        profile.record_skill_use("write", true);
        profile.record_tool_use("file_read", 100, true, None);

        let json = serde_json::to_string(&profile).unwrap();
        let restored: AgentGrowthProfile = serde_json::from_str(&json).unwrap();

        assert_eq!(profile.soul_id, restored.soul_id);
        assert_eq!(profile.skill_mastery, restored.skill_mastery);
        assert_eq!(profile.tool_effectiveness, restored.tool_effectiveness);
    }
}
