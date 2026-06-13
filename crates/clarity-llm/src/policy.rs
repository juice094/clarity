//! C1: Provider selection policy abstraction.
//!
//! Decouples the "which provider should I use?" decision from the
//! "how do I instantiate it?" mechanics.  Keeps core logic testable
//! without async, I/O, or UI state.
//!
//! Risk: `clarity-egui` currently has its own `ProviderSelection` enum and
//! `resolve_provider()` function (`llm_policy.rs`).  Until egui migrates to
//! this core trait, the two implementations may diverge.  The core trait is
//! designed to be a drop-in replacement — egui only needs to wrap its existing
//! logic in `impl ProviderSelectionPolicy for ...`.

/// Outcome of a provider-selection decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSelection {
    /// Use the user's preferred cloud provider.
    Preferred {
        /// Selected provider alias.
        provider: String,
    },
    /// Preferred failed; fallback to local.
    Fallback {
        /// Provider that was originally preferred.
        preferred: String,
        /// Provider to fall back to.
        fallback: String,
        /// Reason for the fallback decision.
        reason: String,
    },
    /// Use a local GGUF model.
    LocalOnly {
        /// Optional path to the local model.
        path: Option<String>,
    },
}

/// Trait for pluggable provider-selection strategies.
///
/// Implementations are pure, synchronous, and side-effect-free.
/// The caller (e.g. `clarity-egui`) is responsible for:
/// - probing network state,
/// - tracking the currently-bound provider,
/// - invoking the policy and acting on the result.
pub trait ProviderSelectionPolicy: Send + Sync {
    /// Decide which provider to load.
    ///
    /// # Arguments
    ///
    /// * `desired_provider` — The provider alias the user configured
    ///   (e.g. "openai", "deepseek", "local").
    /// * `network_available` — Result of the most recent network probe.
    /// * `current_provider` — The provider currently bound to the Agent,
    ///   if any. Policies may use this to short-circuit re-binding.
    fn select(
        &self,
        desired_provider: &str,
        network_available: bool,
        current_provider: Option<&str>,
    ) -> ProviderSelection;
}

/// Default policy: prefer cloud, fall back to local on network failure.
///
/// C2: This policy does **not** trigger network probes itself.
/// The caller is expected to run probes independently (e.g. every 30 s)
/// and feed the cached `network_available` flag into `select()`.
/// This ensures probes drive UI state only and never block provider
/// instantiation.
#[derive(Debug, Clone, Default)]
pub struct DefaultProviderSelectionPolicy;

impl ProviderSelectionPolicy for DefaultProviderSelectionPolicy {
    fn select(
        &self,
        desired_provider: &str,
        network_available: bool,
        current_provider: Option<&str>,
    ) -> ProviderSelection {
        // Short-circuit: already bound to the desired provider.
        if current_provider == Some(desired_provider) {
            return ProviderSelection::Preferred {
                provider: desired_provider.to_string(),
            };
        }

        if desired_provider == "local" {
            return ProviderSelection::LocalOnly { path: None };
        }

        if !network_available {
            return ProviderSelection::Fallback {
                preferred: desired_provider.to_string(),
                fallback: "local".to_string(),
                reason: "Network offline".to_string(),
            };
        }

        ProviderSelection::Preferred {
            provider: desired_provider.to_string(),
        }
    }
}

// ============================================================================
// Unit tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_already_bound() {
        let policy = DefaultProviderSelectionPolicy;
        let sel = policy.select("openai", true, Some("openai"));
        assert_eq!(
            sel,
            ProviderSelection::Preferred {
                provider: "openai".to_string()
            }
        );
    }

    #[test]
    fn test_binding_mismatch_still_resolves() {
        let policy = DefaultProviderSelectionPolicy;
        let sel = policy.select("openai", true, Some("anthropic"));
        assert_eq!(
            sel,
            ProviderSelection::Preferred {
                provider: "openai".to_string()
            }
        );
    }

    #[test]
    fn test_local_only() {
        let policy = DefaultProviderSelectionPolicy;
        let sel = policy.select("local", true, None);
        assert!(matches!(sel, ProviderSelection::LocalOnly { .. }));
    }

    #[test]
    fn test_network_offline_fallback() {
        let policy = DefaultProviderSelectionPolicy;
        let sel = policy.select("openai", false, None);
        assert_eq!(
            sel,
            ProviderSelection::Fallback {
                preferred: "openai".to_string(),
                fallback: "local".to_string(),
                reason: "Network offline".to_string(),
            }
        );
    }

    #[test]
    fn test_preferred_when_online() {
        let policy = DefaultProviderSelectionPolicy;
        let sel = policy.select("openai", true, None);
        assert_eq!(
            sel,
            ProviderSelection::Preferred {
                provider: "openai".to_string()
            }
        );
    }
}
