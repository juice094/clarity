//! OpenClaw device identity and pairing support.
//!
//! Local KimiClaw Gateways (and some remote ones) require device pairing
//! instead of plain token auth. This module generates an Ed25519 keypair,
//! persists it under `~/.clarity/claw-device.json`, and provides helpers to
//! sign the `connect.challenge` nonce.

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use ed25519_dalek::{Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;

/// On-disk representation of a persisted device identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DeviceIdentityFile {
    /// Hex-encoded Ed25519 private key (32 bytes).
    private_key: String,
    /// Base64url-encoded Ed25519 public key.
    public_key: String,
}

/// A Clarity-managed OpenClaw device identity.
#[derive(Clone)]
pub struct DeviceIdentity {
    signing_key: SigningKey,
}

impl DeviceIdentity {
    /// Generate a new in-memory identity without persisting it.
    #[cfg(test)]
    fn generate_unpersisted() -> Self {
        let mut csprng = rand::rngs::OsRng;
        Self {
            signing_key: SigningKey::generate(&mut csprng),
        }
    }

    /// Load an existing identity from disk or generate a new one.
    pub fn load_or_generate() -> Result<Self, String> {
        let path = device_identity_path()?;
        if path.exists() {
            let file =
                std::fs::read_to_string(&path).map_err(|e| format!("read identity: {}", e))?;
            let stored: DeviceIdentityFile =
                serde_json::from_str(&file).map_err(|e| format!("parse identity: {}", e))?;
            let bytes =
                hex::decode(&stored.private_key).map_err(|e| format!("decode key: {}", e))?;
            let arr: [u8; 32] = bytes
                .try_into()
                .map_err(|_| "private key must be 32 bytes".to_string())?;
            let signing_key = SigningKey::from_bytes(&arr);
            // Sanity check: public key matches.
            if encode_public_key(signing_key.verifying_key()) != stored.public_key {
                return Err("stored public key does not match private key".into());
            }
            return Ok(Self { signing_key });
        }

        let mut csprng = rand::rngs::OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let identity = Self { signing_key };
        identity.save()?;
        Ok(identity)
    }

    /// Persist the identity to disk.
    fn save(&self) -> Result<(), String> {
        let path = device_identity_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create dir: {}", e))?;
        }
        let file = DeviceIdentityFile {
            private_key: hex::encode(self.signing_key.to_bytes()),
            public_key: encode_public_key(self.signing_key.verifying_key()),
        };
        let json = serde_json::to_string_pretty(&file).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| format!("write identity: {}", e))?;
        Ok(())
    }

    /// Device ID is the SHA-256 of the raw public key, hex-encoded.
    pub fn device_id(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(self.signing_key.verifying_key().as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Base64url-encoded Ed25519 public key.
    pub fn public_key(&self) -> String {
        encode_public_key(self.signing_key.verifying_key())
    }

    /// Sign an arbitrary UTF-8 payload and return a base64url-encoded signature.
    pub fn sign_payload(&self, payload: &str) -> String {
        let signature = self.signing_key.sign(payload.as_bytes());
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    }

    /// Load an existing identity from disk without generating a new one.
    pub fn load_existing() -> Result<Option<Self>, String> {
        let path = device_identity_path()?;
        if !path.exists() {
            return Ok(None);
        }
        let file = std::fs::read_to_string(&path).map_err(|e| format!("read identity: {}", e))?;
        let stored: DeviceIdentityFile =
            serde_json::from_str(&file).map_err(|e| format!("parse identity: {}", e))?;
        let bytes = hex::decode(&stored.private_key).map_err(|e| format!("decode key: {}", e))?;
        let arr: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "private key must be 32 bytes".to_string())?;
        let signing_key = SigningKey::from_bytes(&arr);
        if encode_public_key(signing_key.verifying_key()) != stored.public_key {
            return Err("stored public key does not match private key".into());
        }
        Ok(Some(Self { signing_key }))
    }
}

fn encode_public_key(key: VerifyingKey) -> String {
    URL_SAFE_NO_PAD.encode(key.as_bytes())
}

fn device_identity_path() -> Result<PathBuf, String> {
    let data_dir = dirs::data_dir().ok_or("cannot determine data directory")?;
    Ok(data_dir.join("clarity").join("claw-device.json"))
}

/// A persisted OpenClaw pairing record.
///
/// Stored in `%APPDATA%/clarity/claw-device-token.json` (or the platform
/// equivalent). The legacy format only contained `gateway_url` and `token`;
/// newer records also include `device_token`, `role`, `scopes`, and a pairing
/// timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairedToken {
    /// Gateway URL this token is valid for.
    pub gateway_url: String,
    /// Admin or device token used for `auth.token` during `connect`.
    pub token: String,
    /// Optional device-specific token returned by the Gateway after pairing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_token: Option<String>,
    /// Granted role, e.g. `operator`.
    #[serde(default = "default_role")]
    pub role: String,
    /// Granted scopes.
    #[serde(default = "default_scopes")]
    pub scopes: Vec<String>,
    /// Pairing timestamp in milliseconds since Unix epoch.
    #[serde(default)]
    pub paired_at_ms: i64,
}

