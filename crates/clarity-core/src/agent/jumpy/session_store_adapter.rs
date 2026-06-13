// SPDX-License-Identifier: AGPL-3.0
// Copyright (C) 2026 juice094 and contributors

//! Session Store Adapter — converts `clarity-memory` session logs into
//! `SkillObservation` vectors for `HistoricalPredictor`.
//!
//! This module provides the "missing link" between historical conversation
//! sessions and the Jumpy world model: it reads raw session records,
//! detects skill boundaries heuristically, and emits structured
//! `(skill_id, params, before_state, after_state)` observations.

use super::predictor::SkillObservation;
use super::state::JumpyState;
use clarity_memory::types::SessionRecord;

/// Heuristic configuration for skill boundary detection.
#[derive(Debug, Clone)]
pub struct AdapterConfig {
    /// Known skill ids to look for inside message content.
    pub known_skills: Vec<String>,
    /// Fallback skill id when no known skill is detected.
    pub fallback_skill: String,
    /// Default params when none can be inferred.
    pub default_params: String,
    /// Max characters for context_summary.
    pub summary_max_len: usize,
}

impl Default for AdapterConfig {
    fn default() -> Self {
        Self {
            known_skills: Vec::new(),
            fallback_skill: "chat".to_string(),
            default_params: "default".to_string(),
            summary_max_len: 200,
        }
    }
}

/// Convert session records into a vector of skill observations.
///
/// Strategy:
/// 1. Flatten `SessionRecord`s into a linear sequence of turns (messages +
///    summaries).  Summaries are treated as `"summary"` role turns.
/// 2. Walk the turn sequence.  Whenever an `assistant` turn mentions a known
///    skill (or any `assistant` turn if no skill is matched), emit one
///    `SkillObservation`.
/// 3. `before` state is built from all turns *before* the current assistant
///    turn.  `after` state is built from all turns *up to and including* it.
/// 4. `progress` is estimated linearly by position in the session.
///
/// # Arguments
/// * `records` — session records from `SessionStore::get_all_records()`
/// * `config` — heuristic configuration (skill names, fallback, etc.)
pub fn session_to_observations(
    records: &[SessionRecord],
    config: &AdapterConfig,
) -> Vec<SkillObservation> {
    let turns = flatten_records(records);
    if turns.is_empty() {
        return Vec::new();
    }

    let total = turns.len();
    let mut observations = Vec::new();

    for i in 0..total {
        let turn = &turns[i];
        if turn.role != "assistant" {
            continue;
        }

        let (skill_id, params) = detect_skill(&turn.content, config);
        let before = build_state(&turns[..i], i, total, config.summary_max_len);
        let after = build_state(&turns[..=i], i + 1, total, config.summary_max_len);

        observations.push(SkillObservation {
            skill_id,
            params,
            before,
            after,
        });
    }

    observations
}

/// A unified turn, collapsed from both `SessionRecord::Message` and
/// `SessionRecord::Summary`.
#[derive(Debug, Clone)]
struct Turn {
    role: String,
    content: String,
}

fn flatten_records(records: &[SessionRecord]) -> Vec<Turn> {
    records
        .iter()
        .map(|r| match r {
            SessionRecord::Message { message, .. } => Turn {
                role: message.role.clone(),
                content: message.content.clone(),
            },
            SessionRecord::Summary { content, .. } => Turn {
                role: "summary".to_string(),
                content: content.clone(),
            },
        })
        .collect()
}

/// Detect which skill (if any) is referenced in the message content.
///
/// Returns `(skill_id, params)`.  If no known skill matches, falls back to
/// `config.fallback_skill` / `config.default_params`.
fn detect_skill(content: &str, config: &AdapterConfig) -> (String, String) {
    let lower = content.to_lowercase();
    for skill in &config.known_skills {
        if lower.contains(&skill.to_lowercase()) {
            return (skill.clone(), config.default_params.clone());
        }
    }
    (config.fallback_skill.clone(), config.default_params.clone())
}

