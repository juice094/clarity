//! Behavior pattern predictor — lightweight statistical forecasting for agent behavior.
//!
//! This module provides time-series prediction utilities used by the adaptive
//! engine to forecast:
//! - Token usage growth (for proactive compaction scheduling)
//! - User activity patterns (for pre-loading preferred models)
//! - Tool call frequency (for warming caches)
//!
//! It reuses the statistical foundation from `clarity_core::agent::jumpy::predictor`
//! but strips the LLM-augmented simulation layer, focusing on pure historical
//! pattern matching for fast, deterministic predictions.

use std::collections::HashMap;

use chrono::{DateTime, Timelike, Utc};

// ============================================================================
// WindowedStats
// ============================================================================

/// Sliding-window statistics for a single time series.
///
/// Maintains a fixed-size window of recent observations and provides
/// EWMA, trend slope, and simple linear extrapolation.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowedStats {
    /// Maximum number of observations to retain.
    window_size: usize,
    /// Observations as (timestamp, value) pairs, ordered oldest → newest.
    observations: Vec<(DateTime<Utc>, f64)>,
    /// EWMA alpha.
    alpha: f64,
}

impl WindowedStats {
    /// Create a new windowed stats tracker.
    pub fn new(window_size: usize) -> Self {
        Self {
            window_size,
            observations: Vec::with_capacity(window_size),
            alpha: 0.3,
        }
    }

    /// Record a new observation.
    pub fn observe(&mut self, timestamp: DateTime<Utc>, value: f64) {
        self.observations.push((timestamp, value));
        if self.observations.len() > self.window_size {
            self.observations.remove(0);
        }
    }

    /// Compute EWMA of the values.
    pub fn ewma(&self) -> Option<f64> {
        let mut iter = self.observations.iter().map(|(_, v)| *v);
        let first = iter.next()?;
        Some(iter.fold(first, |acc, v| self.alpha * v + (1.0 - self.alpha) * acc))
    }

    /// Compute simple linear trend (slope) over the window.
    ///
    /// Uses least-squares on timestamp-indexed observations.
    pub fn trend_slope(&self) -> Option<f64> {
        let n = self.observations.len();
        if n < 2 {
            return None;
        }

        let x: Vec<f64> = self
            .observations
            .iter()
            .enumerate()
            .map(|(i, _)| i as f64)
            .collect();
        let y: Vec<f64> = self.observations.iter().map(|(_, v)| *v).collect();

        let x_mean = x.iter().sum::<f64>() / n as f64;
        let y_mean = y.iter().sum::<f64>() / n as f64;

        let num: f64 = x
            .iter()
            .zip(&y)
            .map(|(xi, yi)| (xi - x_mean) * (yi - y_mean))
            .sum();
        let den: f64 = x.iter().map(|xi| (xi - x_mean).powi(2)).sum();

        if den.abs() < f64::EPSILON {
            Some(0.0)
        } else {
            Some(num / den)
        }
    }

    /// Predict the value at a future index (simple linear extrapolation).
    pub fn predict(&self, steps_ahead: usize) -> Option<f64> {
        let ewma = self.ewma()?;
        let slope = self.trend_slope()?;
        Some(ewma + slope * steps_ahead as f64)
    }

    /// Number of observations in the window.
    pub fn len(&self) -> usize {
        self.observations.len()
    }

    /// Whether the window is empty.
    pub fn is_empty(&self) -> bool {
        self.observations.is_empty()
    }
}

impl Default for WindowedStats {
    fn default() -> Self {
        Self::new(30)
    }
}

// ============================================================================
// TaskPattern
// ============================================================================

/// A recognized recurring pattern in task execution.
#[derive(Debug, Clone, PartialEq)]
pub struct TaskPattern {
    /// Pattern identifier (e.g. "morning-coding-burst", "evening-plan").
    pub id: String,

    /// Hour of day (0-23) when this pattern typically starts.
    pub typical_hour: u8,

    /// Typical duration in minutes.
    pub typical_duration_min: u32,

    /// Most common task types in this pattern.
    pub dominant_tasks: Vec<String>,

    /// Preferred provider for this pattern.
    pub preferred_provider: Option<String>,

    /// Confidence score (0.0 - 1.0) based on historical consistency.
    pub confidence: f64,
}

// ============================================================================
// BehaviorPredictor
// ============================================================================

/// Lightweight predictor that matches current state against historical patterns.
pub struct BehaviorPredictor {
    /// Per-metric time series (metric_name → windowed stats).
    series: HashMap<String, WindowedStats>,

    /// Detected recurring patterns.
    patterns: Vec<TaskPattern>,
}

impl BehaviorPredictor {
    /// Create a new predictor.
    pub fn new() -> Self {
        Self {
            series: HashMap::new(),
            patterns: Vec::new(),
        }
    }

