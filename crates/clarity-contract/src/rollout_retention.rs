//! Rollout retention policy — prevents unbounded JSONL growth.
//!
//! Follows syncthing-rust's `syncthing-versioner/src/simple.rs` +
//! `staggered.rs` pattern of age-gated density-aware file retention.
//!
//! # Policies
//!
//! - **CountBased**: keep at most N sessions per thread.
//! - **AgeBased**: keep sessions from the last N days, prune older ones.
//! - **Staggered**: keep one per hour (last 24h), one per day (last 30d),
//!   one per week beyond. Maximum density where recent history matters.
//! - **Unlimited**: never prune (current behavior).

use serde::{Deserialize, Serialize};
use std::path::Path;

/// Retention policy for rollout session files.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub enum RetentionPolicy {
    /// Keep at most N sessions per thread.
    CountBased {
        /// Maximum number of sessions to retain.
        max_sessions: u32,
    },
    /// Keep sessions from the last N days.
    AgeBased {
        /// Maximum age of sessions in days.
        max_age_days: u32,
    },
    /// Staggered retention: one per hour (last 24h), one per day (last 30d),
    /// one per week (beyond 30d).
    #[default]
    Staggered,
    /// Never prune any sessions (current default).
    Unlimited,
}

impl RetentionPolicy {
    /// Determine which session files to delete based on this policy.
    ///
    /// `sessions` is a list of (session_timestamp_secs, file_path) tuples.
    /// Returns the list of files that should be deleted.
    /// The active (most recent) session is never pruned.
    pub fn select_for_pruning(&self, sessions: &[(u64, String)]) -> Vec<String> {
        if sessions.is_empty() {
            return Vec::new();
        }

        // Sort by timestamp descending (newest first).
        let mut sorted: Vec<&(u64, String)> = sessions.iter().collect();
        sorted.sort_by_key(|b| std::cmp::Reverse(b.0));

        match self {
            RetentionPolicy::CountBased { max_sessions } => {
                let keep = *max_sessions as usize;
                if sorted.len() <= keep {
                    return Vec::new();
                }
                sorted[keep..]
                    .iter()
                    .map(|(_, path)| path.clone())
                    .collect()
            }
            RetentionPolicy::AgeBased { max_age_days } => {
                let now = now_secs();
                let cutoff = now.saturating_sub((*max_age_days as u64) * 86400);
                sorted
                    .iter()
                    .filter(|(ts, _)| *ts < cutoff)
                    .map(|(_, path)| path.clone())
                    .collect()
            }
            RetentionPolicy::Staggered => staggered_prune(&sorted),
            RetentionPolicy::Unlimited => Vec::new(),
        }
    }
}

/// Staggered pruning: keep one per hour (last 24h), one per day (last 30d),
/// one per week beyond 30d.
fn staggered_prune(sessions: &[&(u64, String)]) -> Vec<String> {
    let now = now_secs();
    staggered_prune_at(sessions, now)
}

/// Staggered pruning at a specific reference time (for test determinism).
fn staggered_prune_at(sessions: &[&(u64, String)], now: u64) -> Vec<String> {
    let hour = 3600u64;
    let day = 86400u64;
    let week = 604800u64;

    let mut keep_flags: Vec<bool> = vec![false; sessions.len()];
    let mut last_hour_bucket = 0u64;
    let mut last_day_bucket = 0u64;
    let mut last_week_bucket = 0u64;

    // Always keep the newest session and seed the bucket trackers.
    if let Some((newest_ts, _)) = sessions.first() {
        keep_flags[0] = true;
        let age = now.saturating_sub(*newest_ts);
        if age <= 24 * hour {
            last_hour_bucket = *newest_ts / hour;
        } else if age <= 30 * day {
            last_day_bucket = *newest_ts / day;
        } else {
            last_week_bucket = *newest_ts / week;
        }
    }

    for (i, (ts, _)) in sessions.iter().enumerate() {
        if keep_flags[i] {
            continue;
        }
        let age = now.saturating_sub(*ts);

        if age <= 24 * hour {
            let bucket = *ts / hour;
            if bucket != last_hour_bucket {
                keep_flags[i] = true;
                last_hour_bucket = bucket;
            }
        } else if age <= 30 * day {
            let bucket = *ts / day;
            if bucket != last_day_bucket {
                keep_flags[i] = true;
                last_day_bucket = bucket;
            }
        } else {
            let bucket = *ts / week;
            if bucket != last_week_bucket {
                keep_flags[i] = true;
                last_week_bucket = bucket;
            }
        }
    }

    sessions
        .iter()
        .enumerate()
        .filter(|(i, _)| !keep_flags[*i])
        .map(|(_, (_, path))| path.clone())
        .collect()
}