/// Build a `JumpyState` snapshot from a slice of turns.
///
/// `position` is the current turn count (used for progress heuristics).
/// `total` is the total number of turns in the session.
fn build_state(
    turns: &[Turn],
    position: usize,
    total: usize,
    summary_max_len: usize,
) -> JumpyState {
    let mut tags = Vec::new();
    let mut memory = std::collections::HashMap::new();
    let mut active_files = Vec::new();

    for turn in turns {
        let role_tag = format!("role:{}", turn.role);
        if !tags.contains(&role_tag) {
            tags.push(role_tag);
        }

        let content_lower = turn.content.to_lowercase();
        let keywords = [
            ("error", "error"),
            ("fail", "failed"),
            ("success", "success"),
            ("done", "done"),
            ("fix", "fix"),
            ("build", "build"),
            ("test", "test"),
            ("pass", "tests-passing"),
            ("warning", "warning"),
        ];
        for (needle, tag) in keywords {
            let tag_str = tag.to_string();
            if content_lower.contains(needle) && !tags.contains(&tag_str) {
                tags.push(tag_str);
            }
        }

        for line in turn.content.lines() {
            if let Some((key, value)) = parse_kv(line) {
                memory.insert(key, value);
            }
            for path in extract_file_paths(line) {
                if !active_files.contains(&path) {
                    active_files.push(path);
                }
            }
        }
    }

    let context_summary = turns
        .last()
        .map(|t| t.content.chars().take(summary_max_len).collect())
        .unwrap_or_default();

    JumpyState {
        progress: if total > 0 {
            (position as f32 / total as f32).clamp(0.0, 1.0)
        } else {
            0.0
        },
        tags,
        memory,
        active_files,
        context_summary,
    }
}

/// Try to extract a `key: value` or `key = value` pair from a line.
fn parse_kv(line: &str) -> Option<(String, String)> {
    let line = line.trim();
    if let Some(pos) = line.find(':') {
        let key = line[..pos].trim();
        let value = line[pos + 1..].trim();
        if is_valid_key(key) && !value.is_empty() {
            return Some((key.to_string(), value.to_string()));
        }
    }
    if let Some(pos) = line.find('=') {
        let key = line[..pos].trim();
        let value = line[pos + 1..].trim();
        if is_valid_key(key) && !value.is_empty() {
            return Some((key.to_string(), value.to_string()));
        }
    }
    None
}

fn is_valid_key(key: &str) -> bool {
    !key.is_empty()
        && key
            .chars()
            .all(|c| c.is_alphanumeric() || c == '_' || c == '-')
        && key.len() <= 64
}

