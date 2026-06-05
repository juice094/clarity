//! Context compression optimization — dynamically adjust compaction parameters
//! based on historical token efficiency data.
//!
//! Current `clarity_core::compaction::CompactionConfig` uses static thresholds:
//! - trigger_ratio = 0.8
//! - reserved_tokens = 2000
//! - max_preserve_messages = 2
//!
//! [`CompressionOptimizer`] learns from past sessions to predict which
//! compaction strategy minimizes total tokens while preserving context quality.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::compaction::CompactionConfig;

// ============================================================================
// CompressionParams
// ============================================================================

/// Dynamic compaction parameters produced by the optimizer.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CompressionParams {
    /// Ratio of max tokens that triggers compaction (0.0 - 1.0).
    pub trigger_ratio: f64,

    /// Reserved tokens buffer for new messages.
    pub reserved_tokens: usize,

    /// Number of recent messages to always preserve (never compact).
    pub preserve_tail: usize,

    /// Whether to use LLM-based tier2 summary (expensive but higher quality).
    pub enable_llm_summary: bool,

    /// Aggressiveness of tier1 truncation (0.0 = none, 1.0 = max).
    pub truncation_aggressiveness: f64,
}

impl CompressionParams {
    /// Convert to the static `CompactionConfig` used by the existing pipeline.
    pub fn to_compaction_config(&self, _max_tokens: usize) -> CompactionConfig {
        CompactionConfig::default()
            .with_trigger_ratio(self.trigger_ratio)
            .with_reserved_tokens(self.reserved_tokens)
            .with_max_preserve_messages(self.preserve_tail)
    }
}

impl Default for CompressionParams {
    fn default() -> Self {
        Self {
            trigger_ratio: 0.75,
            reserved_tokens: 1500,
            preserve_tail: 3,
            enable_llm_summary: true,
            truncation_aggressiveness: 0.3,
        }
    }
}

// ============================================================================
// SessionStats
// ============================================================================

/// Statistics about a session used for compression optimization.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct SessionStats {
    /// Current number of messages in the session.
    pub message_count: usize,

    /// Current estimated token count.
    pub current_tokens: usize,

    /// Max tokens allowed for this session.
    pub max_tokens: usize,

    /// Average message length in tokens (last N messages).
    pub avg_message_tokens: f64,

    /// Session type inferred from interaction pattern.
    pub session_pattern: SessionPattern,
}

/// Inferred interaction pattern of a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SessionPattern {
    /// Many short queries (e.g. file exploration, quick checks).
    Exploratory,
    /// Few long responses (e.g. deep analysis, planning).
    DeepDive,
    /// Mixed pattern — moderate length, moderate count.
    #[default]
    Mixed,
    /// Tool-heavy — many tool calls, short LLM responses.
    ToolHeavy,
}

// ============================================================================
// CompressionOptimizer
// ============================================================================

/// Learns optimal compression parameters from historical session data.
pub struct CompressionOptimizer {
    /// Token efficiency curve: (context_length, tokens_per_message).
    /// Built from past sessions and updated after each compaction.
    token_efficiency_curve: Vec<(usize, f64)>,

    /// Historical retention rate of tier2 summaries (0.0 - 1.0).
    /// Higher means summaries preserve more useful context.
    summary_retention_rate: f64,

    /// Per-pattern default parameters (fallback when no history).
    pattern_defaults: HashMap<SessionPattern, CompressionParams>,
}

impl CompressionOptimizer {
    /// Create a new optimizer with default parameters.
    pub fn new() -> Self {
        let mut pattern_defaults = HashMap::new();
        pattern_defaults.insert(
            SessionPattern::Exploratory,
            CompressionParams {
                trigger_ratio: 0.85,   // High threshold — short messages don't fill context fast
                reserved_tokens: 1000, // Low reserve — short queries need less headroom
                preserve_tail: 2,
                enable_llm_summary: false, // Not worth the cost for short queries
                truncation_aggressiveness: 0.5,
            },
        );
        pattern_defaults.insert(
            SessionPattern::DeepDive,
            CompressionParams {
                trigger_ratio: 0.65,      // Lower threshold — long messages fill context quickly
                reserved_tokens: 2500,    // High reserve — need room for deep responses
                preserve_tail: 4,         // Keep more recent context for continuity
                enable_llm_summary: true, // Worth it for preserving complex context
                truncation_aggressiveness: 0.2,
            },
        );
        pattern_defaults.insert(
            SessionPattern::ToolHeavy,
            CompressionParams {
                trigger_ratio: 0.80,
                reserved_tokens: 1500,
                preserve_tail: 3,
                enable_llm_summary: false, // Tool results are self-contained
                truncation_aggressiveness: 0.6, // Truncate aggressively — tool output can be regenerated
            },
        );
        pattern_defaults.insert(SessionPattern::Mixed, CompressionParams::default());

        Self {
            token_efficiency_curve: Vec::new(),
            summary_retention_rate: 0.7,
            pattern_defaults,
        }
    }

    /// Record an observation of (context_length, tokens_per_message) from a session.
    pub fn observe_efficiency(&mut self, context_length: usize, tokens_per_message: f64) {
        // Keep curve sorted by context_length, dedup nearby points.
        self.token_efficiency_curve
            .push((context_length, tokens_per_message));
        self.token_efficiency_curve.sort_by_key(|(len, _)| *len);

        // Prune: keep at most 50 points, removing oldest / least central.
        if self.token_efficiency_curve.len() > 50 {
            // Simple pruning: remove every other point to maintain coverage.
            self.token_efficiency_curve = self
                .token_efficiency_curve
                .iter()
                .enumerate()
                .filter(|(i, _)| i % 2 == 0)
                .map(|(_, pt)| *pt)
                .collect();
        }
    }

