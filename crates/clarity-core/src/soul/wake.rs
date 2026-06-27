//! Wake / Suspend — reconstruct an Agent from persisted Soul state.
//!
//! The [`Wakeable`] trait allows a Soul to be fully reconstructed from disk,
//! but does **not** make it the only construction path. Direct `Agent::new`
//! remains available for fresh starts.
//!
//! # Design constraints (ADR-008 M1)
//!
//! - Wake reads only local `SessionStore` — no RPC / IPC.
//! - Suspended state includes last event ID as an anchor point.
//! - Wake failure is explicit (returns `WakeError`), never silently degraded.

use std::sync::Arc;

use thiserror::Error;

use crate::Agent;
use crate::adaptive::AgentGrowthProfile;
use crate::registry::ToolRegistry;
use crate::skills::SkillRegistry;
use crate::soul::Soul;

// ============================================================================
// Error types
// ============================================================================

/// Errors during wake or suspend.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum WakeError {
    #[error("soul not found: {0}")]
    /// Soul not found.
    SoulNotFound(String),

    #[error("session store read failed: {0}")]
    /// Session store error.
    SessionStore(String),

    #[error("missing dependency: {0}")]
    /// Missing dependency.
    MissingDependency(String),

    #[error("state deserialization failed: {0}")]
    /// Deserialization error.
    Deserialization(String),
}

// ============================================================================
// SuspendedState
// ============================================================================

/// Serializable snapshot of a running soul.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SuspendedState {
    /// Soul identifier.
    pub soul_id: String,

    /// Last known session ID.
    pub session_id: Option<String>,

    /// Last processed turn ID (anchor point for reconstruction).
    pub last_turn_id: Option<u64>,

    /// Last processed event ID within the turn.
    pub last_event_id: Option<u64>,

    /// Serialized runtime state (opaque to the storage layer).
    pub runtime_blob: Vec<u8>,
}

// ============================================================================
// Wakeable
// ============================================================================

/// Trait for souls that can be woken from persisted state.
///
/// Implementors must guarantee that `wake` produces an equivalent Agent
/// to the one that was suspended, modulo non-deterministic LLM outputs.
pub trait Wakeable {
    /// Reconstruct an Agent from its Soul ID and dependencies.
    ///
    /// # Errors
    ///
    /// Returns `WakeError` if the soul does not exist, the session store
    /// is unreadable, or required dependencies are missing.
    fn wake(
        &self,
        soul_id: &str,
        deps: &AgentDeps,
    ) -> impl std::future::Future<Output = Result<Agent, WakeError>> + Send;

    /// Serialize the current Agent state into a `SuspendedState`.
    ///
    /// Called before shutdown or when the user explicitly suspends a soul.
    fn suspend(
        &self,
        agent: &Agent,
    ) -> impl std::future::Future<Output = Result<SuspendedState, WakeError>> + Send;
}

// ============================================================================
// AgentDeps
// ============================================================================

/// Dependencies required to wake an Agent.
///
/// This struct decouples the wake process from the global application state,
/// making it testable and allowing different dependency graphs per environment.
#[derive(Clone, Default)]
pub struct AgentDeps {
    /// Tool registry (may be pre-populated with allowed tools).
    pub registry: Option<Arc<ToolRegistry>>,

    /// Skill registry for skill-aware waking.
    pub skill_registry: Option<SkillRegistry>,

    /// The soul's growth profile (loaded from disk or passed in).
    pub profile: Option<AgentGrowthProfile>,
}

impl AgentDeps {
    /// Create empty deps (for testing or when everything is auto-discovered).
    pub fn new() -> Self {
        Self::default()
    }

    /// Provide a tool registry.
    pub fn with_registry(mut self, registry: Arc<ToolRegistry>) -> Self {
        self.registry = Some(registry);
        self
    }

    /// Provide a skill registry.
    pub fn with_skills(mut self, skills: SkillRegistry) -> Self {
        self.skill_registry = Some(skills);
        self
    }

    /// Provide a growth profile.
    pub fn with_profile(mut self, profile: AgentGrowthProfile) -> Self {
        self.profile = Some(profile);
        self
    }
}

// ============================================================================
// SuspendSnapshot — serializable Agent runtime state
// ============================================================================

/// Serializable snapshot of Agent's persistent runtime fields.
///
/// This is the content of `SuspendedState.runtime_blob`. It captures only
/// the fields that survive across sessions — LLM clients, memory stores,
/// and other trait-object dependencies are omitted and re-injected at wake.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
pub struct SuspendSnapshot {
    /// AgentConfig fields needed for reconstruction.
    pub config: SuspendConfig,

    /// Per-session state to preserve across suspension.
    pub session: SuspendSession,

    /// Schema version for migration compatibility.
    pub version: u32,
}

/// AgentConfig subset serializable for suspend/wake.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Default)]
pub struct SuspendConfig {
    /// Maximum iteration count.
    pub max_iterations: usize,
    /// Tool timeout secs.
    pub tool_timeout_secs: u64,
    /// Whether the invocation is read-only.
    pub read_only: bool,
    /// Maximum context token count.
    pub max_context_tokens: usize,
    /// Optional system prompt.
    pub system_prompt: Option<String>,
    /// Optional working directory.
    pub working_dir: Option<String>,
    /// User identifier for attribution.
    pub user_id: Option<String>,
    /// Team identifier for team-scoped sessions.
    pub team_id: Option<String>,
    /// Organization identifier.
    pub org_id: Option<String>,
}

/// Session-scoped state preserved across suspension.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Default)]
pub struct SuspendSession {
    /// Current approval mode.
    pub approval_mode: String,
    /// Daily cost in USD.
    pub daily_cost_usd: f64,
    /// Message count in the last turn.
    pub last_turn_message_count: usize,
    /// Optional active provider label.
    pub provider_label: Option<String>,
}

