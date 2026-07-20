//! Discover OpenClaw Gateways and paired devices.

use crate::types::{
    ClawConnection, ClawProtocol, ClawType, DeviceInfo, DeviceRecord, DeviceStatus,
    OpenClawSendMethod,
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
    plugins: Option<PluginsSection>,
}

#[derive(Deserialize, Default)]
struct PluginsSection {
    #[serde(default)]
    entries: Option<serde_json::Map<String, serde_json::Value>>,
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
    discover_clarity_openclaw_gateway(hostname, &mut out);
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

    // Local KimiClaw Gateway uses chat.send; generic OpenClaw uses sessions.send.
    let local_gateway_url = "ws://127.0.0.1:18679";
    let send_method = if kimiclaw_local_gateway_url(oc_config)
        .is_some_and(|u| urls_equal(&u, local_gateway_url))
    {
        OpenClawSendMethod::ChatSend
    } else {
        OpenClawSendMethod::SessionsSend
    };

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
            send_method,
        },
    });

    // Register paired devices from devices/paired.json.
    let devices_path = oc_home.join("devices").join("paired.json");
    if let Ok(raw) = std::fs::read_to_string(&devices_path)
        && let Ok(map) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(&raw)
    {
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
                        send_method,
                    },
                });
            }
        }
    }
}

/// Extract the local Gateway URL configured inside the `kimi-claw` plugin, if any.
///
/// Looks at `plugins.entries.kimi-claw.config.gateway.url`.
fn kimiclaw_local_gateway_url(config: &OpenClawConfig) -> Option<String> {
    let entries = config.plugins.as_ref()?.entries.as_ref()?;
    let kimi_claw = entries.get("kimi-claw")?;
    let config_obj = kimi_claw.get("config")?.as_object()?;
    let gateway = config_obj.get("gateway")?.as_object()?;
    gateway.get("url")?.as_str().map(String::from)
}

/// Compare two WebSocket/HTTP URLs ignoring scheme differences and trailing slashes.
///
/// ponytail: simple string normalization; does not handle auth, paths, or query strings.
fn urls_equal(a: &str, b: &str) -> bool {
    fn normalize(url: &str) -> String {
        url.trim()
            .trim_start_matches("wss://")
            .trim_start_matches("ws://")
            .trim_start_matches("https://")
            .trim_start_matches("http://")
            .trim_end_matches('/')
            .to_ascii_lowercase()
    }
    normalize(a) == normalize(b)
}

