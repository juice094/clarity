//! OpenClaw JSON-RPC WebSocket frame types.
//!
//! Kimi Desktop's local gateway and Clarity's OpenClaw-compatible endpoint speak
//! a typed JSON-RPC dialect over WebSocket. All frames carry a top-level `type`
//! discriminator:
//!
//! - `req`:  client → server method call
//! - `res`:  server → client method response
//! - `event`: server → client unsolicited event
//!
//! The first frame is always `connect.challenge` (an event); the client must
//! answer with a `connect` request.

use serde::{Deserialize, Serialize};

/// A single frame on the OpenClaw Gateway wire.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OpenClawFrame {
    /// Client → server method invocation.
    Req {
        /// Request id; echoed in the matching `res`.
        id: String,
        /// Method name, e.g. `connect`, `chat.send`.
        method: String,
        /// Method parameters.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        params: Option<serde_json::Value>,
    },
    /// Server → client response.
    Res {
        /// Request id matching the original `req`.
        id: String,
        /// Whether the call succeeded.
        ok: bool,
        /// Success payload when `ok` is true.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payload: Option<serde_json::Value>,
        /// Error payload when `ok` is false.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        error: Option<OpenClawErrorShape>,
    },
    /// Server → client event.
    Event {
        /// Event name, e.g. `connect.challenge`, `chat`, `health`.
        event: String,
        /// Event payload.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        payload: Option<serde_json::Value>,
        /// Monotonic sequence number for gap detection.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        seq: Option<u64>,
    },
}

/// Error object returned inside a `res` frame.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenClawErrorShape {
    /// Machine-readable error code.
    pub code: String,
    /// Human-readable message.
    pub message: String,
    /// Optional extra details (codes, retry hints, etc.).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
    /// Whether the caller should retry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retryable: Option<bool>,
    /// Recommended delay before retry, in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_after_ms: Option<u64>,
}

/// `connect.challenge` event payload.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectChallenge {
    /// Nonce that must be signed/echoed in the `connect` request.
    pub nonce: String,
    /// Server timestamp in milliseconds since Unix epoch.
    pub ts: u64,
}

/// `connect` request parameters.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConnectParams {
    /// Minimum supported protocol version (use 3).
    pub min_protocol: u8,
    /// Maximum supported protocol version (use 3).
    pub max_protocol: u8,
    /// Client identification.
    pub client: OpenClawClientInfo,
    /// Capability flags.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caps: Vec<String>,
    /// Authentication credentials.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<OpenClawAuth>,
    /// Client role, e.g. `operator`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    /// Requested scopes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub scopes: Vec<String>,
    /// Device identity proof (required for paired-device connections).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device: Option<OpenClawDeviceProof>,
    /// Locale string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locale: Option<String>,
    /// User agent string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

/// Client identification sent during `connect`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenClawClientInfo {
    /// Client id, e.g. `cli`, `openclaw-control-ui`, `gateway-client`.
    pub id: String,
    /// Human-readable display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// Client version.
    pub version: String,
    /// Platform, e.g. `win32`, `darwin`, `linux`.
    pub platform: String,
    /// Device family, e.g. `Windows`, `Mac`, `Desktop`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_family: Option<String>,
    /// Client mode, e.g. `cli`, `ui`, `backend`, `webchat`.
    pub mode: String,
    /// Optional instance id for multi-process clients.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub instance_id: Option<String>,
}

/// Authentication credentials sent during `connect`.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct OpenClawAuth {
    /// Admin or gateway token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Bootstrap token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_token: Option<String>,
    /// Device token obtained after pairing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_token: Option<String>,
    /// Password (when gateway uses password auth).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
    /// Device identity public key (base64url-encoded Ed25519 public key).
    /// Some Gateways accept this alongside `device_token` to identify a paired
    /// device without requiring a full signed device proof.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_identity: Option<String>,
}

/// Device identity proof sent during `connect`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenClawDeviceProof {
    /// Device id (SHA-256 of raw public key, hex).
    pub id: String,
    /// Base64url-encoded raw Ed25519 public key.
    pub public_key: String,
    /// Base64url-encoded Ed25519 signature of the v3 payload.
    pub signature: String,
    /// Timestamp used in the signed payload.
    pub signed_at: u64,
    /// Nonce echoed from the challenge.
    pub nonce: String,
}

/// Server information inside `hello-ok`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawServerInfo {
    /// Server version string.
    pub version: String,
    /// Connection id.
    pub conn_id: String,
}

/// Supported methods/events inside `hello-ok`.
#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct OpenClawFeatures {
    /// Supported RPC methods.
    pub methods: Vec<String>,
    /// Supported unsolicited events.
    pub events: Vec<String>,
}

/// Connection policy limits inside `hello-ok`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenClawPolicy {
    /// Maximum payload size in bytes.
    pub max_payload: usize,
    /// Maximum buffered bytes.
    pub max_buffered_bytes: usize,
    /// Tick interval in milliseconds.
    pub tick_interval_ms: u64,
}

