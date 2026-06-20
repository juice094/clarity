//! Distributed Claw device integration.
//!
//! # Architecture
//!
//! Clarity manages a **distributed Claw network** with two protocol families:
//!
//! | Type | Runtime | Gateway | Typical location |
//! |------|---------|---------|-----------------|
//! | `ZeroClaw` | clarity-claw | clarity-gateway (:18790) | Local machine |
//! | `OpenClaw` | kimi-openclaw | OpenClaw Gateway (:18789) | Local + Cloud (Tailscale) |
//!
//! This module is the egui-specific adapter. The UI-agnostic OpenClaw client,
//! device identity and discovery live in the dedicated `clarity-openclaw` crate.
//! Per-device connection parameters drive the behaviour of the Settings /
//! Workspace / Terminal / WebBridge panels.
//!
//! # Config sources (priority order)
//!
//! 1. **ZeroClaw** — local clarity-claw daemon (always registered as fallback)
//! 2. **Local OpenClaw** — `~/.kimi_openclaw/openclaw.json` + paired devices
//! 3. **Remote OpenClaw (env)** — `OPENCLAW_REMOTE_URL` / `OPENCLAW_REMOTE_TOKEN`
//! 4. **Remote OpenClaw (settings)** — user-configured `GuiSettings::openclaw_connections`
//! 5. **Persisted pairing** — `~/.clarity/claw-device-token.json`

use crate::settings::{GuiSettings, OpenClawAuthMode, OpenClawConnection};
use crate::stores::ui::{BotInstance, BotStatus};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// Re-export UI-agnostic types from the shared crate so existing panels can keep
// using `crate::claw::ClawType` / `crate::claw::ClawConnection`.
pub use clarity_openclaw::types::{ClawConnection, ClawType};

// ── DeviceState ────────────────────────────────────────────────────────

