//! Discover OpenClaw Gateways and paired devices.

use crate::types::{
    ClawConnection, ClawProtocol, ClawType, DeviceInfo, DeviceRecord, DeviceStatus,
};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Paired device entry from `~/.kimi_openclaw/devices/paired.json`.
#[derive(Deserialize)]
struct PairedDevice {
    #[serde(rename = "deviceId")]
    device_id: String,
    platform: Option<String>,
    #[serde(rename = "clientId")]
    client_id: Option<String>,
}

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

/// Discover all OpenClaw devices for the current machine.
///
/// Returns local KimiClaw Gateway devices plus any remote OpenClaw device
/// configured via `OPENCLAW_REMOTE_URL` / `OPENCLAW_REMOTE_TOKEN`.
pub fn discover_openclaw_devices(hostname: &str) -> Vec<DeviceRecord> {
    let oc_home = resolve_openclaw_home();
    let oc_config = read_openclaw_config(&oc_home);

    let mut out = Vec::new();
    discover_local_openclaw(hostname, &oc_home, &oc_config, &mut out);
    discover_remote_openclaw(&mut out);
    out
}

fn discover_local_openclaw(
    hostname: &str,
    oc_home: &Path,
    oc_config: &OpenClawConfig,
    out: &mut Vec<DeviceRecord>,
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
    out.push(DeviceRecord {
        info: DeviceInfo {
            id: "openclaw-local-gateway".into(),
            name: format!("{} (OpenClaw)", hostname),
            device_id: "127.0.0.1:18679".into(),
            status: DeviceStatus::Online,
            version: String::new(),
        },
        connection: ClawConnection {
            claw_type: ClawType::OpenClaw,
            protocol: ClawProtocol::OpenClawJsonRpc,
            gateway_url: "ws://127.0.0.1:18679".into(),
            gateway_token: token.clone(),
            workspace_root: workspace.clone(),
            host: "127.0.0.1".into(),
            auth_mode: None,
            device_token: None,
        },
    });

    // Register paired devices from devices/paired.json.
    let devices_path = oc_home.join("devices").join("paired.json");
    if let Ok(raw) = std::fs::read_to_string(&devices_path) {
        if let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&raw) {
            for (_key, val) in &map {
                if let Ok(pd) = serde_json::from_value::<PairedDevice>(val.clone()) {
                    let name = pd
                        .client_id
                        .clone()
                        .unwrap_or_else(|| pd.device_id[..12.min(pd.device_id.len())].to_string());
                    let platform = pd.platform.unwrap_or_else(|| "unknown".into());
                    let device_id = pd.device_id.clone();
                    out.push(DeviceRecord {
                        info: DeviceInfo {
                            id: device_id.clone(),
                            name: format!("{} ({})", name, platform),
                            device_id,
                            status: DeviceStatus::Online,
                            version: String::new(),
                        },
                        connection: ClawConnection {
                            claw_type: ClawType::OpenClaw,
                            protocol: ClawProtocol::OpenClawJsonRpc,
                            gateway_url: "ws://127.0.0.1:18679".into(),
                            gateway_token: token.clone(),
                            workspace_root: workspace.clone(),
                            host: pd.device_id,
                            auth_mode: None,
                            device_token: None,
                        },
                    });
                }
            }
        }
    }
}

fn discover_remote_openclaw(out: &mut Vec<DeviceRecord>) {
    let Ok(remote_url) = std::env::var("OPENCLAW_REMOTE_URL") else {
        return;
    };
    if remote_url.is_empty() {
        return;
    }
    let remote_token = std::env::var("OPENCLAW_REMOTE_TOKEN").unwrap_or_default();

    let host = gateway_host(&remote_url).unwrap_or_else(|| "openclaw".into());
    let display_name = std::env::var("OPENCLAW_REMOTE_NAME")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| host.clone());

    out.push(DeviceRecord {
        info: DeviceInfo {
            id: "openclaw-remote-env".into(),
            name: format!("{} (OpenClaw)", display_name),
            device_id: remote_url.clone(),
            status: if remote_token.is_empty() {
                DeviceStatus::Offline
            } else {
                DeviceStatus::Online
            },
            version: String::new(),
        },
        connection: ClawConnection {
            claw_type: ClawType::OpenClaw,
            protocol: ClawProtocol::OpenClawJsonRpc,
            gateway_url: remote_url,
            gateway_token: remote_token,
            workspace_root: PathBuf::from("."),
            host,
            auth_mode: None,
            device_token: None,
        },
    });
}

fn gateway_host(url: &str) -> Option<String> {
    url.trim_start_matches("wss://")
        .trim_start_matches("ws://")
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .split(':')
        .next()
        .map(String::from)
}

/// Resolve the OpenClaw home directory.
///
/// Honors `OPENCLAW_HOME`, otherwise defaults to `~/.kimi_openclaw`.
pub fn resolve_openclaw_home() -> PathBuf {
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

fn read_openclaw_config(oc_home: &Path) -> OpenClawConfig {
    let config_path = oc_home.join("openclaw.json");
    if let Ok(raw) = std::fs::read_to_string(&config_path) {
        serde_json::from_str(&raw).unwrap_or_default()
    } else {
        OpenClawConfig::default()
    }
}
