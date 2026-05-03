//! Kimi Code OAuth 2.0 Device Authorization Grant (RFC 8628).
//!
//! Mirrors the flow implemented by `kimi_cli.auth.oauth`:
//! 1. `request_device_authorization()` → user_code + verification_uri
//! 2. Open browser at verification_uri_complete
//! 3. `poll_device_token()` every `interval` seconds until success / expiry
//! 4. Store token; `refresh_token()` before expiry

use reqwest::header::HeaderMap;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::error::AgentError;

const KIMI_CODE_CLIENT_ID: &str = "17e5f671-d194-4dfb-9706-5516cb48c098";
const DEFAULT_OAUTH_HOST: &str = "https://auth.kimi.com";

/// Configuration for an OAuth 2.0 Device Authorization Grant provider.
#[derive(Debug, Clone)]
pub struct OAuthDeviceFlowConfig {
    pub client_id: String,
    pub oauth_host: String,
}

impl Default for OAuthDeviceFlowConfig {
    fn default() -> Self {
        Self {
            client_id: KIMI_CODE_CLIENT_ID.into(),
            oauth_host: DEFAULT_OAUTH_HOST.into(),
        }
    }
}

impl OAuthDeviceFlowConfig {
    /// Create with the default Kimi Code configuration.
    pub fn kimi_code() -> Self {
        Self::default()
    }
}

/// Response from the device-authorization endpoint.
#[derive(Debug, Clone)]
pub struct DeviceAuthorization {
    pub user_code: String,
    pub device_code: String,
    pub verification_uri: String,
    pub verification_uri_complete: String,
    pub expires_in: Option<u64>,
    pub interval: u64,
}

/// Errors specific to the OAuth flow.
#[derive(Debug, Clone)]
pub enum AuthError {
    Request(String),
    Unauthorized(String),
    Expired,
    Other(String),
}

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthError::Request(s) => write!(f, "OAuth request failed: {}", s),
            AuthError::Unauthorized(s) => write!(f, "OAuth unauthorized: {}", s),
            AuthError::Expired => write!(f, "Device authorization expired"),
            AuthError::Other(s) => write!(f, "OAuth error: {}", s),
        }
    }
}

impl std::error::Error for AuthError {}

fn device_model() -> String {
    #[cfg(target_os = "windows")]
    {
        let release = "11".to_string(); // best-effort; no simple API in std
        let arch = std::env::consts::ARCH;
        format!("Windows {} {}", release, arch)
    }
    #[cfg(target_os = "macos")]
    {
        format!("macOS unknown {}", std::env::consts::ARCH)
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        format!("{} {}", std::env::consts::OS, std::env::consts::ARCH)
    }
}

fn common_headers() -> HeaderMap {
    use reqwest::header::{HeaderName, HeaderValue};
    let mut headers = HeaderMap::new();
    headers.insert(
        HeaderName::from_static("x-msh-platform"),
        HeaderValue::from_static("clarity"),
    );
    headers.insert(
        HeaderName::from_static("x-msh-version"),
        HeaderValue::from_static(crate::VERSION),
    );
    let device_name = std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".to_string());
    if let Ok(v) = HeaderValue::from_str(&device_name) {
        headers.insert(HeaderName::from_static("x-msh-device-name"), v);
    }
    let model = device_model();
    if let Ok(v) = HeaderValue::from_str(&model) {
        headers.insert(HeaderName::from_static("x-msh-device-model"), v);
    }
    headers.insert(
        HeaderName::from_static("x-msh-os-version"),
        HeaderValue::from_static(std::env::consts::OS),
    );
    headers
}

/// Low-level OAuth HTTP client for Device Authorization Grant.
#[derive(Debug, Clone)]
pub struct OAuthDeviceFlowClient {
    client: reqwest::Client,
    config: OAuthDeviceFlowConfig,
}

impl Default for OAuthDeviceFlowClient {
    fn default() -> Self {
        Self::new()
    }
}

impl OAuthDeviceFlowClient {
    /// Create with the default Kimi Code configuration.
    pub fn new() -> Self {
        Self::with_config(OAuthDeviceFlowConfig::default())
    }

    /// Create with a custom OAuth configuration.
    pub fn with_config(config: OAuthDeviceFlowConfig) -> Self {
        let headers = common_headers();
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { client, config }
    }

    /// Create with the Kimi Code configuration (convenience alias).
    pub fn kimi_code() -> Self {
        Self::new()
    }


