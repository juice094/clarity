//! Identity types for User, Team, and Organization.
//!
//! These types provide structured attribution for turns, sessions, and
//! approval decisions. All identity fields are optional at every layer so
//! that existing sessions without identity continue to work unchanged.
//!
//! Phase 7 (cross-device sync), Phase 8 (team permissions), and Phase 9
//! (enterprise channels) build on these types.

use serde::{Deserialize, Serialize};

// ============================================================================
// IdentityContext — lightweight identity payload threaded through a turn
// ============================================================================

/// Identity carried through a single turn.
///
/// Populated from [`AgentConfig`] at turn start and threaded into lifecycle
/// events, approval decisions, and rollout logs.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct IdentityContext {
    /// User identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_id: Option<String>,
    /// Team identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub team_id: Option<String>,
    /// Organization identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org_id: Option<String>,
}

// ============================================================================
// TeamRole
// ============================================================================

/// Role within a team or organization.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeamRole {
    /// Full administrative control.
    Owner,
    /// Can manage members and settings.
    Admin,
    /// Standard member with normal permissions.
    #[default]
    Member,
    /// Read-only access.
    Viewer,
}

// ============================================================================
// User
// ============================================================================

/// A Clarity user identity.
///
/// Users can authenticate via multiple providers (local, WeChat, GitHub, etc.).
/// The `provider` + `provider_user_id` pair uniquely identifies a user across
/// auth backends.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct User {
    /// Unique identifier (UUIDv7 or provider-derived).
    pub id: String,
    /// Display name.
    pub display_name: String,
    /// Optional avatar URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub avatar_url: Option<String>,
    /// Optional email address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    /// Auth provider: "local", "wechat", "github", etc.
    pub provider: String,
    /// Provider-specific user identifier (e.g., WeChat OpenID).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_user_id: Option<String>,
    /// Unix timestamp of creation (seconds).
    pub created_at: u64,
    /// Unix timestamp of last update (seconds).
    pub updated_at: u64,
}

// ============================================================================
// Team
// ============================================================================

/// A team within an organization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Team {
    /// Unique identifier.
    pub id: String,
    /// Parent organization identifier.
    pub org_id: String,
    /// Team name.
    pub name: String,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Unix timestamp of creation (seconds).
    pub created_at: u64,
}

// ============================================================================
// TeamMember
// ============================================================================

/// Membership of a user in a team.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamMember {
    /// User identifier.
    pub user_id: String,
    /// Team identifier.
    pub team_id: String,
    /// Role within the team.
    pub role: TeamRole,
    /// Unix timestamp of when the user joined (seconds).
    pub joined_at: u64,
}

// ============================================================================
// Organization
// ============================================================================

/// An organization (top-level container for teams).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Organization {
    /// Unique identifier.
    pub id: String,
    /// Organization name.
    pub name: String,
    /// Optional description.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Unix timestamp of creation (seconds).
    pub created_at: u64,
}

// ============================================================================
// OrgMember
// ============================================================================

/// Membership of a user in an organization.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrgMember {
    /// User identifier.
    pub user_id: String,
    /// Organization identifier.
    pub org_id: String,
    /// Role within the organization.
    pub role: TeamRole,
    /// Unix timestamp of when the user joined (seconds).
    pub joined_at: u64,
}

// ============================================================================
// Permission types (Phase 8)
// ============================================================================

/// Authorization decision for a tool call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthDecision {
    /// Tool is allowed without approval.
    Allow,
    /// Tool requires manual approval.
    RequireApproval,
    /// Tool is denied with a reason.
    Deny(String),
}

/// Risk level for tool classification in permission policies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PolicyRiskLevel {
    /// Zero-risk operations (read-only queries).
    Auto,
    /// Low risk (file reads, web searches).
    Low,
    /// Medium risk (file writes, network calls).
    Medium,
    /// High risk (shell execution, system modification).
    High,
}

/// Per-role permission policy.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermissionPolicy {
    /// Glob patterns for allowed tool names (e.g., "file_*", "*").
    pub allowed_tools: Vec<String>,
    /// Maximum risk level allowed without approval.
    pub max_risk_level: PolicyRiskLevel,
    /// If true, all tool calls require approval regardless of risk.
    pub require_approval: bool,
    /// Optional per-turn iteration cap for this role.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_iterations_per_turn: Option<usize>,
}

impl PermissionPolicy {
    /// A policy that allows everything without approval.
    pub fn permissive() -> Self {
        Self {
            allowed_tools: vec!["*".into()],
            max_risk_level: PolicyRiskLevel::High,
            require_approval: false,
            max_iterations_per_turn: None,
        }
    }

    /// A policy for read-only access.
    pub fn read_only() -> Self {
        Self {
            allowed_tools: vec!["file_read".into(), "web_search".into(), "webfetch".into()],
            max_risk_level: PolicyRiskLevel::Low,
            require_approval: true,
            max_iterations_per_turn: Some(10),
        }
    }
}

/// Team-level permission configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TeamPolicy {
    /// Team identifier.
    pub team_id: String,
    /// Default policy for members without a role-specific override.
    pub default_role_policy: PermissionPolicy,
    /// Per-role policy overrides.
    #[serde(default, skip_serializing_if = "std::collections::HashMap::is_empty")]
    pub role_policies: std::collections::HashMap<TeamRole, PermissionPolicy>,
}