/// Naïve file-path extractor.  Returns all words that end with common
/// source-file extensions.
fn extract_file_paths(line: &str) -> Vec<String> {
    const EXTENSIONS: &[&str] = &[
        ".rs", ".toml", ".md", ".json", ".yaml", ".yml", ".py", ".js", ".ts", ".go", ".java",
        ".cpp", ".c", ".h", ".hpp", ".sh", ".ps1",
    ];

    let mut paths = Vec::new();
    for token in line.split_whitespace() {
        let token = token.trim_matches(|c: char| {
            c == '`'
                || c == '*'
                || c == '['
                || c == ']'
                || c == '('
                || c == ')'
                || c == '"'
                || c == '\''
        });
        if token.len() < 3 {
            continue;
        }
        for ext in EXTENSIONS {
            if token.ends_with(ext)
                && (token.contains('/') || token.contains('\\') || !token.contains(' '))
            {
                paths.push(token.to_string());
                break;
            }
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use clarity_memory::types::MessageRecord;

    fn msg(role: &str, content: &str) -> SessionRecord {
        SessionRecord::Message {
            message: MessageRecord {
                role: role.to_string(),
                content: content.to_string(),
            },
            timestamp: Utc::now(),
        }
    }

    fn summary(content: &str) -> SessionRecord {
        SessionRecord::Summary {
            content: content.to_string(),
            timestamp: Utc::now(),
        }
    }

    #[test]
    fn test_empty_session() {
        let records: Vec<SessionRecord> = vec![];
        let obs = session_to_observations(&records, &AdapterConfig::default());
        assert!(obs.is_empty());
    }

    #[test]
    fn test_single_assistant_message_becomes_chat_observation() {
        let records = vec![msg("user", "Hello"), msg("assistant", "Hi there!")];
        let obs = session_to_observations(&records, &AdapterConfig::default());
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].skill_id, "chat");
        assert_eq!(obs[0].params, "default");
        assert!(obs[0].before.tags.contains(&"role:user".to_string()));
        assert!(obs[0].after.tags.contains(&"role:assistant".to_string()));
    }

    #[test]
    fn test_known_skill_detection() {
        let records = vec![
            msg("user", "Deploy the service"),
            msg("assistant", "Using skill deploy-rust-service to deploy."),
        ];
        let config = AdapterConfig {
            known_skills: vec!["deploy-rust-service".to_string()],
            ..Default::default()
        };
        let obs = session_to_observations(&records, &config);
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].skill_id, "deploy-rust-service");
    }

    #[test]
    fn test_progress_heuristic() {
        let records = vec![
            msg("user", "Step 1"),
            msg("assistant", "Done step 1"),
            msg("user", "Step 2"),
            msg("assistant", "Done step 2"),
        ];
        let obs = session_to_observations(&records, &AdapterConfig::default());
        assert_eq!(obs.len(), 2);
        // First observation: after 2 turns out of 4 -> 0.5
        assert!((obs[0].after.progress - 0.5).abs() < f32::EPSILON);
        // Second observation: after 4 turns out of 4 -> 1.0
        assert!((obs[1].after.progress - 1.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_memory_extraction() {
        let records = vec![
            msg("user", "Set config"),
            msg("assistant", "model: gpt-4\ntemperature = 0.7"),
        ];
        let obs = session_to_observations(&records, &AdapterConfig::default());
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].after.memory.get("model"), Some(&"gpt-4".to_string()));
        assert_eq!(
            obs[0].after.memory.get("temperature"),
            Some(&"0.7".to_string())
        );
    }

    #[test]
    fn test_active_files_extraction() {
        let records = vec![
            msg("user", "Fix the bug"),
            msg("assistant", "Edited src/lib.rs and tests/test.rs"),
        ];
        let obs = session_to_observations(&records, &AdapterConfig::default());
        assert_eq!(obs.len(), 1);
        assert!(
            obs[0]
                .after
                .active_files
                .contains(&"src/lib.rs".to_string())
        );
        assert!(
            obs[0]
                .after
                .active_files
                .contains(&"tests/test.rs".to_string())
        );
    }

    #[test]
    fn test_context_summary_truncation() {
        let long = "a".repeat(500);
        let records = vec![msg("user", "Tell me a story"), msg("assistant", &long)];
        let obs = session_to_observations(&records, &AdapterConfig::default());
        assert_eq!(obs.len(), 1);
        assert_eq!(obs[0].after.context_summary.len(), 200);
    }

    #[test]
    fn test_summary_record_as_boundary() {
        let records = vec![
            msg("user", "Hello"),
            msg("assistant", "Hi"),
            summary("Session summary so far"),
            msg("user", "Next question"),
            msg("assistant", "Answer"),
        ];
        let obs = session_to_observations(&records, &AdapterConfig::default());
        assert_eq!(obs.len(), 2);
        // Second observation should include the summary turn
        assert!(obs[1].before.tags.contains(&"role:summary".to_string()));
    }

    #[test]
    fn test_before_after_states_differ() {
        let records = vec![
            msg("user", "Start task"),
            msg("assistant", "Task completed with success"),
        ];
        let obs = session_to_observations(&records, &AdapterConfig::default());
        assert_eq!(obs.len(), 1);
        // before should not have "success" tag (assistant message not included)
        assert!(!obs[0].before.tags.contains(&"success".to_string()));
        // after should have "success" tag (assistant message included)
        assert!(obs[0].after.tags.contains(&"success".to_string()));
    }

    #[test]
    fn test_multiple_assistant_messages() {
        let records = vec![
            msg("user", "Q1"),
            msg("assistant", "A1"),
            msg("user", "Q2"),
            msg("assistant", "A2"),
            msg("user", "Q3"),
            msg("assistant", "A3"),
        ];
        let obs = session_to_observations(&records, &AdapterConfig::default());
        assert_eq!(obs.len(), 3);
        // Each observation accumulates more state.
        // Tags are deduplicated by role, so the count does not grow linearly.
        assert_eq!(obs[0].before.tags.len(), 1); // role:user only
        assert_eq!(obs[1].before.tags.len(), 2); // role:user, role:assistant
        assert_eq!(obs[2].before.tags.len(), 2); // role:user, role:assistant
    }
}