    /// Step 1: Request a device code from the authorization server.
    pub async fn request_device_authorization(&self) -> Result<DeviceAuthorization, AuthError> {
        let host = &self.config.oauth_host;
        let url = format!("{}/api/oauth/device_authorization", host.trim_end_matches('/'));
        let mut params = HashMap::new();
        params.insert("client_id", self.config.client_id.as_str());

        let response = self
            .client
            .post(&url)
            .form(&params)
            .send()
            .await
            .map_err(|e| AuthError::Request(e.to_string()))?;

        let status = response.status();
        let data: Value = response
            .json()
            .await
            .map_err(|e| AuthError::Request(format!("Failed to parse response: {}", e)))?;

        if status != 200 {
            return Err(AuthError::Request(format!(
                "Device authorization failed ({}): {}",
                status, data
            )));
        }

        Ok(DeviceAuthorization {
            user_code: data["user_code"].as_str().unwrap_or("").to_string(),
            device_code: data["device_code"].as_str().unwrap_or("").to_string(),
            verification_uri: data.get("verification_uri").and_then(|v| v.as_str()).unwrap_or("").to_string(),
            verification_uri_complete: data["verification_uri_complete"]
                .as_str()
                .unwrap_or("")
                .to_string(),
            expires_in: data.get("expires_in").and_then(|v| v.as_u64()),
            interval: data.get("interval").and_then(|v| v.as_u64()).unwrap_or(5),
        })
    }

    /// Step 3: Poll the token endpoint until the user authorizes the device.
    ///
    /// Returns `Err(AuthError::Expired)` when the device code has expired.
    pub async fn poll_device_token(
        &self,
        auth: &DeviceAuthorization,
    ) -> Result<super::OAuthToken, AuthError> {
        let host = &self.config.oauth_host;
        let url = format!("{}/api/oauth/token", host.trim_end_matches('/'));
        let mut params = HashMap::new();
        params.insert("client_id", self.config.client_id.as_str());
        params.insert("device_code", auth.device_code.as_str());
        params.insert("grant_type", "urn:ietf:params:oauth:grant-type:device_code");

        let response = self
            .client
            .post(&url)
            .form(&params)
            .send()
            .await
            .map_err(|e| AuthError::Request(e.to_string()))?;

        let status = response.status();
        let data: Value = response
            .json()
            .await
            .map_err(|e| AuthError::Request(format!("Failed to parse response: {}", e)))?;

        if status.as_u16() >= 500 {
            return Err(AuthError::Request(format!(
                "Token polling server error: {}",
                status
            )));
        }

        if status == 200 {
            return super::OAuthToken::from_response(data)
                .map_err(|e| AuthError::Other(e.to_string()));
        }

        let error = data.get("error").and_then(|v| v.as_str()).unwrap_or("unknown_error");
        if error == "expired_token" {
            return Err(AuthError::Expired);
        }
        let desc = data
            .get("error_description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        Err(AuthError::Request(format!("{}: {}", error, desc)))
    }

    /// Refresh an access token using its associated refresh token.
    pub async fn refresh_token(
        &self,
        refresh_token: &str,
    ) -> Result<super::OAuthToken, AuthError> {
        let host = &self.config.oauth_host;
        let url = format!("{}/api/oauth/token", host.trim_end_matches('/'));
        let mut params = HashMap::new();
        params.insert("client_id", self.config.client_id.as_str());
        params.insert("grant_type", "refresh_token");
        params.insert("refresh_token", refresh_token);

        let response = self
            .client
            .post(&url)
            .form(&params)
            .send()
            .await
            .map_err(|e| AuthError::Request(e.to_string()))?;

        let status = response.status();
        let data: Value = response
            .json()
            .await
            .map_err(|e| AuthError::Request(format!("Failed to parse response: {}", e)))?;

        if status == 401 || status == 403 {
            let desc = data
                .get("error_description")
                .and_then(|v| v.as_str())
                .unwrap_or("Token refresh unauthorized");
            return Err(AuthError::Unauthorized(desc.to_string()));
        }

        if status != 200 {
            let default_desc = format!("Token refresh failed (HTTP {})", status);
            let desc = data
                .get("error_description")
                .and_then(|v| v.as_str())
                .unwrap_or(&default_desc);
            return Err(AuthError::Request(desc.to_string()));
        }

        super::OAuthToken::from_response(data)
            .map_err(|e| AuthError::Other(e.to_string()))
    }
}

// ---------------------------------------------------------------------------
// TokenManager — high-level helper that couples storage + refresh logic
// ---------------------------------------------------------------------------

use super::token_store::{OAuthToken, TokenStore};

/// Manages the lifecycle of an OAuth token:
/// load from disk, refresh before expiry, persist atomically.
#[derive(Debug, Clone)]
pub struct OAuthTokenManager {
    store: TokenStore,
    client: OAuthDeviceFlowClient,
    refresh_lock: Arc<tokio::sync::Mutex<()>>,
}

impl Default for OAuthTokenManager {
    fn default() -> Self {
        Self::new()
    }
}

impl OAuthTokenManager {
    /// Create with the default Kimi Code configuration.
    pub fn new() -> Self {
        Self::with_config(OAuthDeviceFlowConfig::default(), "kimi-code")
    }

