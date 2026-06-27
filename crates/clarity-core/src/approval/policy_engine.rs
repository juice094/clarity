//! Team-level permission policy engine (Phase 8).
//!
//! Evaluates [`TeamPolicy`] against tool calls to produce an [`AuthDecision`].
//! Ponytail: simple prefix-glob matching, no regex dependency, O(n) per check.

use clarity_contract::{AuthDecision, PolicyRiskLevel, TeamPolicy, TeamRole};

/// Classify a tool by name into a risk level.
///
/// Must stay in sync with `RuleEngine::with_defaults()` in `approval/rules.rs`.
pub fn classify_risk(tool_name: &str) -> PolicyRiskLevel {
    if tool_name.starts_with("shell")
        || tool_name.starts_with("bash")
        || tool_name.starts_with("powershell")
    {
        PolicyRiskLevel::High
    } else if tool_name.starts_with("file_write") || tool_name.starts_with("file_edit") {
        PolicyRiskLevel::Medium
    } else if tool_name.starts_with("file_read")
        || tool_name == "web_search"
        || tool_name == "webfetch"
        || tool_name == "web_fetch"
    {
        PolicyRiskLevel::Low
    } else {
        // ponytail: unknown tools default to Low
        PolicyRiskLevel::Low
    }
}

/// Check if a tool name matches a glob pattern.
///
/// Ponytail: only supports `*` (match all) and `prefix*` (prefix match).
/// No regex, no brace expansion, no character classes.
fn tool_matches(tool_name: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        tool_name.starts_with(prefix)
    } else {
        tool_name == pattern
    }
}

/// Authorize a tool call against a team policy.
///
/// Returns [`AuthDecision::Allow`] if the tool is permitted without approval,
/// [`AuthDecision::RequireApproval`] if the tool is allowed but needs approval,
/// or [`AuthDecision::Deny`] if the tool is not in the allowed list.
pub fn authorize(policy: &TeamPolicy, member_role: &TeamRole, tool_name: &str) -> AuthDecision {
    let perm = policy.resolve(member_role);

    // Check tool whitelist.
    let allowed = perm
        .allowed_tools
        .iter()
        .any(|p| tool_matches(tool_name, p));
    if !allowed {
        return AuthDecision::Deny(format!("Tool '{tool_name}' is not allowed by team policy"));
    }

    // Check risk level.
    let risk = classify_risk(tool_name);
    if risk > perm.max_risk_level {
        return AuthDecision::RequireApproval;
    }

    // Check global require_approval flag.
    if perm.require_approval {
        return AuthDecision::RequireApproval;
    }

    AuthDecision::Allow
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::{FunctionCall, PermissionPolicy};
    use std::collections::HashMap;

    fn make_tool_call(name: &str) -> clarity_contract::ToolCall {
        clarity_contract::ToolCall {
            id: "tc-1".into(),
            call_type: "function".into(),
            function: FunctionCall {
                name: name.into(),
                arguments: "{}".into(),
            },
        }
    }

    #[test]
    fn permissive_policy_allows_all() {
        let policy = TeamPolicy {
            team_id: "t1".into(),
            default_role_policy: PermissionPolicy::permissive(),
            role_policies: HashMap::new(),
        };
        assert_eq!(
            authorize(&policy, &TeamRole::Member, "bash"),
            AuthDecision::Allow
        );
        assert_eq!(
            authorize(&policy, &TeamRole::Member, "web_search"),
            AuthDecision::Allow
        );
    }

    #[test]
    fn read_only_denies_bash() {
        let policy = TeamPolicy {
            team_id: "t1".into(),
            default_role_policy: PermissionPolicy::read_only(),
            role_policies: HashMap::new(),
        };
        let result = authorize(&policy, &TeamRole::Member, "bash");
        assert!(matches!(result, AuthDecision::Deny(_)));
    }

    #[test]
    fn read_only_allows_file_read() {
        let policy = TeamPolicy {
            team_id: "t1".into(),
            default_role_policy: PermissionPolicy::read_only(),
            role_policies: HashMap::new(),
        };
        assert_eq!(
            authorize(&policy, &TeamRole::Member, "file_read"),
            AuthDecision::RequireApproval // read_only has require_approval: true
        );
    }

    #[test]
    fn risk_gating_requires_approval() {
        let mut perm = PermissionPolicy::permissive();
        perm.max_risk_level = PolicyRiskLevel::Low;
        let policy = TeamPolicy {
            team_id: "t1".into(),
            default_role_policy: perm,
            role_policies: HashMap::new(),
        };
        assert_eq!(
            authorize(&policy, &TeamRole::Member, "bash"),
            AuthDecision::RequireApproval
        );
    }

    #[test]
    fn role_specific_policy_overrides_default() {
        let mut role_policies = HashMap::new();
        role_policies.insert(TeamRole::Admin, PermissionPolicy::permissive());
        let policy = TeamPolicy {
            team_id: "t1".into(),
            default_role_policy: PermissionPolicy::read_only(),
            role_policies,
        };
        // Admin uses permissive → Allow
        assert_eq!(
            authorize(&policy, &TeamRole::Admin, "bash"),
            AuthDecision::Allow
        );
        // Member uses default read_only → Deny
        assert!(matches!(
            authorize(&policy, &TeamRole::Member, "bash"),
            AuthDecision::Deny(_)
        ));
    }

    #[test]
    fn wildcard_pattern_matches_everything() {
        assert!(tool_matches("any_tool_name", "*"));
        assert!(tool_matches("bash", "*"));
    }

    #[test]
    fn prefix_glob_matches() {
        assert!(tool_matches("file_read_text", "file_*"));
        assert!(!tool_matches("bash", "file_*"));
    }

    #[test]
    fn exact_match() {
        assert!(tool_matches("bash", "bash"));
        assert!(!tool_matches("bash", "shell"));
    }

    #[test]
    fn policy_risk_level_ordering() {
        assert!(PolicyRiskLevel::High > PolicyRiskLevel::Medium);
        assert!(PolicyRiskLevel::Medium > PolicyRiskLevel::Low);
        assert!(PolicyRiskLevel::Low > PolicyRiskLevel::Auto);
    }

    #[test]
    fn classify_risk_high() {
        assert_eq!(classify_risk("bash"), PolicyRiskLevel::High);
        assert_eq!(classify_risk("powershell"), PolicyRiskLevel::High);
        assert_eq!(classify_risk("shell_cmd"), PolicyRiskLevel::High);
    }

    #[test]
    fn classify_risk_medium() {
        assert_eq!(classify_risk("file_write"), PolicyRiskLevel::Medium);
        assert_eq!(classify_risk("file_edit_content"), PolicyRiskLevel::Medium);
    }

    #[test]
    fn classify_risk_low() {
        assert_eq!(classify_risk("file_read"), PolicyRiskLevel::Low);
        assert_eq!(classify_risk("web_search"), PolicyRiskLevel::Low);
        assert_eq!(classify_risk("unknown_tool"), PolicyRiskLevel::Low);
    }
}
