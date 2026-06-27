//! Rule-based risk evaluation for tool call approval.
//!
//! Provides a lightweight, deterministic alternative to LLM-based classification
//! for the first iteration of the approval system.

use serde_json::Value;

/// Risk level assigned to a tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiskLevel {
    /// Automatically approved (Yolo-equivalent for this tool).
    Auto,
    /// Low risk — logged but no blocking approval required.
    Low,
    /// Medium risk — logged prominently, approval depends on mode.
    Medium,
    /// High risk — always requires explicit approval in Interactive mode.
    High,
}

/// A single approval rule.
#[derive(Debug, Clone)]
pub struct ApprovalRule {
    /// Tool name or glob pattern (e.g. "shell", "file_*").
    pub tool_pattern: String,
    /// Optional JSON-path-like condition on arguments.
    /// For now we only support exact key existence or value match.
    pub arg_condition: Option<ArgCondition>,
    /// Assigned risk when this rule matches.
    pub risk: RiskLevel,
}

/// Simple argument condition for rule matching.
#[derive(Debug, Clone)]
pub enum ArgCondition {
    /// Key must exist in the top-level args object.
    KeyExists(String),
    /// Key must equal the given string value.
    KeyEquals(String, String),
}

/// Engine that evaluates a set of rules against a tool call.
#[derive(Debug, Clone, Default)]
pub struct RuleEngine {
    rules: Vec<ApprovalRule>,
}

impl RuleEngine {
    /// Create an empty engine.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an engine with sensible built-in defaults.
    ///
    /// ponytail: hard-coded rule table. Move to a config-driven manifest
    /// (e.g. `approval-rules.toml`) when the rule count exceeds ~15 or when
    /// per-project overrides are required.
    pub fn with_defaults() -> Self {
        let mut engine = Self::new();
        engine.add_rule(ApprovalRule {
            tool_pattern: "shell".to_string(),
            arg_condition: None,
            risk: RiskLevel::High,
        });
        engine.add_rule(ApprovalRule {
            tool_pattern: "bash".to_string(),
            arg_condition: None,
            risk: RiskLevel::High,
        });
        engine.add_rule(ApprovalRule {
            tool_pattern: "powershell".to_string(),
            arg_condition: None,
            risk: RiskLevel::High,
        });
        engine.add_rule(ApprovalRule {
            tool_pattern: "web_browser".to_string(),
            arg_condition: None,
            risk: RiskLevel::High,
        });
        engine.add_rule(ApprovalRule {
            tool_pattern: "file_write".to_string(),
            arg_condition: None,
            risk: RiskLevel::Medium,
        });
        engine.add_rule(ApprovalRule {
            tool_pattern: "file_edit".to_string(),
            arg_condition: None,
            risk: RiskLevel::Medium,
        });
        engine.add_rule(ApprovalRule {
            tool_pattern: "file_read".to_string(),
            arg_condition: None,
            risk: RiskLevel::Low,
        });
        engine.add_rule(ApprovalRule {
            tool_pattern: "web_search".to_string(),
            arg_condition: None,
            risk: RiskLevel::Low,
        });
        engine.add_rule(ApprovalRule {
            tool_pattern: "webfetch".to_string(),
            arg_condition: None,
            risk: RiskLevel::Low,
        });
        engine
    }

    /// Add a custom rule.
    pub fn add_rule(&mut self, rule: ApprovalRule) {
        self.rules.push(rule);
    }

    /// Evaluate the risk of a tool call.
    ///
    /// Returns the risk of the *first* matching rule, or `RiskLevel::Low` if
    /// no rule matches.
    pub fn evaluate(&self, tool_name: &str, args: &Value) -> RiskLevel {
        for rule in &self.rules {
            if Self::tool_matches(&rule.tool_pattern, tool_name)
                && Self::args_match(&rule.arg_condition, args)
            {
                return rule.risk;
            }
        }
        RiskLevel::Low
    }

    fn tool_matches(pattern: &str, name: &str) -> bool {
        if pattern == name {
            return true;
        }
        // Simple glob: "file_*" matches "file_read", "file_write", etc.
        if let Some(prefix) = pattern.strip_suffix('*') {
            return name.starts_with(prefix);
        }
        false
    }

    fn args_match(condition: &Option<ArgCondition>, args: &Value) -> bool {
        let Some(cond) = condition else {
            return true;
        };
        match cond {
            ArgCondition::KeyExists(key) => args.get(key).is_some(),
            ArgCondition::KeyEquals(key, val) => args
                .get(key)
                .and_then(|v| v.as_str())
                .map(|s| s == val)
                .unwrap_or(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_shell_high() {
        let engine = RuleEngine::with_defaults();
        assert_eq!(
            engine.evaluate("shell", &serde_json::json!({})),
            RiskLevel::High
        );
    }

    #[test]
    fn test_default_file_read_low() {
        let engine = RuleEngine::with_defaults();
        assert_eq!(
            engine.evaluate("file_read", &serde_json::json!({})),
            RiskLevel::Low
        );
    }

    #[test]
    fn test_default_file_write_medium() {
        let engine = RuleEngine::with_defaults();
        assert_eq!(
            engine.evaluate("file_write", &serde_json::json!({})),
            RiskLevel::Medium
        );
    }

    #[test]
    fn test_unknown_tool_fallback_low() {
        let engine = RuleEngine::with_defaults();
        assert_eq!(
            engine.evaluate("unknown", &serde_json::json!({})),
            RiskLevel::Low
        );
    }

    #[test]
    fn test_glob_pattern() {
        let mut engine = RuleEngine::new();
        engine.add_rule(ApprovalRule {
            tool_pattern: "foo_*".to_string(),
            arg_condition: None,
            risk: RiskLevel::Medium,
        });
        assert_eq!(
            engine.evaluate("foo_bar", &serde_json::json!({})),
            RiskLevel::Medium
        );
        assert_eq!(
            engine.evaluate("foo", &serde_json::json!({})),
            RiskLevel::Low
        );
    }

    #[test]
    fn test_arg_condition_key_exists() {
        let mut engine = RuleEngine::new();
        engine.add_rule(ApprovalRule {
            tool_pattern: "test".to_string(),
            arg_condition: Some(ArgCondition::KeyExists("dangerous".to_string())),
            risk: RiskLevel::High,
        });
        assert_eq!(
            engine.evaluate("test", &serde_json::json!({ "dangerous": true })),
            RiskLevel::High
        );
        assert_eq!(
            engine.evaluate("test", &serde_json::json!({ "safe": true })),
            RiskLevel::Low
        );
    }

    #[test]
    fn test_arg_condition_key_equals() {
        let mut engine = RuleEngine::new();
        engine.add_rule(ApprovalRule {
            tool_pattern: "test".to_string(),
            arg_condition: Some(ArgCondition::KeyEquals(
                "mode".to_string(),
                "destructive".to_string(),
            )),
            risk: RiskLevel::High,
        });
        assert_eq!(
            engine.evaluate("test", &serde_json::json!({ "mode": "destructive" })),
            RiskLevel::High
        );
        assert_eq!(
            engine.evaluate("test", &serde_json::json!({ "mode": "safe" })),
            RiskLevel::Low
        );
    }
}
