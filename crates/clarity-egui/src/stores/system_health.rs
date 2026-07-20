//! Global system health state.
//!
//! Aggregates network, provider, memory, gateway and MCP health so that the UI
//! can surface a single, consistent status to the user. Background tasks write
//! to the store; UI panels read from it every frame.

use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Discrete health level for a subsystem.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum HealthState {
    /// The subsystem has not been checked yet.
    #[default]
    Unknown,
    /// The subsystem is operating normally.
    Healthy,
    /// The subsystem is reachable but showing problems (e.g. slow, rate limited).
    /// Currently unused, but reserved for future HTTP-level health probes.
    #[allow(dead_code)]
    Degraded {
        /// Human-readable detail shown in the status banner.
        message: String,
    },
    /// The subsystem is unavailable or misconfigured.
    Unhealthy {
        /// Human-readable detail shown in the status banner.
        message: String,
    },
}

/// Per-provider health snapshot.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ProviderHealth {
    /// Provider alias, e.g. "kimi" or "openai".
    pub name: String,
    /// Current health level.
    pub state: HealthState,
}

/// A user-visible error that should be surfaced until explicitly dismissed.
#[derive(Clone, Debug, PartialEq)]
pub struct UserVisibleError {
    /// Stable id used for egui state.
    pub id: egui::Id,
    /// Short title shown in the status banner.
    pub title: String,
    /// Longer explanation.
    pub message: String,
}

/// Aggregated system health consumed by the UI.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SystemHealth {
    /// General internet reachability.
    pub network: HealthState,
    /// Configured LLM providers.
    pub providers: HashMap<String, ProviderHealth>,
    /// Long-term memory store.
    pub memory: HealthState,
    /// Local Gateway server.
    pub gateway: HealthState,
    /// MCP tool servers.
    pub mcp: HealthState,
    /// Last global error that should be shown to the user.
    pub last_error: Option<UserVisibleError>,
}

/// Thread-safe store for [`SystemHealth`].
#[derive(Clone, Debug, Default)]
pub struct SystemHealthStore {
    inner: Arc<Mutex<SystemHealth>>,
}

impl SystemHealthStore {
    /// Create an empty health store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return a snapshot of the current health state.
    pub fn get(&self) -> SystemHealth {
        self.inner.lock().clone()
    }

    /// Set overall network health.
    pub fn set_network(&self, state: HealthState) {
        self.inner.lock().network = state;
    }

    /// Set health for a specific provider alias.
    pub fn set_provider(&self, name: &str, state: HealthState) {
        self.inner.lock().providers.insert(
            name.to_string(),
            ProviderHealth {
                name: name.to_string(),
                state,
            },
        );
    }

    /// Set long-term memory store health.
    pub fn set_memory(&self, state: HealthState) {
        self.inner.lock().memory = state;
    }

    /// Set local Gateway health.
    pub fn set_gateway(&self, state: HealthState) {
        self.inner.lock().gateway = state;
    }

    /// Set MCP health.
    pub fn set_mcp(&self, state: HealthState) {
        self.inner.lock().mcp = state;
    }

    /// Push a user-visible error, replacing any previous one.
    pub fn push_error(&self, title: impl Into<String>, message: impl Into<String>) {
        let title = title.into();
        let message = message.into();
        self.inner.lock().last_error = Some(UserVisibleError {
            id: egui::Id::new(format!("{}:{}", title, message)),
            title,
            message,
        });
    }

    /// Clear the currently displayed user-visible error.
    pub fn clear_error(&self) {
        self.inner.lock().last_error = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn network_state_roundtrip() {
        let store = SystemHealthStore::new();
        store.set_network(HealthState::Healthy);
        assert_eq!(store.get().network, HealthState::Healthy);
    }

    #[test]
    fn provider_state_roundtrip() {
        let store = SystemHealthStore::new();
        store.set_provider(
            "kimi",
            HealthState::Unhealthy {
                message: "timeout".into(),
            },
        );
        let health = store.get();
        assert!(matches!(
            health.providers.get("kimi"),
            Some(ProviderHealth {
                state: HealthState::Unhealthy { .. },
                ..
            })
        ));
    }

    #[test]
    fn error_push_and_clear() {
        let store = SystemHealthStore::new();
        store.push_error("Save failed", "disk full");
        assert_eq!(
            store.get().last_error.as_ref().unwrap().title,
            "Save failed"
        );
        store.clear_error();
        assert!(store.get().last_error.is_none());
    }
}
