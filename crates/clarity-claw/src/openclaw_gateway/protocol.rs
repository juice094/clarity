//! OpenClaw JSON-RPC WebSocket frame types.
//!
//! This module re-exports the canonical protocol types from
//! `clarity_contract::openclaw_protocol` so that both client and server can
//! share a single source of truth. Client-specific convenience constructors
//! remain here.

pub use clarity_contract::openclaw_protocol::*;

/// Convenience: build `connect` parameters for the no-device CLI mode.
pub fn build_cli_connect_params(
    token: &str,
    platform: &str,
    device_family: Option<&str>,
) -> ConnectParams {
    ConnectParams {
        min_protocol: 3,
        max_protocol: 3,
        client: OpenClawClientInfo {
            id: "cli".to_string(),
            display_name: Some("Clarity Claw CLI".to_string()),
            version: env!("CARGO_PKG_VERSION").to_string(),
            platform: platform.to_string(),
            device_family: device_family.map(String::from),
            mode: "cli".to_string(),
            instance_id: None,
        },
        caps: Vec::new(),
        auth: Some(OpenClawAuth {
            token: Some(token.to_string()),
            ..Default::default()
        }),
        role: Some("operator".to_string()),
        scopes: vec![
            "operator.admin".to_string(),
            "operator.read".to_string(),
            "operator.write".to_string(),
            "operator.approvals".to_string(),
            "operator.pairing".to_string(),
        ],
        device: None,
        locale: None,
        user_agent: None,
    }
}

/// Convenience: build `connect` parameters with a device identity proof.
pub fn build_device_connect_params(
    token: &str,
    platform: &str,
    device_family: Option<&str>,
    device_proof: OpenClawDeviceProof,
) -> ConnectParams {
    ConnectParams {
        min_protocol: 3,
        max_protocol: 3,
        client: OpenClawClientInfo {
            id: "gateway-client".to_string(),
            display_name: Some("Clarity Claw".to_string()),
            version: env!("CARGO_PKG_VERSION").to_string(),
            platform: platform.to_string(),
            device_family: device_family.map(String::from),
            mode: "backend".to_string(),
            instance_id: None,
        },
        caps: Vec::new(),
        auth: Some(OpenClawAuth {
            token: Some(token.to_string()),
            ..Default::default()
        }),
        role: Some("operator".to_string()),
        scopes: vec![
            "operator.admin".to_string(),
            "operator.read".to_string(),
            "operator.write".to_string(),
            "operator.approvals".to_string(),
            "operator.pairing".to_string(),
        ],
        device: Some(device_proof),
        locale: None,
        user_agent: None,
    }
}
