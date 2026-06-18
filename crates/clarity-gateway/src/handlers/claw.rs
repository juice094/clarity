//! Claw device registry handlers.
//!
//! The Gateway maintains an in-memory registry of connected Claw daemon
//! instances. Each instance registers on startup and sends periodic
//! heartbeats. The egui frontend polls this registry to display live
//! device status in the navigation tree.
//!
//! # Endpoints
//!
//! - `POST /api/v1/claw/devices` — register or heartbeat a device
//! - `GET  /api/v1/claw/devices` — list all registered devices

use crate::server::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

// ── Types ──────────────────────────────────────────────────────────────────

/// A registered Claw device instance.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ClawDevice {
    /// Stable machine identifier (hostname or user-assigned).
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// IP address or hostname of the device.
    pub host: String,
    /// Claw daemon version string.
    pub version: String,
    /// Online / Offline / Syncing.
    pub status: DeviceStatus,
    /// ISO-8601 timestamp of the last heartbeat or status change.
    pub last_heartbeat: String,
}

/// Liveness status of a registered Claw device.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DeviceStatus {
    /// Device is connected and heartbeating.
    Online,
    /// Device has not heartbeated within the expiry window.
    Offline,
    /// Device is online but busy synchronising data.
    Syncing,
}

/// Payload for device registration / heartbeat.
#[derive(Debug, Deserialize)]
pub struct DeviceRegistration {
    /// Stable machine identifier.
    pub id: String,
    /// Human-readable display name.
    pub name: String,
    /// IP address or hostname.
    pub host: String,
    /// Claw daemon version string.
    pub version: String,
    /// Optional status override (e.g. "syncing").
    #[serde(default)]
    pub status: Option<String>,
}

// ── Registry ───────────────────────────────────────────────────────────────

/// Thread-safe in-memory registry of Claw devices.
#[derive(Clone, Default)]
pub struct DeviceRegistry {
    devices: Arc<RwLock<HashMap<String, ClawDevice>>>,
}

impl DeviceRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new device or heartbeat an existing one.
    ///
    /// If the device is already registered, its heartbeat timestamp is
    /// updated. If the status field is provided, it overrides the
    /// existing status.
    pub fn register(&self, reg: DeviceRegistration) -> ClawDevice {
        let now = Utc::now().to_rfc3339();
        let status = match reg.status.as_deref() {
            Some("syncing") => DeviceStatus::Syncing,
            Some("offline") => DeviceStatus::Offline,
            _ => DeviceStatus::Online,
        };
        let device = ClawDevice {
            id: reg.id.clone(),
            name: reg.name,
            host: reg.host,
            version: reg.version,
            status,
            last_heartbeat: now,
        };
        let mut map = self.devices.write();
        map.insert(reg.id, device.clone());
        device
    }

    /// List all registered devices.
    pub fn list(&self) -> Vec<ClawDevice> {
        let map = self.devices.read();
        let mut devices: Vec<_> = map.values().cloned().collect();
        devices.sort_by(|a, b| a.name.cmp(&b.name));
        devices
    }

    /// Mark devices that haven't heartbeated within `timeout_secs` as offline.
    /// Returns the number of devices marked offline.
    pub fn expire_stale(&self, timeout_secs: i64) -> usize {
        let mut map = self.devices.write();
        let cutoff = Utc::now() - chrono::Duration::seconds(timeout_secs);
        let mut expired = 0;
        for device in map.values_mut() {
            if device.status == DeviceStatus::Online
                && let Ok(ts) = DateTime::parse_from_rfc3339(&device.last_heartbeat)
                && ts < cutoff
            {
                device.status = DeviceStatus::Offline;
                expired += 1;
            }
        }
        expired
    }
}

impl std::fmt::Debug for DeviceRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let map = self.devices.read();
        f.debug_struct("DeviceRegistry")
            .field("count", &map.len())
            .finish()
    }
}

// ── Handlers ───────────────────────────────────────────────────────────────

/// `POST /api/v1/claw/devices` — register or heartbeat.
pub async fn register_device(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<DeviceRegistration>,
) -> Result<Json<ClawDevice>, StatusCode> {
    if payload.id.is_empty() || payload.name.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let device = state.device_registry.register(payload);
    tracing::debug!(
        device_id = %device.id,
        device_name = %device.name,
        "Claw device heartbeat"
    );
    Ok(Json(device))
}

/// `GET /api/v1/claw/devices` — list all registered devices.
pub async fn list_devices(State(state): State<Arc<AppState>>) -> Json<Vec<ClawDevice>> {
    Json(state.device_registry.list())
}
