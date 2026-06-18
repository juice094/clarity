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
//! Devices are discovered from multiple sources and aggregated into a
//! single `DeviceState`. Per-device connection parameters drive the
//! behaviour of the Settings / Workspace / Terminal / WebBridge panels.
//!
//! # Config sources (priority order)
//!
//! 1. **ZeroClaw** — local clarity-claw daemon (if running)
//! 2. **Local OpenClaw** — `~/.kimi_openclaw/openclaw.json` + paired devices
//! 3. **Remote OpenClaw** — cloud server via Tailscale (`OPENCLAW_REMOTE` env)

use crate::stores::ui::{BotInstance, BotStatus};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

// ── Claw type ──────────────────────────────────────────────────────────

/// Protocol family of a Claw device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClawType {
    /// Clarity-native claw (clarity-claw daemon).
    ZeroClaw,
    /// Kimi OpenClaw Gateway.
    OpenClaw,
}

/// Per-device connection parameters.
#[derive(Clone, Debug)]
pub struct ClawConnection {
    pub claw_type: ClawType,
    /// WebSocket or HTTP URL of the Gateway.
    pub gateway_url: String,
    /// Auth token (empty = no auth).
    pub gateway_token: String,
    /// Local workspace path (may not exist for remote devices).
    pub workspace_root: PathBuf,
    /// Display hostname / IP (used for SSH/terminal connections).
    #[allow(dead_code)]
    pub host: String,
}

// ── OpenClaw config types ──────────────────────────────────────────────

#[derive(Deserialize, Default)]
struct OpenClawConfig {
    gateway: Option<GatewaySection>,
    agents: Option<AgentSection>,
}

#[derive(Deserialize, Default)]
struct GatewaySection {
    #[serde(default)]
    auth: AuthSection,
}

#[derive(Deserialize, Default)]
struct AuthSection {
    #[serde(default)]
    token: String,
}

#[derive(Deserialize, Default)]
struct AgentSection {
    defaults: Option<AgentDefaults>,
}

#[derive(Deserialize, Default)]
struct AgentDefaults {
    #[serde(default)]
    workspace: Option<String>,
}

#[derive(Deserialize)]
struct PairedDevice {
    #[serde(rename = "deviceId")]
    device_id: String,
    platform: Option<String>,
    #[serde(rename = "clientId")]
    client_id: Option<String>,
    #[serde(rename = "clientMode")]
    #[allow(dead_code)]
    client_mode: Option<String>,
    #[allow(dead_code)]
    role: Option<String>,
}

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
pub fn discover() -> DeviceState {
    let state = DeviceState::default();
    let hostname = local_hostname();

    // ── Source 1: ZeroClaw (local clarity-claw) ─────────────────────
    discover_zeroclaw(&state, &hostname);

    // ── Source 2: Local OpenClaw (Kimi Desktop) ─────────────────────
    let oc_home = resolve_openclaw_home();
    let oc_config = read_openclaw_config(&oc_home);
    discover_local_openclaw(&state, &oc_home, &oc_config, &hostname);

    // ── Source 3: Remote OpenClaw (cloud via Tailscale) ─────────────
    discover_remote_openclaw(&state, &hostname);

    // ── Ultimate fallback ───────────────────────────────────────────
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
            },
        );
    }

    state
}

// ── Source 1: ZeroClaw ────────────────────────────────────────────────

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
        },
    );
}

// ── Source 2: Local OpenClaw (Kimi Desktop) ───────────────────────────

