//! Authentication and device-pairing verification for the OpenClaw server.

use clarity_contract::openclaw_protocol::{
    ConnectParams, OpenClawDeviceProof, OpenClawErrorShape, build_device_auth_payload,
};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use std::collections::HashSet;

/// Result of authenticating a `connect` request.
#[derive(Clone, Debug)]
pub enum AuthResult {
    /// Authenticated as an admin-capable connection.
    Admin {
        /// Granted scopes.
        scopes: Vec<String>,
    },
    /// Authenticated as a paired device.
    Device {
        /// Device id.
        device_id: String,
        /// Granted scopes.
        scopes: Vec<String>,
    },
    /// Authentication failed.
    Denied(OpenClawErrorShape),
}

/// Verify a `connect` request against the configured admin token and approved
/// device list.
///
/// `admin_token` is the token required for CLI/admin connections.
/// `approved_devices` maps device id → (public_key, scopes).
pub fn authenticate_connect(
    params: &ConnectParams,
    admin_token: &str,
    approved_devices: &std::collections::HashMap<String, (String, Vec<String>)>,
) -> AuthResult {
    // 1. If a device proof is present, verify the signature and look up the
    //    device in the approved list.
    if let Some(ref proof) = params.device {
        return authenticate_device(proof, params, approved_devices);
    }

    // 2. Otherwise, check the auth token against the admin token.
    let supplied = params
        .auth
        .as_ref()
        .and_then(|a| a.token.as_deref())
        .unwrap_or("");

    if supplied == admin_token && !admin_token.is_empty() {
        return AuthResult::Admin {
            scopes: full_scopes(),
        };
    }

    AuthResult::Denied(OpenClawErrorShape {
        code: "UNAUTHORIZED".to_string(),
        message: "Invalid admin token or unpaired device".to_string(),
        details: None,
        retryable: Some(false),
        retry_after_ms: None,
    })
}

fn authenticate_device(
    proof: &OpenClawDeviceProof,
    params: &ConnectParams,
    approved_devices: &std::collections::HashMap<String, (String, Vec<String>)>,
) -> AuthResult {
    let Some((stored_public_key, scopes)) = approved_devices.get(&proof.id) else {
        return denied("Device not paired");
    };

    if stored_public_key != &proof.public_key {
        return denied("Public key mismatch");
    }

    let Ok(public_key_bytes) = base64_decode_url_safe(&proof.public_key) else {
        return denied("Invalid public key encoding");
    };

    let Ok(public_key_array) = public_key_bytes.try_into() else {
        return denied("Invalid Ed25519 public key length");
    };
    let Ok(verifying_key) = VerifyingKey::from_bytes(&public_key_array) else {
        return denied("Invalid Ed25519 public key");
    };

    let payload = build_device_auth_payload(
        &proof.id,
        &params.client.id,
        &params.client.mode,
        params.role.as_deref().unwrap_or("operator"),
        &params.scopes,
        proof.signed_at,
        params
            .auth
            .as_ref()
            .and_then(|a| a.token.as_deref())
            .unwrap_or(""),
        &proof.nonce,
        &params.client.platform,
        params.client.device_family.as_deref(),
    );

    let Ok(signature_bytes) = base64_decode_url_safe(&proof.signature) else {
        return denied("Invalid signature encoding");
    };

    let signature_arr: [u8; 64] = match signature_bytes.try_into() {
        Ok(arr) => arr,
        Err(_) => return denied("Invalid Ed25519 signature length"),
    };
    let signature = Signature::from_bytes(&signature_arr);

    if verifying_key
        .verify(payload.as_bytes(), &signature)
        .is_err()
    {
        return denied("Device proof signature verification failed");
    }

    AuthResult::Device {
        device_id: proof.id.clone(),
        scopes: scopes.clone(),
    }
}

fn denied(message: &str) -> AuthResult {
    AuthResult::Denied(OpenClawErrorShape {
        code: "UNAUTHORIZED".to_string(),
        message: message.to_string(),
        details: None,
        retryable: Some(false),
        retry_after_ms: None,
    })
}

fn base64_decode_url_safe(s: &str) -> Result<Vec<u8>, base64::DecodeError> {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
    URL_SAFE_NO_PAD.decode(s)
}