    /// Observe a metric value.
    pub fn observe(
        &mut self,
        metric_name: impl Into<String>,
        timestamp: DateTime<Utc>,
        value: f64,
    ) {
        self.series
            .entry(metric_name.into())
            .or_default()
            .observe(timestamp, value);
    }

    /// Predict the next value of a metric.
    pub fn predict(&self, metric_name: &str, steps_ahead: usize) -> Option<f64> {
        self.series.get(metric_name)?.predict(steps_ahead)
    }

    /// Get EWMA of a metric.
    pub fn ewma(&self, metric_name: &str) -> Option<f64> {
        self.series.get(metric_name)?.ewma()
    }

    /// Add a recognized pattern.
    pub fn add_pattern(&mut self, pattern: TaskPattern) {
        self.patterns.push(pattern);
    }

    /// Find patterns that are likely active now.
    pub fn active_patterns(&self, now: DateTime<Utc>) -> Vec<&TaskPattern> {
        let current_hour = now.hour() as u8;
        self.patterns
            .iter()
            .filter(|p| {
                // Match if current hour is within ±1 hour of typical hour.
                let hour_diff = current_hour.abs_diff(p.typical_hour);
                hour_diff <= 1 && p.confidence > 0.5
            })
            .collect()
    }

    /// Predict token usage after N more messages based on current trend.
    pub fn predict_token_usage(
        &self,
        current_tokens: usize,
        messages_to_send: usize,
    ) -> Option<usize> {
        let tokens_per_msg = self.ewma("tokens_per_message")?;
        let slope = self.trend_slope("tokens_per_message").unwrap_or(0.0);
        let predicted_per_msg = tokens_per_msg + slope * messages_to_send as f64;
        let predicted_total = current_tokens as f64 + predicted_per_msg * messages_to_send as f64;
        Some(predicted_total.max(0.0) as usize)
    }

    fn trend_slope(&self, metric_name: &str) -> Option<f64> {
        self.series.get(metric_name)?.trend_slope()
    }
}

impl Default for BehaviorPredictor {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use chrono::Duration;

    use super::*;

    #[test]
    fn test_windowed_stats_ewma() {
        let mut stats = WindowedStats::new(10);
        stats.observe(Utc::now(), 10.0);
        stats.observe(Utc::now(), 20.0);
        // EWMA: 0.3 * 20 + 0.7 * 10 = 13
        assert!((stats.ewma().unwrap() - 13.0).abs() < 0.01);
    }

    #[test]
    fn test_windowed_stats_trend() {
        let mut stats = WindowedStats::new(10);
        let base = Utc::now();
        for i in 0..5 {
            stats.observe(base + Duration::seconds(i), i as f64 * 10.0);
        }
        // Linear trend: slope should be ~10
        let slope = stats.trend_slope().unwrap();
        assert!((slope - 10.0).abs() < 0.01);
    }

    #[test]
    fn test_windowed_stats_predict() {
        let mut stats = WindowedStats::new(10);
        let base = Utc::now();
        for i in 0..5 {
            stats.observe(base + Duration::seconds(i), i as f64 * 10.0);
        }
        // Predict 2 steps ahead from EWMA + slope
        let pred = stats.predict(2).unwrap();
        assert!(pred > 30.0); // Should be growing
    }

    #[test]
    fn test_behavior_predictor_token_usage() {
        let mut predictor = BehaviorPredictor::new();
        let base = Utc::now();
        for i in 0..10 {
            predictor.observe(
                "tokens_per_message",
                base + Duration::minutes(i),
                100.0 + i as f64 * 5.0,
            );
        }

        let usage = predictor.predict_token_usage(1000, 5);
        assert!(usage.is_some());
        assert!(usage.unwrap() > 1000); // Growing trend
    }

    #[test]
    fn test_active_patterns() {
        let mut predictor = BehaviorPredictor::new();
        predictor.add_pattern(TaskPattern {
            id: "morning-code".to_string(),
            typical_hour: 9,
            typical_duration_min: 60,
            dominant_tasks: vec!["coding".to_string()],
            preferred_provider: Some("kimi-coding".to_string()),
            confidence: 0.8,
        });

        // Simulate 9 AM
        let morning = Utc::now().with_hour(9).unwrap().with_minute(0).unwrap();
        let active = predictor.active_patterns(morning);
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, "morning-code");

        // Simulate 3 AM (no match)
        let night = Utc::now().with_hour(3).unwrap().with_minute(0).unwrap();
        let active = predictor.active_patterns(night);
        assert!(active.is_empty());
    }

    #[test]
    fn test_predictor_empty() {
        let predictor = BehaviorPredictor::new();
        assert!(predictor.ewma("unknown").is_none());
        assert!(predictor.predict("unknown", 1).is_none());
    }
}
