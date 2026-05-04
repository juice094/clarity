//! Secure token storage for OAuth credentials.
//!
//! Tokens are stored as JSON in the platform data directory:
//! - Windows: `%APPDATA%/clarity/credentials/kimi-code.json`
//! - Unix: `~/.local/share/clarity/credentials/kimi-code.json`
//!
//! Writes are atomic (temp file + rename) to prevent corruption during
//! concurrent access.  Process-level locking is handled by the caller
//! (`KimiCodeTokenManager`); cross-process coordination relies on the
//! atomic-rename semantics.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::error::AgentError;

/// An OAuth 2.0 token set returned by the Kimi Code authorization server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub refresh_token: String,
    /// Unix timestamp when the token expires.
    #[serde(default)]
    pub expires_at: f64,
    #[serde(default)]
    pub scope: String,
    #[serde(default)]
    pub token_type: String,
    /// Original expiry duration in seconds (used for threshold calculation).
    #[serde(default)]
    pub expires_in: f64,
}

impl OAuthToken {
    /// Build from the JSON response returned by `/api/oauth/token`.
    pub fn from_response(payload: serde_json::Value) -> Result<Self, AgentError> {
        let expires_in = payload
            .get("expires_in")
            .and_then(|v| v.as_f64())
            .or_else(|| {
                payload
                    .get("expires_in")
                    .and_then(|v| v.as_i64())
                    .map(|v| v as f64)
            })
            .unwrap_or(0.0);
        Ok(Self {
            access_token: payload
                .get("access_token")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            refresh_token: payload
                .get("refresh_token")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            expires_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs_f64()
                + expires_in,
            scope: payload
                .get("scope")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            token_type: payload
                .get("token_type")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string(),
            expires_in,
        })
    }

    /// Return true when the token is within the refresh window.
    ///
    /// Threshold = max(300 s, expires_in * 0.5).
    pub fn is_expired_or_close(&self) -> bool {
        if self.expires_at <= 0.0 {
            return false; // no expiry info → assume valid
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        let threshold = if self.expires_in > 0.0 {
            (300.0_f64).max(self.expires_in * 0.5)
        } else {
            300.0
        };
        now >= self.expires_at - threshold
    }

    /// Return true when the token has fully expired (no grace period).
    pub fn is_fully_expired(&self) -> bool {
        if self.expires_at <= 0.0 {
            return false;
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs_f64();
        now >= self.expires_at
    }
}

/// File-backed token store with atomic writes.
#[derive(Debug, Clone)]
pub struct TokenStore {
    path: PathBuf,
}

impl TokenStore {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Default path for Kimi Code credentials.
    pub fn default_kimi_code() -> Self {
        Self::for_provider("kimi-code")
    }

    /// Return a TokenStore for an arbitrary OAuth provider key.
    /// File is stored at `<data_dir>/clarity/credentials/<key>.json`.
    pub fn for_provider(key: &str) -> Self {
        let path = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clarity")
            .join("credentials")
            .join(format!("{}.json", key));
        Self::new(path)
    }

    /// Return the default credentials directory path.
    pub fn default_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clarity")
            .join("credentials")
    }

    pub fn load(&self) -> Result<Option<OAuthToken>, AgentError> {
        if !self.path.exists() {
            return Ok(None);
        }
        let content = std::fs::read_to_string(&self.path)
            .map_err(|e| AgentError::Llm(format!("Failed to read token file: {}", e)))?;
        let token: OAuthToken = serde_json::from_str(&content)
            .map_err(|e| AgentError::Llm(format!("Failed to parse token: {}", e)))?;
        Ok(Some(token))
    }

    /// Atomic write: temp file → rename.
    pub fn save(&self, token: &OAuthToken) -> Result<(), AgentError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AgentError::Llm(format!("Failed to create credentials dir: {}", e)))?;
        }
        let tmp = self.path.with_extension("tmp");
        let data = serde_json::to_vec_pretty(token)
            .map_err(|e| AgentError::Llm(format!("Failed to serialize token: {}", e)))?;
        std::fs::write(&tmp, &data)
            .map_err(|e| AgentError::Llm(format!("Failed to write temp token file: {}", e)))?;
        std::fs::rename(&tmp, &self.path)
            .map_err(|e| AgentError::Llm(format!("Failed to rename token file: {}", e)))?;
        Ok(())
    }

    pub fn delete(&self) -> Result<(), AgentError> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)
                .map_err(|e| AgentError::Llm(format!("Failed to delete token file: {}", e)))?;
        }
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_token() -> OAuthToken {
        OAuthToken {
            access_token: "access_123".into(),
            refresh_token: "refresh_456".into(),
            expires_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs_f64()
                + 3600.0,
            scope: "all".into(),
            token_type: "Bearer".into(),
            expires_in: 3600.0,
        }
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let store = TokenStore::new(dir.path().join("token.json"));
        let token = sample_token();
        store.save(&token).unwrap();
        let loaded = store.load().unwrap().unwrap();
        assert_eq!(loaded.access_token, "access_123");
        assert_eq!(loaded.refresh_token, "refresh_456");
        assert_eq!(loaded.expires_in, 3600.0);
    }

    #[test]
    fn test_delete_clears_token() {
        let dir = TempDir::new().unwrap();
        let store = TokenStore::new(dir.path().join("token.json"));
        let token = sample_token();
        store.save(&token).unwrap();
        assert!(store.load().unwrap().is_some());
        store.delete().unwrap();
        assert!(store.load().unwrap().is_none());
    }

    #[test]
    fn test_atomic_write_no_tmp_residue() {
        let dir = TempDir::new().unwrap();
        let store = TokenStore::new(dir.path().join("token.json"));
        let token = sample_token();
        store.save(&token).unwrap();
        let entries: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].as_ref().unwrap().path().ends_with("token.json"));
    }

    #[test]
    fn test_token_not_expired_when_fresh() {
        let token = sample_token();
        assert!(!token.is_expired_or_close());
        assert!(!token.is_fully_expired());
    }

    #[test]
    fn test_token_expired_or_close_within_threshold() {
        let mut token = sample_token();
        // expires_in=3600 → threshold = max(300, 1800) = 1800
        // Set expires_at to now + 1000 (within threshold)
        token.expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            + 1000.0;
        assert!(token.is_expired_or_close());
        assert!(!token.is_fully_expired());
    }

    #[test]
    fn test_token_fully_expired() {
        let mut token = sample_token();
        token.expires_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs_f64()
            - 10.0;
        assert!(token.is_expired_or_close());
        assert!(token.is_fully_expired());
    }

    #[test]
    fn test_token_no_expiry_never_expired() {
        let token = OAuthToken {
            access_token: "x".into(),
            refresh_token: "y".into(),
            expires_at: 0.0,
            scope: "".into(),
            token_type: "".into(),
            expires_in: 0.0,
        };
        assert!(!token.is_expired_or_close());
        assert!(!token.is_fully_expired());
    }
}