/// `hello-ok` payload returned by a successful `connect`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HelloOk {
    /// Always `hello-ok`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Negotiated protocol version.
    pub protocol: u8,
    /// Server information.
    pub server: OpenClawServerInfo,
    /// Supported methods and events.
    pub features: OpenClawFeatures,
    /// Connection policy limits.
    pub policy: OpenClawPolicy,
    /// Optional post-connection auth info (device token, granted scopes).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub auth: Option<HelloOkAuth>,
    /// Optional canvas host URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canvas_host_url: Option<String>,
}

/// Auth information returned inside `hello-ok`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HelloOkAuth {
    /// Device token to use on subsequent reconnects.
    pub device_token: String,
    /// Granted role.
    pub role: String,
    /// Granted scopes.
    pub scopes: Vec<String>,
    /// Token issue timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub issued_at_ms: Option<u64>,
}

/// Build the v3 device-auth payload that gets signed.
///
/// Format:
/// `v3|deviceId|clientId|clientMode|role|scopes|signedAtMs|token|nonce|platform|deviceFamily`
#[allow(clippy::too_many_arguments)]
pub fn build_device_auth_payload(
    device_id: &str,
    client_id: &str,
    client_mode: &str,
    role: &str,
    scopes: &[String],
    signed_at_ms: u64,
    token: &str,
    nonce: &str,
    platform: &str,
    device_family: Option<&str>,
) -> String {
    let parts = [
        "v3",
        device_id,
        client_id,
        client_mode,
        role,
        &scopes.join(","),
        &signed_at_ms.to_string(),
        token,
        nonce,
        platform,
        device_family.unwrap_or(""),
    ];
    parts.join("|")
}

/// Standard OpenClaw method names.
pub mod methods {
    /// List sessions.
    pub const SESSIONS_LIST: &str = "sessions.list";
    /// Preview a session.
    pub const SESSIONS_PREVIEW: &str = "sessions.preview";
    /// Patch a session.
    pub const SESSIONS_PATCH: &str = "sessions.patch";
    /// Reset a session.
    pub const SESSIONS_RESET: &str = "sessions.reset";
    /// Delete a session.
    pub const SESSIONS_DELETE: &str = "sessions.delete";
    /// Compact a session.
    pub const SESSIONS_COMPACT: &str = "sessions.compact";
    /// Send a chat message.
    pub const CHAT_SEND: &str = "chat.send";
    /// Fetch chat history.
    pub const CHAT_HISTORY: &str = "chat.history";
    /// Abort an in-flight chat.
    pub const CHAT_ABORT: &str = "chat.abort";
    /// Request device pairing.
    pub const DEVICE_PAIR_REQUEST: &str = "device.pair.request";
    /// List pending/approved devices.
    pub const DEVICE_PAIR_LIST: &str = "device.pair.list";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_req_serializes_with_type() {
        let f = OpenClawFrame::Req {
            id: "r1".into(),
            method: "connect".into(),
            params: None,
        };
        let json = serde_json::to_string(&f).unwrap();
        assert!(json.contains("\"type\":\"req\""));
        assert!(json.contains("\"method\":\"connect\""));
    }

    #[test]
    fn frame_event_deserializes_challenge() {
        let json =
            r#"{"type":"event","event":"connect.challenge","payload":{"nonce":"n1","ts":1234}}"#;
        let frame: OpenClawFrame = serde_json::from_str(json).unwrap();
        match frame {
            OpenClawFrame::Event { event, payload, .. } => {
                assert_eq!(event, "connect.challenge");
                let challenge: ConnectChallenge = serde_json::from_value(payload.unwrap()).unwrap();
                assert_eq!(challenge.nonce, "n1");
                assert_eq!(challenge.ts, 1234);
            }
            _ => panic!("expected event"),
        }
    }

    #[test]
    fn v3_payload_format_matches_openclaw() {
        let payload = build_device_auth_payload(
            "did",
            "cid",
            "backend",
            "operator",
            &["operator.admin".into(), "operator.read".into()],
            42,
            "tok",
            "nonce",
            "win32",
            Some("Windows"),
        );
        assert_eq!(
            payload,
            "v3|did|cid|backend|operator|operator.admin,operator.read|42|tok|nonce|win32|Windows"
        );
    }

    #[test]
    fn hello_ok_deserializes_features() {
        let json = r#"{
            "type":"hello-ok",
            "protocol":3,
            "server":{"version":"2026.3.13","connId":"c1"},
            "features":{"methods":["chat.send"],"events":["chat"]},
            "policy":{"maxPayload":26214400,"maxBufferedBytes":52428800,"tickIntervalMs":30000}
        }"#;
        let hello: HelloOk = serde_json::from_str(json).unwrap();
        assert_eq!(hello.protocol, 3);
        assert!(hello.features.methods.contains(&"chat.send".to_string()));
        assert_eq!(hello.policy.tick_interval_ms, 30000);
    }
}
