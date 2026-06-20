//! C1: Provider selection policy.
//!
//! Decouples the "which provider should I use?" decision from the
//! "how do I instantiate it?" mechanics.  Keeps core logic testable
//! without async, I/O, or UI state.

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

/// Decide which provider to load.
///
/// Default policy: prefer cloud, fall back to local on network failure.
/// This function does **not** trigger network probes itself.  The caller is
/// expected to run probes independently (e.g. every 30 s) and feed the cached
/// `network_available` flag in here.  This ensures probes drive UI state only
/// and never block provider instantiation.
///
/// # Arguments
///
/// * `desired_provider` — The provider alias the user configured
///   (e.g. "openai", "deepseek", "local").
/// * `network_available` — Result of the most recent network probe.
/// * `current_provider` — The provider currently bound to the Agent,
///   if any. Used to short-circuit re-binding.
pub fn select_provider(
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

// ============================================================================
// Unit tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_already_bound() {
        let sel = select_provider("openai", true, Some("openai"));
        assert_eq!(
            sel,
            ProviderSelection::Preferred {
                provider: "openai".to_string()
            }
        );
    }

    #[test]
    fn test_binding_mismatch_still_resolves() {
        let sel = select_provider("openai", true, Some("anthropic"));
        assert_eq!(
            sel,
            ProviderSelection::Preferred {
                provider: "openai".to_string()
            }
        );
    }

    #[test]
    fn test_local_only() {
        let sel = select_provider("local", true, None);
        assert!(matches!(sel, ProviderSelection::LocalOnly { .. }));
    }

    #[test]
    fn test_network_offline_fallback() {
        let sel = select_provider("openai", false, None);
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
        let sel = select_provider("openai", true, None);
        assert_eq!(
            sel,
            ProviderSelection::Preferred {
                provider: "openai".to_string()
            }
        );
    }
}