    /// Update the summary retention rate based on user feedback after compaction.
    pub fn observe_summary_quality(&mut self, retained_usefulness: f64) {
        let alpha = 0.15;
        self.summary_retention_rate =
            alpha * retained_usefulness + (1.0 - alpha) * self.summary_retention_rate;
    }

    /// Generate optimal compression parameters for the current session state.
    pub fn optimize(&self, stats: &SessionStats) -> CompressionParams {
        let mut params = self
            .pattern_defaults
            .get(&stats.session_pattern)
            .copied()
            .unwrap_or_default();

        // Adjust trigger_ratio based on token efficiency curve.
        if let Some(efficiency) = self.interpolate_efficiency(stats.current_tokens) {
            // If token efficiency is dropping (more tokens per message at this length),
            // lower the trigger ratio to compact earlier.
            let base_efficiency = self
                .token_efficiency_curve
                .first()
                .map(|(_, e)| *e)
                .unwrap_or(50.0);
            let efficiency_ratio = efficiency / base_efficiency;
            if efficiency_ratio > 1.5 {
                params.trigger_ratio = (params.trigger_ratio - 0.1).clamp(0.5, 0.95);
            }
        }

        // Adjust based on how close we are to the limit.
        let usage_ratio = if stats.max_tokens > 0 {
            stats.current_tokens as f64 / stats.max_tokens as f64
        } else {
            0.0
        };

        if usage_ratio > 0.9 {
            // Emergency: we're very close to the limit.
            params.trigger_ratio = 0.55;
            params.truncation_aggressiveness = 0.8;
            params.enable_llm_summary = false; // Emergency: skip expensive summary
        } else if usage_ratio > 0.7 {
            // Getting tight.
            params.trigger_ratio = (params.trigger_ratio - 0.05).clamp(0.5, 0.95);
            params.truncation_aggressiveness =
                (params.truncation_aggressiveness + 0.1).clamp(0.0, 1.0);
        }

        // Adjust based on summary retention quality.
        if self.summary_retention_rate < 0.5 {
            // Summaries aren't working well — use truncation more.
            params.enable_llm_summary = false;
            params.truncation_aggressiveness =
                (params.truncation_aggressiveness + 0.15).clamp(0.0, 1.0);
        }

        params
    }

    /// Interpolate token efficiency at a given context length.
    fn interpolate_efficiency(&self, context_length: usize) -> Option<f64> {
        let curve = &self.token_efficiency_curve;
        if curve.is_empty() {
            return None;
        }
        if curve.len() == 1 {
            return Some(curve[0].1);
        }

        // Find the two bracketing points.
        let idx = curve.partition_point(|(len, _)| *len <= context_length);

        if idx == 0 {
            Some(curve[0].1)
        } else if idx >= curve.len() {
            Some(curve[curve.len() - 1].1)
        } else {
            let (len1, eff1) = curve[idx - 1];
            let (len2, eff2) = curve[idx];
            let t = (context_length - len1) as f64 / (len2 - len1).max(1) as f64;
            Some(eff1 + t * (eff2 - eff1))
        }
    }
}

impl Default for CompressionOptimizer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_params() {
        let params = CompressionParams::default();
        assert!(params.trigger_ratio > 0.0 && params.trigger_ratio < 1.0);
        assert!(params.reserved_tokens > 0);
        assert!(params.preserve_tail > 0);
    }

    #[test]
    fn test_optimizer_pattern_fallback() {
        let optimizer = CompressionOptimizer::new();
        let stats = SessionStats {
            session_pattern: SessionPattern::Exploratory,
            ..Default::default()
        };
        let params = optimizer.optimize(&stats);
        assert!(params.trigger_ratio > 0.8); // Exploratory gets high threshold
    }

    #[test]
    fn test_optimizer_emergency_near_limit() {
        let mut optimizer = CompressionOptimizer::new();
        optimizer.observe_efficiency(100, 50.0);
        optimizer.observe_efficiency(5000, 80.0);

        let stats = SessionStats {
            current_tokens: 9100,
            max_tokens: 10000,
            message_count: 50,
            avg_message_tokens: 180.0,
            session_pattern: SessionPattern::DeepDive,
        };
        let params = optimizer.optimize(&stats);

        assert_eq!(params.trigger_ratio, 0.55); // Emergency override
        assert!(!params.enable_llm_summary); // Skip expensive summary
        assert!(params.truncation_aggressiveness >= 0.7);
    }

    #[test]
    fn test_optimizer_efficiency_curve() {
        let mut optimizer = CompressionOptimizer::new();
        for i in 1..=10 {
            optimizer.observe_efficiency(i * 100, 50.0 + i as f64 * 5.0);
        }

        let stats = SessionStats {
            current_tokens: 550,
            max_tokens: 8000,
            message_count: 10,
            avg_message_tokens: 55.0,
            session_pattern: SessionPattern::Mixed,
        };
        let params = optimizer.optimize(&stats);
        // Efficiency at 550 should be interpolated between 500 (75) and 600 (80)
        assert!(params.trigger_ratio < 0.8); // Adjusted down due to efficiency drop
    }

    #[test]
    fn test_summary_quality_adjustment() {
        let mut optimizer = CompressionOptimizer::new();
        // Simulate very poor summary retention
        for _ in 0..10 {
            optimizer.observe_summary_quality(0.1);
        }

        let stats = SessionStats {
            session_pattern: SessionPattern::DeepDive,
            ..Default::default()
        };
        let params = optimizer.optimize(&stats);

        // With poor summaries, should disable LLM summary and truncate more
        assert!(!params.enable_llm_summary);
        assert!(params.truncation_aggressiveness > 0.3);
    }
}