    /// Create with a custom OAuth configuration and token storage key.
    pub fn with_config(config: OAuthDeviceFlowConfig, token_key: &str) -> Self {
        Self {
            store: TokenStore::for_provider(token_key),
            client: OAuthDeviceFlowClient::with_config(config),
            refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    /// Create with the Kimi Code configuration (convenience alias).
    pub fn kimi_code() -> Self {
        Self::new()
    }


    /// Ensure a fresh access token is available.
    ///
    /// * If no token is on disk → `AgentError::Llm` telling the user to login.
    /// * If token is still valid → returns its `access_token`.
    /// * If token is within refresh window → attempts refresh, persists, returns new token.
    /// * If refresh is rejected (401/403) → deletes stored token, asks user to re-login.
    pub async fn ensure_fresh(&self) -> Result<String, AgentError> {
        match self.try_fresh().await? {
            Some(token) => Ok(token),
            None => Err(AgentError::Llm(
                "No OAuth token found. Please login via Settings → Login.".into(),
            )),
        }
    }

    /// Try to obtain a fresh access token.
    ///
    /// Returns `Ok(None)` when no token file exists (caller can fall back to a static API key).
    /// Returns `Ok(Some(token))` when a valid or refreshed token is available.
    pub async fn try_fresh(&self) -> Result<Option<String>, AgentError> {
        let _guard = self.refresh_lock.lock().await;

        // Re-read from disk — another process may have rotated the token.
        let current = self.store.load()?;

        let token = match current {
            Some(t) => t,
            None => return Ok(None),
        };

        if !token.is_expired_or_close() {
            return Ok(Some(token.access_token));
        }

        if token.refresh_token.is_empty() {
            self.store.delete()?;
            return Ok(None);
        }

        match self.client.refresh_token(&token.refresh_token).await {
            Ok(new_token) => {
                self.store.save(&new_token)?;
                tracing::info!("OAuth token refreshed successfully");
                Ok(Some(new_token.access_token))
            }
            Err(AuthError::Unauthorized(e)) => {
                tracing::warn!("OAuth refresh unauthorized: {}", e);
                self.store.delete()?;
                Ok(None)
            }
            Err(e) => {
                tracing::warn!("OAuth refresh failed: {}", e);
                // If the token is not *fully* expired yet, allow one more request
                // with the existing access token so the user isn't immediately blocked.
                if !token.is_fully_expired() {
                    tracing::info!("Returning existing token despite refresh failure (not yet fully expired)");
                    Ok(Some(token.access_token))
                } else {
                    Err(AgentError::Llm(format!("Token refresh failed: {}", e)))
                }
            }
        }
    }

    /// Save a newly obtained token (e.g. after completing device flow).
    pub fn save_token(&self, token: &OAuthToken) -> Result<(), AgentError> {
        self.store.save(token)
    }

    /// Delete stored credentials (logout).
    pub fn delete_token(&self) -> Result<(), AgentError> {
        self.store.delete()
    }

    /// Return the underlying store path (useful for `${file:...}` references).
    pub fn token_path(&self) -> &std::path::Path {
        self.store.path()
    }
}

#[cfg(test)]
impl OAuthTokenManager {
    pub fn new_with_store(store: TokenStore) -> Self {
        Self {
            store,
            client: OAuthDeviceFlowClient::new(),
            refresh_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }
}

/// Backward-compatible alias for the Kimi Code OAuth client.
pub type KimiCodeOAuthClient = OAuthDeviceFlowClient;

/// Backward-compatible alias for the Kimi Code token manager.
pub type KimiCodeTokenManager = OAuthTokenManager;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn fresh_token() -> OAuthToken {
        OAuthToken {
            access_token: "access_abc".into(),
            refresh_token: "refresh_def".into(),
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

    #[tokio::test]
    async fn test_try_fresh_no_token_returns_none() {
        let dir = TempDir::new().unwrap();
        let store = TokenStore::new(dir.path().join("token.json"));
        let manager = KimiCodeTokenManager::new_with_store(store);
        let result = manager.try_fresh().await.unwrap();
        assert_eq!(result, None);
    }

    #[tokio::test]
    async fn test_try_fresh_valid_token_returns_access_token() {
        let dir = TempDir::new().unwrap();
        let store = TokenStore::new(dir.path().join("token.json"));
        store.save(&fresh_token()).unwrap();
        let manager = KimiCodeTokenManager::new_with_store(store);
        let result = manager.try_fresh().await.unwrap();
        assert_eq!(result, Some("access_abc".to_string()));
    }

    #[tokio::test]
    async fn test_ensure_fresh_no_token_returns_error() {
        let dir = TempDir::new().unwrap();
        let store = TokenStore::new(dir.path().join("token.json"));
        let manager = KimiCodeTokenManager::new_with_store(store);
        let result = manager.ensure_fresh().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Please login"));
    }

    #[tokio::test]
    async fn test_save_and_delete_token() {
        let dir = TempDir::new().unwrap();
        let store = TokenStore::new(dir.path().join("token.json"));
        let manager = KimiCodeTokenManager::new_with_store(store);
        manager.save_token(&fresh_token()).unwrap();
        assert!(manager.token_path().exists());
        let loaded = manager.try_fresh().await.unwrap();
        assert_eq!(loaded, Some("access_abc".to_string()));
        manager.delete_token().unwrap();
        assert!(!manager.token_path().exists());
        let after = manager.try_fresh().await.unwrap();
        assert_eq!(after, None);
    }
}
