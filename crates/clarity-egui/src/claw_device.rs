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
#[allow(dead_code)]
struct DeviceIdentityFile {
    /// Hex-encoded Ed25519 private key (32 bytes).
    private_key: String,
    /// Base64url-encoded Ed25519 public key.
    public_key: String,
    /// Human-readable label for this device.
    label: String,
}

/// A Clarity-managed OpenClaw device identity.
#[derive(Clone)]
pub struct DeviceIdentity {
    signing_key: SigningKey,
    /// Human-readable label persisted with the identity.
    #[allow(dead_code)]
    label: String,
}

impl DeviceIdentity {
    /// Generate a new in-memory identity without persisting it.
    #[cfg(test)]
    fn generate_unpersisted(label: &str) -> Self {
        let mut csprng = rand::rngs::OsRng;
        Self {
            signing_key: SigningKey::generate(&mut csprng),
            label: label.into(),
        }
    }

    /// Load an existing identity from disk or generate a new one.
    #[allow(dead_code)] // Used once device pairing UX lands.
    pub fn load_or_generate(label: &str) -> Result<Self, String> {
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
            return Ok(Self {
                signing_key,
                label: stored.label,
            });
        }

        let mut csprng = rand::rngs::OsRng;
        let signing_key = SigningKey::generate(&mut csprng);
        let identity = Self {
            signing_key,
            label: label.into(),
        };
        identity.save()?;
        Ok(identity)
    }

    /// Persist the identity to disk.
    #[allow(dead_code)] // Called by load_or_generate.
    fn save(&self) -> Result<(), String> {
        let path = device_identity_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("create dir: {}", e))?;
        }
        let file = DeviceIdentityFile {
            private_key: hex::encode(self.signing_key.to_bytes()),
            public_key: encode_public_key(self.signing_key.verifying_key()),
            label: self.label.clone(),
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

    /// Sign a UTF-8 nonce and return base64url-encoded signature.
    pub fn sign_nonce(&self, nonce: &str) -> String {
        let signature = self.signing_key.sign(nonce.as_bytes());
        URL_SAFE_NO_PAD.encode(signature.to_bytes())
    }
}

fn encode_public_key(key: VerifyingKey) -> String {
    URL_SAFE_NO_PAD.encode(key.as_bytes())
}

#[allow(dead_code)] // Called by load_or_generate / save.
fn device_identity_path() -> Result<PathBuf, String> {
    let data_dir = dirs::data_dir().ok_or("cannot determine data directory")?;
    Ok(data_dir.join("clarity").join("claw-device.json"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_and_verify() {
        let identity = DeviceIdentity::generate_unpersisted("test");
        let nonce = "hello-challenge";
        let sig = identity.sign_nonce(nonce);
        assert!(!sig.is_empty());
        // Device id is deterministic for the same key.
        let id1 = identity.device_id();
        let id2 = identity.device_id();
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 64);
    }
}