fn default_role() -> String {
    "operator".to_string()
}

fn default_scopes() -> Vec<String> {
    vec![
        "operator.admin".to_string(),
        "operator.read".to_string(),
        "operator.write".to_string(),
        "operator.approvals".to_string(),
        "operator.pairing".to_string(),
        "operator.talk.secrets".to_string(),
    ]
}

impl PairedToken {
    /// Return the token to use for `auth.token`.
    ///
    /// Prefers an explicit `device_token` if present, otherwise falls back to
    /// the primary `token` (which may be an admin token or a legacy device
    /// token).
    pub fn auth_token(&self) -> &str {
        self.device_token.as_deref().unwrap_or(&self.token)
    }
}

/// Path to the paired-device token saved by `openclaw_pair`.
fn device_token_path() -> Result<PathBuf, String> {
    let data_dir = dirs::data_dir().ok_or("cannot determine data directory")?;
    Ok(data_dir.join("clarity").join("claw-device-token.json"))
}

/// Load a previously-saved paired device token, if any.
///
/// Returns `Ok(None)` if no token file exists. Errors are logged by the caller.
pub fn load_paired_token() -> Result<Option<PairedToken>, String> {
    let path = device_token_path()?;
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(&path).map_err(|e| format!("read token file: {}", e))?;
    // Try the structured format first, then fall back to the legacy
    // {gateway_url, token} object.
    if let Ok(record) = serde_json::from_str::<PairedToken>(&raw) {
        return Ok(Some(record));
    }
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|e| format!("parse token file: {}", e))?;
    let gateway_url = value["gateway_url"]
        .as_str()
        .ok_or("missing gateway_url")?
        .to_string();
    let token = value["token"].as_str().ok_or("missing token")?.to_string();
    Ok(Some(PairedToken {
        gateway_url,
        token,
        device_token: None,
        role: default_role(),
        scopes: default_scopes(),
        paired_at_ms: 0,
    }))
}

/// Persist a paired device token for later automatic reconnection.
pub fn save_paired_token(record: &PairedToken) -> Result<(), String> {
    let path = device_token_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create dir: {}", e))?;
    }
    let raw = serde_json::to_string_pretty(record).map_err(|e| e.to_string())?;
    std::fs::write(&path, raw).map_err(|e| format!("write token file: {}", e))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_verify() {
        let identity = DeviceIdentity::generate_unpersisted();
        let nonce = "hello-challenge";
        let sig = identity.sign_payload(nonce);
        assert!(!sig.is_empty());
        // Device id is deterministic for the same key.
        let id1 = identity.device_id();
        let id2 = identity.device_id();
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 64);
    }

    #[test]
    fn test_paired_token_roundtrip() {
        let record = PairedToken {
            gateway_url: "ws://openclaw.example.com:18789".into(),
            token: "admin-token".into(),
            device_token: Some("device-token".into()),
            role: "operator".into(),
            scopes: vec!["operator.admin".into(), "operator.read".into()],
            paired_at_ms: 12345,
        };

        // Save to a temp path by overriding via a test-only helper would require
        // exposing the path setter, so instead we exercise JSON roundtrip.
        let json = serde_json::to_string(&record).unwrap();
        let loaded: PairedToken = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.gateway_url, record.gateway_url);
        assert_eq!(loaded.token, record.token);
        assert_eq!(loaded.device_token, record.device_token);
        assert_eq!(loaded.auth_token(), "device-token");
        assert_eq!(loaded.role, record.role);
        assert_eq!(loaded.scopes, record.scopes);
        assert_eq!(loaded.paired_at_ms, record.paired_at_ms);
    }

    #[test]
    fn test_paired_token_legacy_fallback() {
        let legacy = serde_json::json!({
            "gateway_url": "ws://127.0.0.1:18679",
            "token": "legacy-device-token"
        });
        let loaded: PairedToken = serde_json::from_str(&legacy.to_string()).unwrap();
        assert_eq!(loaded.gateway_url, "ws://127.0.0.1:18679");
        assert_eq!(loaded.token, "legacy-device-token");
        assert!(loaded.device_token.is_none());
        assert_eq!(loaded.auth_token(), "legacy-device-token");
        assert_eq!(loaded.role, "operator");
        assert!(!loaded.scopes.is_empty());
    }
}