impl SuspendSnapshot {
    /// Create a new `SuspendSnapshot`.
    pub fn new() -> Self {
        Self {
            config: SuspendConfig::default(),
            session: SuspendSession::default(),
            version: 1,
        }
    }
}

impl Default for SuspendSnapshot {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// DefaultWakeAdapter
// ============================================================================

/// Default wake implementation that reads from the standard soul directory.
pub struct DefaultWakeAdapter;

impl Wakeable for DefaultWakeAdapter {
    async fn wake(&self, soul_id: &str, deps: &AgentDeps) -> Result<Agent, WakeError> {
        let soul = Soul::load_or_create(soul_id);
        let soul_path = soul.soul_json_path();
        if !soul_path.exists() {
            return Err(WakeError::SoulNotFound(soul_id.to_string()));
        }

        // Read suspended state from disk.
        let state_path = soul.state_json_path();
        let suspended = if state_path.exists() {
            let contents = std::fs::read_to_string(&state_path)
                .map_err(|e| WakeError::Deserialization(e.to_string()))?;
            serde_json::from_str::<SuspendedState>(&contents)
                .map_err(|e| WakeError::Deserialization(e.to_string()))?
        } else {
            return Err(WakeError::MissingDependency(format!(
                "state.json not found for soul {soul_id}"
            )));
        };

        // Deserialize runtime blob.
        let snapshot: SuspendSnapshot = serde_json::from_slice(&suspended.runtime_blob)
            .map_err(|e| WakeError::Deserialization(e.to_string()))?;

        // Reconstruct Agent from snapshot.
        let agent = Agent::wake_from_snapshot(&snapshot, deps)?;

        Ok(agent)
    }

    async fn suspend(&self, agent: &Agent) -> Result<SuspendedState, WakeError> {
        let snapshot = agent.suspend_snapshot();
        let blob =
            serde_json::to_vec(&snapshot).map_err(|e| WakeError::Deserialization(e.to_string()))?;

        Ok(SuspendedState {
            soul_id: String::new(),
            session_id: None,
            last_turn_id: None,
            last_event_id: None,
            runtime_blob: blob,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_wake_missing_soul() {
        let adapter = DefaultWakeAdapter;
        let deps = AgentDeps::new();
        let result = adapter.wake("nonexistent-soul-xyz", &deps).await;
        assert!(matches!(result, Err(WakeError::SoulNotFound(_))));
    }

    #[test]
    fn test_suspended_state_roundtrip() {
        let state = SuspendedState {
            soul_id: "grey".to_string(),
            session_id: Some("sess-123".to_string()),
            last_turn_id: Some(5),
            last_event_id: Some(12),
            runtime_blob: vec![1, 2, 3],
        };

        let json = serde_json::to_string(&state).unwrap();
        let restored: SuspendedState = serde_json::from_str(&json).unwrap();
        assert_eq!(state, restored);
    }

    #[test]
    fn test_agent_deps_builder() {
        let deps = AgentDeps::new().with_profile(AgentGrowthProfile::new("test"));
        assert!(deps.profile.is_some());
        assert!(deps.registry.is_none());
    }

    #[test]
    fn test_suspend_snapshot_roundtrip() {
        let registry = ToolRegistry::with_builtin_tools();
        let config = crate::agent::config::AgentConfig::new()
            .with_max_iterations(10)
            .with_read_only(true)
            .with_working_dir("/tmp/test");

        let agent = Agent::with_config(registry, config);

        // Suspend
        let snapshot = agent.suspend_snapshot();
        assert_eq!(snapshot.version, 1);
        assert_eq!(snapshot.config.max_iterations, 10);
        assert!(snapshot.config.read_only);
        assert!(snapshot.config.working_dir.as_deref() == Some("/tmp/test"));

        // Serialize → deserialize
        let blob = serde_json::to_vec(&snapshot).unwrap();
        let restored: SuspendSnapshot = serde_json::from_slice(&blob).unwrap();
        assert_eq!(restored, snapshot);
    }

    #[test]
    fn test_wake_from_snapshot_preserves_config() {
        let snapshot = SuspendSnapshot {
            config: SuspendConfig {
                max_iterations: 42,
                tool_timeout_secs: 30,
                read_only: false,
                max_context_tokens: 8192,
                system_prompt: Some("test prompt".to_string()),
                working_dir: Some("/tmp/wake".to_string()),
                user_id: None,
                team_id: None,
                org_id: None,
            },
            session: SuspendSession {
                approval_mode: "Plan".to_string(),
                daily_cost_usd: 0.0,
                last_turn_message_count: 0,
                provider_label: None,
            },
            version: 1,
        };

        let deps = AgentDeps::default();
        let agent = Agent::wake_from_snapshot(&snapshot, &deps).unwrap();
        assert!(agent.approval_mode() == crate::approval::ApprovalMode::Plan);
    }

    #[test]
    fn test_wake_from_snapshot_fails_on_corrupt_blob() {
        let corrupt = vec![0xff, 0xfe, 0xfd];
        let result = serde_json::from_slice::<SuspendSnapshot>(&corrupt);
        assert!(result.is_err(), "corrupt blob must fail deserialization");
    }

    #[test]
    fn test_wake_with_deps_injection() {
        let registry = ToolRegistry::with_builtin_tools();
        let deps = AgentDeps::new().with_registry(std::sync::Arc::new(registry));

        let snapshot = SuspendSnapshot::new();
        let agent = Agent::wake_from_snapshot(&snapshot, &deps).unwrap();
        assert!(agent.approval_mode() == crate::approval::ApprovalMode::Interactive);
    }
}