/// Full operator scopes granted to admin connections and approved devices.
pub fn full_scopes() -> Vec<String> {
    [
        "operator.admin",
        "operator.read",
        "operator.write",
        "operator.approvals",
        "operator.pairing",
        "operator.talk.secrets",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Generate a fresh admin token.
pub fn generate_admin_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}

/// Validate a `device.pair.request` payload and return the device public key
/// when valid.
///
/// The payload shape mirrors `clarity-claw::openclaw_gateway::device::PairRequestResult`.
pub fn validate_pair_request(
    value: &serde_json::Value,
) -> Result<(String, String), OpenClawErrorShape> {
    let device_id = value
        .get("deviceId")
        .or_else(|| value.get("device_id"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| error_shape("INVALID_PARAMS", "missing deviceId"))?;

    let public_key = value
        .get("publicKey")
        .and_then(|v| v.as_str())
        .ok_or_else(|| error_shape("INVALID_PARAMS", "missing publicKey"))?;

    // Basic sanity check: public key must be base64url-decodable Ed25519 key.
    let bytes = base64_decode_url_safe(public_key)
        .map_err(|_| error_shape("INVALID_PARAMS", "publicKey is not valid base64url"))?;

    if bytes.len() != 32 {
        return Err(error_shape(
            "INVALID_PARAMS",
            "publicKey must be a 32-byte Ed25519 key",
        ));
    }

    Ok((device_id.to_string(), public_key.to_string()))
}

fn error_shape(code: &str, message: &str) -> OpenClawErrorShape {
    OpenClawErrorShape {
        code: code.to_string(),
        message: message.to_string(),
        details: None,
        retryable: Some(false),
        retry_after_ms: None,
    }
}

/// Check whether `required` scopes are a subset of `granted`.
pub fn has_scopes(granted: &[String], required: &[String]) -> bool {
    let granted: HashSet<&str> = granted.iter().map(String::as_str).collect();
    required.iter().all(|s| granted.contains(s.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::openclaw_protocol::{ConnectParams, OpenClawAuth, OpenClawClientInfo};

    fn make_connect_params(token: &str) -> ConnectParams {
        ConnectParams {
            min_protocol: 3,
            max_protocol: 3,
            client: OpenClawClientInfo {
                id: "cli".to_string(),
                display_name: None,
                version: "0.0".to_string(),
                platform: "test".to_string(),
                device_family: None,
                mode: "cli".to_string(),
                instance_id: None,
            },
            caps: Vec::new(),
            auth: Some(OpenClawAuth {
                token: Some(token.to_string()),
                ..Default::default()
            }),
            role: Some("operator".to_string()),
            scopes: vec!["operator.read".to_string()],
            device: None,
            locale: None,
            user_agent: None,
        }
    }

    #[test]
    fn authenticate_connect_accepts_valid_admin_token() {
        let params = make_connect_params("admin-secret");
        let approved = std::collections::HashMap::new();
        let result = authenticate_connect(&params, "admin-secret", &approved);
        assert!(matches!(result, AuthResult::Admin { .. }));
    }

    #[test]
    fn authenticate_connect_rejects_invalid_admin_token() {
        let params = make_connect_params("wrong");
        let approved = std::collections::HashMap::new();
        let result = authenticate_connect(&params, "admin-secret", &approved);
        assert!(matches!(result, AuthResult::Denied(_)));
    }

    #[test]
    fn authenticate_connect_rejects_empty_admin_token() {
        let params = make_connect_params("");
        let approved = std::collections::HashMap::new();
        let result = authenticate_connect(&params, "", &approved);
        assert!(matches!(result, AuthResult::Denied(_)));
    }

    #[test]
    fn has_scopes_true_when_superset() {
        assert!(has_scopes(
            &["operator.read".to_string(), "operator.write".to_string()],
            &["operator.read".to_string()]
        ));
    }

    #[test]
    fn has_scopes_false_when_missing() {
        assert!(!has_scopes(
            &["operator.read".to_string()],
            &["operator.write".to_string()]
        ));
    }

    #[test]
    fn validate_pair_request_extracts_device_id_and_public_key() {
        // 32 zero bytes encoded with base64url (no padding).
        let public_key = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
        let value = serde_json::json!({
            "deviceId": "dev-1",
            "publicKey": public_key
        });
        let result = validate_pair_request(&value).unwrap();
        assert_eq!(result.0, "dev-1");
        assert_eq!(result.1, public_key);
    }

    #[test]
    fn validate_pair_request_rejects_invalid_public_key_length() {
        let value = serde_json::json!({
            "deviceId": "dev-1",
            "publicKey": "aGVsbG8" // 5 bytes, not 32
        });
        let result = validate_pair_request(&value);
        assert!(result.is_err());
    }
}
