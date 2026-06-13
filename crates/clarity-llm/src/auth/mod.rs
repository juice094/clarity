//! Authentication & credential management for Clarity.
//!
//! Currently implements Kimi Code OAuth 2.0 Device Authorization Grant
//! (RFC 8628) with automatic token refresh and secure file storage.

pub mod kimi_code;
pub mod service;
pub mod token_store;

pub use kimi_code::{
    AuthError, DeviceAuthorization, KimiCodeOAuthClient, KimiCodeTokenManager,
    OAuthDeviceFlowClient, OAuthDeviceFlowConfig, OAuthTokenManager,
};
pub use service::OAuthService;
pub use token_store::{OAuthToken, TokenStore};