/// Get current Unix timestamp in seconds.
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

/// Delete rollout files matching the given paths.
///
/// Returns the count of successfully deleted files.
pub fn prune_files(rollout_dir: &Path, paths: &[String]) -> usize {
    let mut deleted = 0usize;
    for path in paths {
        let full = rollout_dir.join(path);
        match std::fs::remove_file(&full) {
            Ok(()) => {
                tracing::debug!(path = %full.display(), "Pruned rollout file");
                deleted += 1;
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Already gone — not an error.
            }
            Err(e) => {
                tracing::warn!(path = %full.display(), error = %e, "Failed to prune rollout file");
            }
        }
    }
    deleted
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_based_keeps_newest() {
        let policy = RetentionPolicy::CountBased { max_sessions: 2 };
        let sessions = vec![
            (1000, "old.jsonl".into()),
            (2000, "mid.jsonl".into()),
            (3000, "new.jsonl".into()),
        ];
        let prune = policy.select_for_pruning(&sessions);
        assert_eq!(prune.len(), 1);
        assert!(prune.contains(&"old.jsonl".to_string()));
    }

    #[test]
    fn test_count_based_none_when_under_limit() {
        let policy = RetentionPolicy::CountBased { max_sessions: 5 };
        let sessions = vec![(1000, "a.jsonl".into()), (2000, "b.jsonl".into())];
        let prune = policy.select_for_pruning(&sessions);
        assert!(prune.is_empty());
    }

    #[test]
    fn test_age_based_prunes_old() {
        let policy = RetentionPolicy::AgeBased { max_age_days: 1 };
        let now = now_secs();
        let old = now - 2 * 86400; // 2 days ago
        let recent = now - 3600; // 1 hour ago
        let sessions = vec![(old, "old.jsonl".into()), (recent, "recent.jsonl".into())];
        let prune = policy.select_for_pruning(&sessions);
        assert_eq!(prune.len(), 1);
        assert!(prune.contains(&"old.jsonl".to_string()));
    }

    #[test]
    fn test_staggered_keeps_one_per_hour() {
        let hour = 3600u64;
        let now = 1000 * hour + 45 * 60;
        let newer = 1000 * hour + 30 * 60;
        let older = 1000 * hour + 10 * 60;
        let sessions = [
            (newer, "newer.jsonl".to_string()),
            (older, "older.jsonl".to_string()),
        ];
        let refs: Vec<&(u64, String)> = sessions.iter().collect();
        let prune = staggered_prune_at(&refs, now);
        assert_eq!(
            prune.len(),
            1,
            "expected 1 pruned, got {}: {:?}",
            prune.len(),
            prune
        );
    }

    #[test]
    fn test_staggered_keeps_different_hours() {
        let hour = 3600u64;
        let now = 1000 * hour + 45 * 60;
        let s1 = 1000 * hour + 30 * 60;
        let s2 = 999 * hour + 30 * 60;
        let sessions = [
            (s1, "hour1000.jsonl".to_string()),
            (s2, "hour999.jsonl".to_string()),
        ];
        let refs: Vec<&(u64, String)> = sessions.iter().collect();
        let prune = staggered_prune_at(&refs, now);
        assert_eq!(prune.len(), 0, "different hours should both be kept");
    }

    #[test]
    fn test_unlimited_never_prunes() {
        let sessions = vec![(1000, "a.jsonl".into())];
        let prune = RetentionPolicy::Unlimited.select_for_pruning(&sessions);
        assert!(prune.is_empty());
    }

    #[test]
    fn test_empty_sessions() {
        let prune = RetentionPolicy::Staggered.select_for_pruning(&[]);
        assert!(prune.is_empty());
    }
}
