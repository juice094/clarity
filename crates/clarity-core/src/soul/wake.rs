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

use crate::adaptive::AgentGrowthProfile;
use crate::registry::ToolRegistry;
use crate::skills::SkillRegistry;
use crate::soul::Soul;
use crate::Agent;

// ============================================================================
// Error types
// ============================================================================

/// Errors during wake or suspend.
#[derive(Debug, Error, Clone, PartialEq)]
pub enum WakeError {
    #[error("soul not found: {0}")]
    SoulNotFound(String),

    #[error("session store read failed: {0}")]
    SessionStore(String),

    #[error("missing dependency: {0}")]
    MissingDependency(String),

    #[error("state deserialization failed: {0}")]
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
// DefaultWakeAdapter
// ============================================================================

/// Default wake implementation that reads from the standard soul directory.
pub struct DefaultWakeAdapter;

impl Wakeable for DefaultWakeAdapter {
    async fn wake(&self, soul_id: &str, deps: &AgentDeps) -> Result<Agent, WakeError> {
        // Load soul metadata.
        let soul = Soul::load_or_create(soul_id);

        // Verify soul exists on disk (load_or_create creates a default if missing,
        // but wake should only succeed for previously persisted souls).
        let soul_path = soul.soul_json_path();
        if !soul_path.exists() {
            return Err(WakeError::SoulNotFound(soul_id.to_string()));
        }

        // Load profile (use provided or disk).
        let _profile = deps
            .profile
            .clone()
            .unwrap_or_else(|| AgentGrowthProfile::load_or_create(soul_id));

        // NOTE: Full Agent reconstruction requires linking to the existing
        // Agent::new path, which is complex due to the many optional fields.
        // For now, we return a placeholder error indicating the integration
        // point. A production implementation would:
        // 1. Read state.json for last_turn_id / last_event_id
        // 2. Reconstruct conversation context from SessionStoreV2
        // 3. Build Agent with soul's persona, capabilities, and profile
        // 4. Inject growth-profile-derived model preferences

        Err(WakeError::MissingDependency(
            "Agent::wake full integration pending ADR-008 M1 acceptance".to_string(),
        ))
    }

    async fn suspend(&self, _agent: &Agent) -> Result<SuspendedState, WakeError> {
        // NOTE: Full suspension requires extracting Agent inner state,
        // which is private. This is the integration boundary.
        Err(WakeError::MissingDependency(
            "Agent::suspend full integration pending ADR-008 M1 acceptance".to_string(),
        ))
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
}