fn discover_local_openclaw(
    state: &DeviceState,
    oc_home: &std::path::Path,
    oc_config: &OpenClawConfig,
    hostname: &str,
) {
    let token = oc_config
        .gateway
        .as_ref()
        .map(|g| g.auth.token.clone())
        .unwrap_or_default();

    let workspace = oc_config
        .agents
        .as_ref()
        .and_then(|a| a.defaults.as_ref())
        .and_then(|d| d.workspace.clone())
        .map(PathBuf::from)
        .or_else(|| {
            let ws = oc_home.join("workspace");
            if ws.exists() { Some(ws) } else { None }
        })
        .unwrap_or_else(|| PathBuf::from("."));

    // Register the local OpenClaw Gateway as a device.
    let gateway_id = "openclaw-local-gateway";
    state.register(
        BotInstance {
            id: gateway_id.into(),
            name: format!("{} (OpenClaw)", hostname),
            device_id: "127.0.0.1:18679".into(),
            status: BotStatus::Online,
            version: String::new(),
            last_backup: String::new(),
        },
        ClawConnection {
            claw_type: ClawType::OpenClaw,
            gateway_url: "ws://127.0.0.1:18679".into(),
            gateway_token: token.clone(),
            workspace_root: workspace.clone(),
            host: "127.0.0.1".into(),
        },
    );

    // Register paired devices from devices/paired.json.
    let devices_path = oc_home.join("devices").join("paired.json");
    if let Ok(raw) = std::fs::read_to_string(&devices_path) {
        if let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&raw) {
            for (_key, val) in &map {
                if let Ok(pd) = serde_json::from_value::<PairedDevice>(val.clone()) {
                    let name = pd
                        .client_id
                        .clone()
                        .unwrap_or_else(|| pd.device_id[..12].to_string());
                    let platform = pd.platform.unwrap_or_else(|| "unknown".into());
                    state.register(
                        BotInstance {
                            id: pd.device_id.clone(),
                            name: format!("{} ({})", name, platform),
                            device_id: pd.device_id.clone(),
                            status: BotStatus::Online,
                            version: String::new(),
                            last_backup: String::new(),
                        },
                        ClawConnection {
                            claw_type: ClawType::OpenClaw,
                            gateway_url: "ws://127.0.0.1:18679".into(),
                            gateway_token: token.clone(),
                            workspace_root: workspace.clone(),
                            host: pd.device_id,
                        },
                    );
                }
            }
        }
    }
}

// ── Source 3: Remote OpenClaw (cloud via Tailscale) ───────────────────

fn discover_remote_openclaw(state: &DeviceState, _hostname: &str) {
    // Remote OpenClaw is configured purely through environment variables.
    // No defaults are baked in, to avoid leaking gateway addresses or tokens.
    let Ok(remote_url) = std::env::var("OPENCLAW_REMOTE_URL") else {
        return;
    };
    let remote_token = std::env::var("OPENCLAW_REMOTE_TOKEN").unwrap_or_default();

    // Derive a display host from the URL for terminal/workspace labels.
    let host = remote_url
        .trim_start_matches("wss://")
        .trim_start_matches("ws://")
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split_once(':')
        .map(|(h, _)| h.to_string())
        .unwrap_or_else(|| remote_url.clone());

    let gateway_id = "openclaw-remote-gray";
    state.register(
        BotInstance {
            id: gateway_id.into(),
            name: "Gray-Cloud (OpenClaw)".into(),
            device_id: remote_url.clone(),
            status: if remote_token.is_empty() {
                BotStatus::Offline
            } else {
                BotStatus::Online
            },
            version: "2026.3.13".into(),
            last_backup: String::new(),
        },
        ClawConnection {
            claw_type: ClawType::OpenClaw,
            gateway_url: remote_url,
            gateway_token: remote_token,
            workspace_root: PathBuf::from("."), // remote — accessed via Gateway API
            host,
        },
    );
}

// ── Helpers ────────────────────────────────────────────────────────────

fn local_hostname() -> String {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .unwrap_or_else(|_| "unknown".into())
}

fn resolve_openclaw_home() -> PathBuf {
    if let Ok(h) = std::env::var("OPENCLAW_HOME") {
        let p = PathBuf::from(&h);
        if p.exists() {
            return p;
        }
    }
    let home = if cfg!(target_os = "windows") {
        std::env::var("USERPROFILE").unwrap_or_else(|_| "C:".into())
    } else {
        std::env::var("HOME").unwrap_or_else(|_| "/".into())
    };
    PathBuf::from(home).join(".kimi_openclaw")
}

fn read_openclaw_config(oc_home: &std::path::Path) -> OpenClawConfig {
    let config_path = oc_home.join("openclaw.json");
    if let Ok(raw) = std::fs::read_to_string(&config_path) {
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        OpenClawConfig::default()
    }
}
