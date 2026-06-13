//! Multi-provider OAuth service.
//!
//! Wraps per-provider `OAuthTokenManager` instances and exposes a unified
//! interface for device-flow initiation, polling, and token retrieval.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use super::{
    AuthError, DeviceAuthorization, OAuthDeviceFlowClient, OAuthDeviceFlowConfig, OAuthToken,
    OAuthTokenManager,
};

/// Per-provider OAuth state used by the service.
#[derive(Clone)]
struct ProviderAuth {
    token_manager: OAuthTokenManager,
    client: OAuthDeviceFlowClient,
}

/// Unified OAuth service for multiple providers.
///
/// Each provider is identified by a unique token key (defaults to the provider
/// name). The service is cheaply cloneable because managers are atomically
/// reference-counted internally.
#[derive(Clone)]
pub struct OAuthService {
    providers: Arc<Mutex<HashMap<String, ProviderAuth>>>,
}

impl OAuthService {
    /// Create an empty service.
    pub fn new() -> Self {
        Self {
            providers: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a provider with its OAuth configuration.
    ///
    /// `token_key` is used both for the persisted token file and as the lookup
    /// key for this service. If not provided, it defaults to the provider name.
    pub fn register(
        &self,
        provider: impl Into<String>,
        config: OAuthDeviceFlowConfig,
        token_key: Option<String>,
    ) {
        let provider = provider.into();
        let token_key = token_key.unwrap_or_else(|| provider.clone());
        let manager = OAuthTokenManager::with_config(config.clone(), &token_key);
        let client = OAuthDeviceFlowClient::with_config(config);
        let auth = ProviderAuth {
            token_manager: manager,
            client,
        };
        let mut providers = self.providers.lock().unwrap_or_else(|e| e.into_inner());
        providers.insert(token_key, auth);
    }

    /// Register the default Kimi Code provider.
    pub fn register_kimi_code(&self) {
        self.register("kimi-code", OAuthDeviceFlowConfig::default(), None);
    }

    fn get_auth(&self, token_key: &str) -> Option<ProviderAuth> {
        let providers = self.providers.lock().unwrap_or_else(|e| e.into_inner());
        providers.get(token_key).cloned()
    }

    /// Start a device authorization flow for the given provider.
    pub async fn start_device_flow(
        &self,
        token_key: &str,
    ) -> Result<DeviceAuthorization, AuthError> {
        let auth = self.get_auth(token_key).ok_or_else(|| {
            AuthError::Other(format!("OAuth provider '{}' not registered", token_key))
        })?;
        auth.client.request_device_authorization().await
    }

    /// Poll the token endpoint for a provider until the user authorizes the
    /// device or the device code expires.
    pub async fn poll_device_flow(
        &self,
        token_key: &str,
        auth: &DeviceAuthorization,
    ) -> Result<OAuthToken, AuthError> {
        let provider_auth = self.get_auth(token_key).ok_or_else(|| {
            AuthError::Other(format!("OAuth provider '{}' not registered", token_key))
        })?;
        let token = provider_auth.client.poll_device_token(auth).await?;
        provider_auth
            .token_manager
            .save_token(&token)
            .map_err(|e| AuthError::Other(format!("Failed to persist token: {}", e)))?;
        Ok(token)
    }

    /// Return a valid access token if one exists or can be refreshed.
    pub async fn get_valid_token(
        &self,
        token_key: &str,
    ) -> Result<Option<String>, clarity_contract::AgentError> {
        match self.get_auth(token_key) {
            Some(auth) => auth.token_manager.try_fresh().await,
            None => Ok(None),
        }
    }
}

impl Default for OAuthService {
    fn default() -> Self {
        Self::new()
    }
}
