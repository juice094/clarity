//! EguiError — structured error types for the clarity-egui frontend.
//!
//! Replaces bare `String` errors throughout the egui crate with a typed enum,
//! enabling pattern matching in tests and uniform error display in the UI.

/// Structured errors originating from the egui frontend layer.
#[derive(Debug, Clone, PartialEq)]
#[allow(dead_code)]
pub enum EguiError {
    /// Failed to load settings from disk.
    SettingsLoad(String),
    /// Failed to persist settings to disk.
    SettingsSave(String),
    /// LLM provider initialization failed.
    LlmLoad(String),
    /// Network probe failed (used for offline detection).
    NetworkUnavailable(String),
    /// User selected an unknown/unsupported provider.
    InvalidProvider(String),
    /// User selected an unknown/unsupported model.
    InvalidModel(String),
    /// General I/O error (file system, paths, etc.).
    Io(String),
    /// Catch-all for errors that do not fit above categories.
    Other(String),
}

impl std::fmt::Display for EguiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EguiError::SettingsLoad(msg) => write!(f, "Settings load failed: {}", msg),
            EguiError::SettingsSave(msg) => write!(f, "Settings save failed: {}", msg),
            EguiError::LlmLoad(msg) => write!(f, "LLM load failed: {}", msg),
            EguiError::NetworkUnavailable(msg) => write!(f, "Network unavailable: {}", msg),
            EguiError::InvalidProvider(msg) => write!(f, "Invalid provider: {}", msg),
            EguiError::InvalidModel(msg) => write!(f, "Invalid model: {}", msg),
            EguiError::Io(msg) => write!(f, "IO error: {}", msg),
            EguiError::Other(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for EguiError {}

// ============================================================================
// Unit tests for EguiError itself
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_variants() {
        assert_eq!(
            EguiError::SettingsLoad("missing file".into()).to_string(),
            "Settings load failed: missing file"
        );
        assert_eq!(
            EguiError::LlmLoad("bad key".into()).to_string(),
            "LLM load failed: bad key"
        );
        assert_eq!(
            EguiError::NetworkUnavailable("timeout".into()).to_string(),
            "Network unavailable: timeout"
        );
    }

    #[test]
    fn test_equality() {
        let a = EguiError::InvalidProvider("foo".into());
        let b = EguiError::InvalidProvider("foo".into());
        let c = EguiError::InvalidProvider("bar".into());
        assert_eq!(a, b);
        assert_ne!(a, c);
    }
}
