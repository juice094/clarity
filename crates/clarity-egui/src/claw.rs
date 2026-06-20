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
use crate::ui::types::DeviceAffinity;
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

// Re-export UI-agnostic types from the shared crate so existing panels can keep
// using `crate::claw::ClawType` / `crate::claw::ClawConnection`.
pub use clarity_openclaw::types::{ClawConnection, ClawType};

// ── DeviceState ────────────────────────────────────────────────────────

/// Aggregated device list + per-device connection parameters.
///
/// Devices are stored grouped by their `role` so that role-based routing can
/// pick an instance without scanning a flat list on every frame.
#[derive(Clone)]
pub struct DeviceState {
    /// role → devices with that role
    roles: Arc<RwLock<HashMap<String, Vec<BotInstance>>>>,
    /// device_id → connection info
    connections: Arc<RwLock<HashMap<String, ClawConnection>>>,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            roles: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl DeviceState {
    /// Snapshot of the current device list for the UI thread.
    ///
    /// Returns a flat, deterministic ordering: sorted role name, then sorted
    /// device id. This preserves compatibility with panels that expect a
    /// one-dimensional list.
    pub fn snapshot(&self) -> Vec<BotInstance> {
        let guard = match self.roles.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        let mut role_names: Vec<String> = guard.keys().cloned().collect();
        role_names.sort();
        let mut out = Vec::new();
        for role in role_names {
            if let Some(devices) = guard.get(&role) {
                let mut sorted = devices.clone();
                sorted.sort_by(|a, b| a.id.cmp(&b.id));
                out.extend(sorted);
            }
        }
        out
    }

    /// Snapshot grouped by role, useful for UI sections that render devices
    /// under role headings.
    pub fn snapshot_grouped(&self) -> Vec<(String, Vec<BotInstance>)> {
        let guard = match self.roles.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };
        let mut role_names: Vec<String> = guard.keys().cloned().collect();
        role_names.sort();
        role_names
            .into_iter()
            .filter_map(|role| {
                let mut devices = guard.get(&role)?.clone();
                devices.sort_by(|a, b| a.id.cmp(&b.id));
                Some((role, devices))
            })
            .collect()
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

    /// Pick a bot instance for a Claw session according to the requested role
    /// and affinity.
    ///
    /// - `Specific(device_id)`: returns the device with that id, preferring the
    ///   requested role but falling back to a search across all roles. If the
    ///   device is offline it is still returned in Stage 2.
    /// - `AnyOnline`: returns the first device in the requested role whose
    ///   status is `Online` or `Syncing`.
    pub fn pick_instance(&self, role: &str, affinity: &DeviceAffinity) -> Option<BotInstance> {
        let guard = self.roles.read().ok()?;
        match affinity {
            DeviceAffinity::Specific(device_id) => {
                // Prefer the requested device if it is still alive.
                let preferred = guard.get(role).into_iter();
                let others = guard
                    .iter()
                    .filter(|(r, _)| r.as_str() != role)
                    .map(|(_, v)| v);
                if let Some(device) = preferred
                    .chain(others)
                    .flat_map(|v| v.iter())
                    .find(|b| b.id == *device_id && !matches!(b.status, BotStatus::Offline))
                {
                    return Some(device.clone());
                }
                // Pinned device is offline or missing — failover to any online/syncing
                // instance of the requested role.
                guard.get(role).and_then(|devices| {
                    devices
                        .iter()
                        .find(|b| matches!(b.status, BotStatus::Online | BotStatus::Syncing))
                        .cloned()
                })
            }
            DeviceAffinity::AnyOnline => guard.get(role).and_then(|devices| {
                devices
                    .iter()
                    .find(|b| matches!(b.status, BotStatus::Online | BotStatus::Syncing))
                    .cloned()
            }),
        }
    }

    /// Update the status of a device by id.
    ///
    /// Called by the connection loop when a device goes offline or comes back
    /// online.
    pub fn update_status(&self, device_id: &str, status: BotStatus) {
        if let Ok(mut guard) = self.roles.write() {
            for devices in guard.values_mut() {
                if let Some(device) = devices.iter_mut().find(|d| d.id == device_id) {
                    device.status = status;
                    break;
                }
            }
        }
    }

    /// Add a device with its connection info.
    fn register(&self, device: BotInstance, conn: ClawConnection) {
        if let Ok(mut guard) = self.roles.write() {
            guard
                .entry(device.role.clone())
                .or_default()
                .push(device.clone());
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
                role: "operator".into(),
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
                role: "operator".into(),
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
                role: "operator".into(),
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
            role: "operator".into(),
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
            role: "operator".into(),
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

    fn bot(id: &str, role: &str, status: BotStatus) -> BotInstance {
        BotInstance {
            id: id.into(),
            name: format!("Bot {}", id),
            device_id: id.into(),
            role: role.into(),
            status,
            version: "0.0.0".into(),
            last_backup: String::new(),
        }
    }

    fn conn(id: &str) -> ClawConnection {
        ClawConnection {
            claw_type: ClawType::ZeroClaw,
            gateway_url: format!("http://{}", id),
            gateway_token: String::new(),
            workspace_root: std::env::current_dir().unwrap_or_default(),
            host: id.into(),
            auth_mode: None,
            device_token: None,
        }
    }

    #[test]
    fn test_devices_grouped_by_role() {
        let state = DeviceState::default();
        state.register(bot("a-op-1", "operator", BotStatus::Online), conn("a-op-1"));
        state.register(
            bot("a-coder-1", "coder", BotStatus::Online),
            conn("a-coder-1"),
        );
        state.register(
            bot("a-op-2", "operator", BotStatus::Offline),
            conn("a-op-2"),
        );

        let grouped = state.snapshot_grouped();
        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[0].0, "coder");
        assert_eq!(grouped[0].1.len(), 1);
        assert_eq!(grouped[1].0, "operator");
        assert_eq!(grouped[1].1.len(), 2);

        // Flat snapshot is sorted by role then id.
        let flat = state.snapshot();
        let ids: Vec<_> = flat.iter().map(|b| b.id.as_str()).collect();
        assert_eq!(ids, vec!["a-coder-1", "a-op-1", "a-op-2"]);
    }

    #[test]
    fn test_pick_instance_specific() {
        let state = DeviceState::default();
        state.register(bot("op-1", "operator", BotStatus::Online), conn("op-1"));
        state.register(bot("coder-1", "coder", BotStatus::Online), conn("coder-1"));

        let picked = state
            .pick_instance("operator", &DeviceAffinity::Specific("op-1".into()))
            .expect("finds specific operator device");
        assert_eq!(picked.id, "op-1");

        // Searching within a role that does not contain the id falls back to
        // other roles.
        let cross = state
            .pick_instance("operator", &DeviceAffinity::Specific("coder-1".into()))
            .expect("falls back across roles");
        assert_eq!(cross.id, "coder-1");

        // A missing pinned id falls back to the first online/syncing device in
        // the requested role.
        let missing = state
            .pick_instance("operator", &DeviceAffinity::Specific("missing".into()))
            .expect("falls back to online device in role");
        assert_eq!(missing.id, "op-1");
    }

    #[test]
    fn test_pick_instance_any_online() {
        let state = DeviceState::default();
        state.register(
            bot("op-off", "operator", BotStatus::Offline),
            conn("op-off"),
        );
        state.register(
            bot("op-sync", "operator", BotStatus::Syncing),
            conn("op-sync"),
        );
        state.register(bot("op-on", "operator", BotStatus::Online), conn("op-on"));

        let picked = state
            .pick_instance("operator", &DeviceAffinity::AnyOnline)
            .expect("finds online or syncing device");
        // Devices are kept in registration order; the first online/syncing
        // device is op-sync.
        assert_eq!(picked.id, "op-sync");
        assert!(matches!(
            picked.status,
            BotStatus::Online | BotStatus::Syncing
        ));
    }

    #[test]
    fn test_pick_instance_specific_failover_to_online() {
        let state = DeviceState::default();
        state.register(
            bot("op-off", "operator", BotStatus::Offline),
            conn("op-off"),
        );
        state.register(
            bot("op-sync", "operator", BotStatus::Syncing),
            conn("op-sync"),
        );
        state.register(bot("op-on", "operator", BotStatus::Online), conn("op-on"));

        // Pinned offline device fails over to the first online/syncing device
        // in the same role.
        let picked = state
            .pick_instance("operator", &DeviceAffinity::Specific("op-off".into()))
            .expect("fails over to online/syncing device");
        assert_eq!(picked.id, "op-sync");

        // A missing pinned id also falls back to the first online/syncing role
        // device.
        let missing = state
            .pick_instance("operator", &DeviceAffinity::Specific("missing".into()))
            .expect("fails over to online/syncing device");
        assert_eq!(missing.id, "op-sync");
    }

    #[test]
    fn test_update_status() {
        let state = DeviceState::default();
        state.register(bot("op-1", "operator", BotStatus::Online), conn("op-1"));

        state.update_status("op-1", BotStatus::Offline);
        let flat = state.snapshot();
        assert_eq!(flat[0].status, BotStatus::Offline);

        state.update_status("missing", BotStatus::Syncing);
        // No panic and existing device unchanged.
        assert_eq!(state.snapshot()[0].status, BotStatus::Offline);
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
