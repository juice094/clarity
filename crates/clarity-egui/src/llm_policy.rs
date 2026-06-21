//! Layer 1 — Policy: pure, sync provider selection.
//!
//! Decoupled from I/O, async, and Agent state. 100% unit-testable.

use crate::app_state::LlmBinding;

/// The outcome of provider selection policy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSelection {
    /// Use the user's preferred cloud provider.
    Preferred { provider: String },
    /// Preferred failed; fallback to local.
    Fallback {
        preferred: String,
        fallback: String,
        reason: String,
    },
    /// Use a local GGUF model.
    LocalOnly { path: String },
}

/// Pure function: given current state, decide which provider to load.
///
/// # Testability
/// - No async, no I/O, no mutexes.
/// - 100% branch coverage achievable with plain `#[test]`.
pub fn resolve_provider(
    desired_provider: &str,
    network_available: bool,
    current_binding: &Option<LlmBinding>,
) -> ProviderSelection {
    // Early exit: already bound to the desired provider.
    if let Some(b) = current_binding {
        if b.provider == desired_provider {
            return ProviderSelection::Preferred {
                provider: desired_provider.to_string(),
            };
        }
    }

    if desired_provider == "local" {
        return ProviderSelection::LocalOnly {
            path: String::new(), // resolved later by loader
        };
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
        let binding = Some(LlmBinding {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            local_model_path: String::new(),
        });
        let sel = resolve_provider("openai", true, &binding);
        assert_eq!(
            sel,
            ProviderSelection::Preferred {
                provider: "openai".to_string()
            }
        );
    }

    #[test]
    fn test_local_only() {
        let sel = resolve_provider("local", true, &None);
        assert!(matches!(sel, ProviderSelection::LocalOnly { .. }));
    }

    #[test]
    fn test_network_offline_fallback() {
        let sel = resolve_provider("openai", false, &None);
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
        let sel = resolve_provider("openai", true, &None);
        assert_eq!(
            sel,
            ProviderSelection::Preferred {
                provider: "openai".to_string()
            }
        );
    }

    #[test]
    fn test_binding_mismatch_still_resolves() {
        let binding = Some(LlmBinding {
            provider: "anthropic".to_string(),
            model: "claude-sonnet".to_string(),
            local_model_path: String::new(),
        });
        let sel = resolve_provider("openai", true, &binding);
        assert_eq!(
            sel,
            ProviderSelection::Preferred {
                provider: "openai".to_string()
            }
        );
    }
}