impl TeamPolicy {
    /// Resolve the effective policy for a given role, falling back to the default.
    pub fn resolve(&self, role: &TeamRole) -> &PermissionPolicy {
        self.role_policies
            .get(role)
            .unwrap_or(&self.default_role_policy)
    }

    /// Convenience: get the permission profile for a role (used in TurnContextItem).
    pub fn profile_for(&self, role: &TeamRole) -> PermissionProfile {
        let policy = self.resolve(role);
        PermissionProfile {
            team_id: self.team_id.clone(),
            role: role.clone(),
            allowed_tools: policy.allowed_tools.clone(),
            max_risk_level: policy.max_risk_level,
            require_approval: policy.require_approval,
        }
    }
}

/// Serializable snapshot of the active permission profile for a turn.
///
/// This is the defined schema for `TurnContextItem::permission_profile`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PermissionProfile {
    /// Team identifier.
    pub team_id: String,
    /// Effective role.
    pub role: TeamRole,
    /// Allowed tool name patterns.
    pub allowed_tools: Vec<String>,
    /// Maximum risk level without approval.
    pub max_risk_level: PolicyRiskLevel,
    /// Whether all tools require approval.
    pub require_approval: bool,
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_context_default_is_all_none() {
        let ctx = IdentityContext::default();
        assert!(ctx.user_id.is_none());
        assert!(ctx.team_id.is_none());
        assert!(ctx.org_id.is_none());
    }

    #[test]
    fn identity_context_serde_roundtrip() {
        let ctx = IdentityContext {
            user_id: Some("user-1".into()),
            team_id: None,
            org_id: Some("org-1".into()),
        };
        let json = serde_json::to_string(&ctx).unwrap();
        let back: IdentityContext = serde_json::from_str(&json).unwrap();
        assert_eq!(back.user_id.as_deref(), Some("user-1"));
        assert!(back.team_id.is_none());
        assert_eq!(back.org_id.as_deref(), Some("org-1"));
    }

    #[test]
    fn identity_context_omits_none_fields() {
        let ctx = IdentityContext {
            user_id: Some("u1".into()),
            team_id: None,
            org_id: None,
        };
        let json = serde_json::to_string(&ctx).unwrap();
        // Only user_id should appear
        assert!(json.contains("user_id"));
        assert!(!json.contains("team_id"));
        assert!(!json.contains("org_id"));
    }

    #[test]
    fn user_serde_roundtrip() {
        let user = User {
            id: "u-001".into(),
            display_name: "Alice".into(),
            avatar_url: Some("https://example.com/avatar.png".into()),
            email: None,
            provider: "local".into(),
            provider_user_id: None,
            created_at: 1700000000,
            updated_at: 1700000001,
        };
        let json = serde_json::to_string(&user).unwrap();
        let back: User = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "u-001");
        assert_eq!(back.display_name, "Alice");
        assert_eq!(
            back.avatar_url.as_deref(),
            Some("https://example.com/avatar.png")
        );
        assert!(back.email.is_none());
        assert_eq!(back.provider, "local");
    }

    #[test]
    fn team_serde_roundtrip() {
        let team = Team {
            id: "t-001".into(),
            org_id: "o-001".into(),
            name: "Engineering".into(),
            description: Some("Core engineering team".into()),
            created_at: 1700000000,
        };
        let json = serde_json::to_string(&team).unwrap();
        let back: Team = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "t-001");
        assert_eq!(back.org_id, "o-001");
        assert_eq!(back.name, "Engineering");
    }

    #[test]
    fn organization_serde_roundtrip() {
        let org = Organization {
            id: "o-001".into(),
            name: "Acme Corp".into(),
            description: None,
            created_at: 1700000000,
        };
        let json = serde_json::to_string(&org).unwrap();
        let back: Organization = serde_json::from_str(&json).unwrap();
        assert_eq!(back.id, "o-001");
        assert_eq!(back.name, "Acme Corp");
        assert!(back.description.is_none());
    }

    #[test]
    fn team_role_default_is_member() {
        assert_eq!(TeamRole::default(), TeamRole::Member);
    }

    #[test]
    fn team_role_serde_snake_case() {
        let role = TeamRole::Admin;
        let json = serde_json::to_string(&role).unwrap();
        assert_eq!(json, r#""admin""#);
        let back: TeamRole = serde_json::from_str(r#""owner""#).unwrap();
        assert_eq!(back, TeamRole::Owner);
    }

    #[test]
    fn team_member_serde_roundtrip() {
        let member = TeamMember {
            user_id: "u-001".into(),
            team_id: "t-001".into(),
            role: TeamRole::Admin,
            joined_at: 1700000000,
        };
        let json = serde_json::to_string(&member).unwrap();
        let back: TeamMember = serde_json::from_str(&json).unwrap();
        assert_eq!(back.user_id, "u-001");
        assert_eq!(back.team_id, "t-001");
        assert_eq!(back.role, TeamRole::Admin);
    }

    #[test]
    fn org_member_serde_roundtrip() {
        let member = OrgMember {
            user_id: "u-001".into(),
            org_id: "o-001".into(),
            role: TeamRole::Owner,
            joined_at: 1700000000,
        };
        let json = serde_json::to_string(&member).unwrap();
        let back: OrgMember = serde_json::from_str(&json).unwrap();
        assert_eq!(back.user_id, "u-001");
        assert_eq!(back.org_id, "o-001");
        assert_eq!(back.role, TeamRole::Owner);
    }
}
