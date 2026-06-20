//! UI-agnostic types for OpenClaw device discovery and connections.

use std::path::PathBuf;

/// Protocol family of a Claw device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClawType {
    /// Clarity-native claw (clarity-claw daemon).
    ZeroClaw,
    /// Kimi OpenClaw Gateway.
    OpenClaw,
}

/// Wire protocol spoken to a Claw Gateway.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ClawProtocol {
    /// OpenClaw / KimiClaw JSON-RPC over WebSocket.
    OpenClawJsonRpc,
    /// Native Clarity Gateway WebSocket protocol (`WsRequest`/`WsResponse`).
    GatewayWebSocket,
}

/// Runtime status of a discovered device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeviceStatus {
    /// Device is reachable and ready.
    Online,
    /// Device is configured but not reachable.
    Offline,
    /// Device is actively synchronizing.
    Syncing,
}

/// UI-agnostic description of a discovered Claw device.
#[derive(Clone, Debug)]
pub struct DeviceInfo {
    /// Stable identifier used for selection and connection lookup.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// Physical/logical device identifier (hostname, URL, or paired device id).
    pub device_id: String,
    /// Current reachability status.
    pub status: DeviceStatus,
    /// Optional version string reported by the device.
    pub version: String,
}

/// Per-device connection parameters.
#[derive(Clone, Debug)]
pub struct ClawConnection {
    /// Protocol family of the device.
    pub claw_type: ClawType,
    /// Wire protocol spoken to the Gateway.
    pub protocol: ClawProtocol,
    /// WebSocket or HTTP URL of the Gateway.
    pub gateway_url: String,
    /// Auth token (empty = no auth).
    pub gateway_token: String,
    /// Local workspace path (may not exist for remote devices).
    pub workspace_root: PathBuf,
    /// Display hostname / IP (used for terminal/workspace labels).
    pub host: String,
    /// Optional auth mode hint from the consumer (`token_only`, `token_with_device`,
    /// `device_paired`). When `None`, the consumer should apply its default policy.
    pub auth_mode: Option<String>,
    /// Optional device-specific token for paired-device auth.
    pub device_token: Option<String>,
}

/// A discovered device together with its connection parameters.
#[derive(Clone, Debug)]
pub struct DeviceRecord {
    /// Device description.
    pub info: DeviceInfo,
    /// Connection parameters.
    pub connection: ClawConnection,
}