/// Aggregated device list + per-device connection parameters.
#[derive(Clone)]
pub struct DeviceState {
    devices: Arc<RwLock<Vec<BotInstance>>>,
    /// device_id → connection info
    connections: Arc<RwLock<HashMap<String, ClawConnection>>>,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            devices: Arc::new(RwLock::new(Vec::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl DeviceState {
    /// Snapshot of the current device list for the UI thread.
    pub fn snapshot(&self) -> Vec<BotInstance> {
        self.devices.read().map(|g| g.clone()).unwrap_or_default()
    }

    /// Look up connection parameters for a device.
    pub fn connection(&self, device_id: &str) -> Option<ClawConnection> {
        self.connections.read().ok()?.get(device_id).cloned()
    }

    /// Connection for the active device, or the first available.
    pub fn active_connection(&self, active_bot_id: &str) -> Option<ClawConnection> {
        self.connection(active_bot_id).or_else(|| {
            let devs = self.snapshot();
            devs.first().and_then(|d| self.connection(&d.id))
        })
    }

    /// Add a device with its connection info.
    fn register(&self, device: BotInstance, conn: ClawConnection) {
        if let Ok(mut devs) = self.devices.write() {
            devs.push(device.clone());
        }
        if let Ok(mut conns) = self.connections.write() {
            conns.insert(device.id, conn);
        }
    }
}

// ── Discovery ──────────────────────────────────────────────────────────

/// Discover all Claw devices from all sources.
///
/// `settings_connections` comes from `GuiSettings::openclaw_connections` and is
/// re-evaluated on every app launch so users can add/remove remote Gateways
/// without recompiling.
pub fn discover(settings_connections: &[OpenClawConnection]) -> DeviceState {
    let state = DeviceState::default();
    let hostname = local_hostname();

    // Source 1: ZeroClaw (local clarity-claw).
    discover_zeroclaw(&state, &hostname);

    // Source 2 & 3: Local and remote OpenClaw via the shared crate
    // (local config + OPENCLAW_REMOTE_* env vars).
    for record in clarity_openclaw::discovery::discover_openclaw_devices(&hostname) {
        state.register(
            BotInstance {
                id: record.info.id,
                name: record.info.name,
                device_id: record.info.device_id,
                status: map_status(record.info.status),
                version: record.info.version,
                last_backup: String::new(),
            },
            record.connection,
        );
    }

    // Source 4: User-configured remote OpenClaw connections from settings.
    discover_settings_openclaw(&state, settings_connections);

    // Source 5: Persisted paired token (e.g. a remote private Claw Gateway).
    discover_saved_openclaw(&state);

    // Ultimate fallback.
    if state.snapshot().is_empty() {
        state.register(
            BotInstance {
                id: hostname.clone(),
                name: hostname,
                device_id: "127.0.0.1".into(),
                status: BotStatus::Online,
                version: env!("CARGO_PKG_VERSION").into(),
                last_backup: String::new(),
            },
            ClawConnection {
                claw_type: ClawType::ZeroClaw,
                gateway_url: "http://127.0.0.1:18790".into(),
                gateway_token: String::new(),
                workspace_root: std::env::current_dir().unwrap_or_default(),
                host: "127.0.0.1".into(),
                auth_mode: None,
                device_token: None,
            },
        );
    }

    state
}

fn discover_settings_openclaw(state: &DeviceState, connections: &[OpenClawConnection]) {
    let existing = state.snapshot();
    for conn in connections
        .iter()
        .filter(|c| c.enabled && !c.gateway_url.is_empty())
    {
        let normalized = normalize_gateway_url(&conn.gateway_url);
        if existing.iter().any(|d| {
            state
                .connection(&d.id)
                .map(|c| normalize_gateway_url(&c.gateway_url) == normalized)
                .unwrap_or(false)
        }) {
            continue;
        }

        let name = if conn.name.is_empty() {
            format!("OpenClaw ({})", conn.gateway_url)
        } else {
            conn.name.clone()
        };
        let host = gateway_host(&conn.gateway_url).unwrap_or_else(|| "openclaw".into());
        let id = format!("openclaw-settings-{}", host);
        let auth_mode = Some(match conn.auth_mode {
            OpenClawAuthMode::TokenOnly => "token_only".into(),
            OpenClawAuthMode::TokenWithDevice => "token_with_device".into(),
            OpenClawAuthMode::DevicePaired => "device_paired".into(),
        });
        state.register(
            BotInstance {
                id: id.clone(),
                name,
                device_id: id,
                status: BotStatus::Online,
                version: env!("CARGO_PKG_VERSION").into(),
                last_backup: String::new(),
            },
            ClawConnection {
                claw_type: ClawType::OpenClaw,
                gateway_url: conn.gateway_url.clone(),
                gateway_token: GuiSettings::resolve_api_key(&Some(conn.token.clone()))
                    .unwrap_or_default(),
                workspace_root: std::env::current_dir().unwrap_or_default(),
                host,
                auth_mode,
                device_token: GuiSettings::resolve_api_key(&conn.device_token),
            },
        );
    }
}

fn discover_saved_openclaw(state: &DeviceState) {
    let paired = match clarity_openclaw::load_paired_token() {
        Ok(Some(p)) => p,
        Ok(None) => return,
        Err(e) => {
            tracing::warn!("Failed to load saved OpenClaw token: {}", e);
            return;
        }
    };

    // Avoid duplicating an already-discovered Gateway.
    let existing = state.snapshot();
    let normalized = normalize_gateway_url(&paired.gateway_url);
    if existing.iter().any(|d| {
        state
            .connection(&d.id)
            .map(|c| normalize_gateway_url(&c.gateway_url) == normalized)
            .unwrap_or(false)
    }) {
        return;
    }

    let gateway_url = paired.gateway_url.clone();
    let host = gateway_host(&gateway_url).unwrap_or_else(|| "openclaw".into());
    let id = format!("openclaw-saved-{}", host);
    state.register(
        BotInstance {
            id: id.clone(),
            name: format!("OpenClaw Saved ({host})"),
            device_id: id,
            status: BotStatus::Online,
            version: env!("CARGO_PKG_VERSION").into(),
            last_backup: String::new(),
        },
        ClawConnection {
            claw_type: ClawType::OpenClaw,
            gateway_url,
            gateway_token: paired.auth_token().to_string(),
            workspace_root: std::env::current_dir().unwrap_or_default(),
            host,
            auth_mode: Some("device_paired".into()),
            device_token: None,
        },
    );
}

/// Normalize a Gateway URL so that `127.0.0.1` and `localhost` are treated as
/// equivalent and trailing slashes are removed.
pub(crate) fn normalize_gateway_url(url: &str) -> String {
    url.to_ascii_lowercase()
        .replace("127.0.0.1", "localhost")
        .trim_end_matches('/')
        .to_string()
}

/// Convert an HTTP(S) Gateway URL to a WebSocket URL.
pub(crate) fn to_ws_url(url: &str) -> String {
    if url.starts_with("ws://") || url.starts_with("wss://") {
        url.to_string()
    } else {
        url.replace("http://", "ws://")
            .replace("https://", "wss://")
    }
}

fn gateway_host(url: &str) -> Option<String> {
    url.trim_start_matches("ws://")
        .trim_start_matches("wss://")
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split(':')
        .next()
        .map(String::from)
}

fn discover_zeroclaw(state: &DeviceState, hostname: &str) {
    state.register(
        BotInstance {
            id: "zeroclaw-local".into(),
            name: format!("{} (ZeroClaw)", hostname),
            device_id: hostname.into(),
            status: BotStatus::Online,
            version: env!("CARGO_PKG_VERSION").into(),
            last_backup: String::new(),
        },
        ClawConnection {
            claw_type: ClawType::ZeroClaw,
            gateway_url: "http://127.0.0.1:18790".to_string(),
            gateway_token: String::new(),
            workspace_root: std::env::current_dir().unwrap_or_default(),
            host: hostname.into(),
            auth_mode: None,
            device_token: None,
        },
    );
}

fn map_status(status: clarity_openclaw::types::DeviceStatus) -> BotStatus {
    match status {
        clarity_openclaw::types::DeviceStatus::Online => BotStatus::Online,
        clarity_openclaw::types::DeviceStatus::Offline => BotStatus::Offline,
        clarity_openclaw::types::DeviceStatus::Syncing => BotStatus::Syncing,
    }
}

fn local_hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::OpenClawAuthMode;

    fn sample_settings_connection() -> OpenClawConnection {
        OpenClawConnection {
            name: "Gray-Cloud".into(),
            gateway_url: "wss://gray-cloud.example:18789".into(),
            token: "token-with-device".into(),
            auth_mode: OpenClawAuthMode::TokenWithDevice,
            enabled: true,
            device_token: None,
        }
    }

    #[test]
    fn test_discover_settings_preserves_auth_mode() {
        let conn = sample_settings_connection();
        let state = discover(&[conn]);
        let snapshot = state.snapshot();
        let bot = snapshot
            .iter()
            .find(|b| b.name == "Gray-Cloud")
            .expect("Gray-Cloud bot registered");
        let c = state.connection(&bot.id).expect("connection exists");
        assert_eq!(c.auth_mode.as_deref(), Some("token_with_device"));
        assert_eq!(c.gateway_token, "token-with-device");
    }

    #[test]
    fn test_discover_settings_device_paired() {
        let mut conn = sample_settings_connection();
        conn.auth_mode = OpenClawAuthMode::DevicePaired;
        conn.device_token = Some("paired-device-token".into());
        let state = discover(&[conn]);
        let snapshot = state.snapshot();
        let bot = snapshot
            .iter()
            .find(|b| b.name == "Gray-Cloud")
            .expect("Gray-Cloud bot registered");
        let c = state.connection(&bot.id).expect("connection exists");
        assert_eq!(c.auth_mode.as_deref(), Some("device_paired"));
        assert_eq!(c.device_token.as_deref(), Some("paired-device-token"));
    }
}