/// Discover the OpenClaw-compatible endpoint built into `clarity-gateway`.
///
/// This lets Claw clients fall back to Clarity's own Gateway when Kimi Desktop
/// is not installed. The endpoint is `ws://127.0.0.1:18790/openclaw/ws` and
/// authenticates with the admin token persisted in `.clarity/openclaw-admin-token`.
fn discover_clarity_openclaw_gateway(hostname: &str, out: &mut Vec<DeviceRecord>) {
    let admin_token = std::env::current_dir()
        .map(|cwd| cwd.join(".clarity").join("openclaw-admin-token"))
        .and_then(|path| std::fs::read_to_string(path).map(|s| s.trim().to_string()))
        .unwrap_or_default();

    out.push(DeviceRecord {
        info: DeviceInfo {
            id: "openclaw-clarity-gateway".into(),
            name: format!("{} (Clarity OpenClaw)", hostname),
            device_id: "127.0.0.1:18790".into(),
            status: DeviceStatus::Online,
            version: String::new(),
        },
        connection: ClawConnection {
            claw_type: ClawType::OpenClaw,
            protocol: ClawProtocol::OpenClawJsonRpc,
            gateway_url: "ws://127.0.0.1:18790/openclaw/ws".into(),
            gateway_token: admin_token,
            workspace_root: PathBuf::from("."),
            host: "127.0.0.1".into(),
            auth_mode: None,
            device_token: None,
            send_method: OpenClawSendMethod::ChatSend,
        },
    });
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
            send_method: OpenClawSendMethod::SessionsSend,
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

#[cfg(test)]
mod tests {
    use super::*;

    fn write_config(dir: &tempfile::TempDir, contents: &str) {
        std::fs::write(dir.path().join("openclaw.json"), contents).unwrap();
    }

    #[test]
    fn urls_equal_ignores_scheme_and_trailing_slash() {
        assert!(urls_equal("ws://127.0.0.1:18679", "ws://127.0.0.1:18679"));
        assert!(urls_equal("ws://127.0.0.1:18679/", "ws://127.0.0.1:18679"));
        assert!(urls_equal("http://127.0.0.1:18679", "ws://127.0.0.1:18679"));
        assert!(urls_equal(
            "wss://example.com:443",
            "https://example.com:443"
        ));
        assert!(!urls_equal("ws://127.0.0.1:18679", "ws://127.0.0.1:18789"));
    }

    #[test]
    fn kimiclaw_local_gateway_url_extracts_from_plugins() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            &dir,
            r#"{
                "plugins": {
                    "entries": {
                        "kimi-claw": {
                            "enabled": true,
                            "config": {
                                "gateway": {
                                    "url": "ws://127.0.0.1:18679",
                                    "token": "abc"
                                }
                            }
                        }
                    }
                }
            }"#,
        );

        let config = read_openclaw_config(dir.path());
        assert_eq!(
            kimiclaw_local_gateway_url(&config),
            Some("ws://127.0.0.1:18679".into())
        );
    }

    #[test]
    fn discover_local_openclaw_uses_chat_send_for_kimiclaw() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            &dir,
            r#"{
                "gateway": { "auth": { "token": "local-token" } },
                "plugins": {
                    "entries": {
                        "kimi-claw": {
                            "enabled": true,
                            "config": {
                                "gateway": { "url": "ws://127.0.0.1:18679" }
                            }
                        }
                    }
                }
            }"#,
        );

        let config = read_openclaw_config(dir.path());
        let mut out = Vec::new();
        discover_local_openclaw("test-host", dir.path(), &config, &mut out);

        let gateway = out
            .iter()
            .find(|r| r.info.id == "openclaw-local-gateway")
            .expect("local gateway record missing");
        assert_eq!(gateway.connection.send_method, OpenClawSendMethod::ChatSend);
    }

    #[test]
    fn discover_local_openclaw_defaults_to_sessions_send() {
        let dir = tempfile::tempdir().unwrap();
        write_config(
            &dir,
            r#"{
                "gateway": { "auth": { "token": "local-token" } }
            }"#,
        );

        let config = read_openclaw_config(dir.path());
        let mut out = Vec::new();
        discover_local_openclaw("test-host", dir.path(), &config, &mut out);

        let gateway = out
            .iter()
            .find(|r| r.info.id == "openclaw-local-gateway")
            .expect("local gateway record missing");
        assert_eq!(
            gateway.connection.send_method,
            OpenClawSendMethod::SessionsSend
        );
    }

    #[test]
    fn discover_clarity_openclaw_gateway_registers_fallback_device() {
        let dir = tempfile::tempdir().unwrap();
        let clarity_dir = dir.path().join(".clarity");
        std::fs::create_dir_all(&clarity_dir).unwrap();
        std::fs::write(clarity_dir.join("openclaw-admin-token"), "oc-admin-token").unwrap();

        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let mut out = Vec::new();
        discover_clarity_openclaw_gateway("test-host", &mut out);
        std::env::set_current_dir(prev).unwrap();

        let record = out
            .iter()
            .find(|r| r.info.id == "openclaw-clarity-gateway")
            .expect("Clarity OpenClaw gateway record missing");
        assert_eq!(
            record.connection.gateway_url,
            "ws://127.0.0.1:18790/openclaw/ws"
        );
        assert_eq!(record.connection.gateway_token, "oc-admin-token");
        assert_eq!(record.connection.send_method, OpenClawSendMethod::ChatSend);
    }

    #[test]
    fn discover_clarity_openclaw_gateway_allows_missing_token() {
        let dir = tempfile::tempdir().unwrap();
        let prev = std::env::current_dir().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        let mut out = Vec::new();
        discover_clarity_openclaw_gateway("test-host", &mut out);
        std::env::set_current_dir(prev).unwrap();

        let record = out
            .iter()
            .find(|r| r.info.id == "openclaw-clarity-gateway")
            .expect("Clarity OpenClaw gateway record missing");
        assert_eq!(record.connection.gateway_token, "");
    }
}
