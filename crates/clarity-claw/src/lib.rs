//! clarity-claw —— Clarity 分布式节点客户端库与系统托盘常驻程序
//!
//! 本 crate 是 Claw 概念的 client-side 统一入口：
//! - **库**：UI 无关的 Claw 客户端，支持 Gateway WebSocket、设备发现、配对、
//!   角色上下文同步，以及可选的 OpenClaw/KimiClaw JSON-RPC 兼容层。
//! - **二进制**：系统托盘常驻节点，与本地 `clarity-gateway` 通信。
//!
//! Server-side 对应物是 `clarity-gateway`；跨 crate 共享契约见
//! `clarity-contract::claw_context` 与 `clarity-contract::federation`。
//!
//! "Claw" 名字来自早期对外部 ZeroClaw / OpenClaw / KimiClaw 的参照，
//! 在 Clarity 内部已重新定义为分布式协作节点概念。

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        missing_docs,
        unsafe_code
    )
)]

pub mod acp_bridge;
pub mod client;
pub mod connection_manager;
pub mod device;
pub mod discovery;
pub mod gateway_client;
#[cfg(feature = "mesh")]
pub mod mesh;
#[cfg(feature = "mesh")]
pub mod mesh_client;
pub mod netmon;
pub mod openclaw_gateway;
pub mod protocol;
pub mod transports;
pub mod types;
pub mod watchdog;

#[cfg(feature = "tray")]
pub mod tray;

mod util;

pub use client::ClawClient as OpenClawClient;
pub use client::{ClawAuth, ClawClient};
pub use connection_manager::ClawConnectionManager;
pub use device::{DeviceIdentity, PairedToken, load_paired_token, save_paired_token};
pub use discovery::discover_openclaw_devices;
pub use gateway_client::{GatewayClient, GatewayMessage, GatewayResponse, ToolCall};
pub use netmon::{NetChangeEvent, NetMonitor};
pub use protocol::{DetectedProtocol, ProtocolCommand, ProtocolEvent, ProtocolHistoryMessage};
pub use transports::{GatewayWebSocketTransport, OpenClawTransport, TransportManager};
pub use types::{ClawConnection, ClawProtocol, ClawType, DeviceInfo, DeviceRecord, DeviceStatus};
pub use watchdog::GatewayWatchdog;

use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;

// ------------------------------------------------------------------
// Gateway interaction helpers (shared by library consumers and tray)
// ------------------------------------------------------------------

/// Default Gateway address.
pub const GATEWAY_URL: &str = "http://127.0.0.1:18790";

/// Gateway polling interval in seconds.
pub const POLL_INTERVAL_SECS: u64 = 5;

/// Minimal task info returned by Gateway `list_tasks`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct TaskSummary {
    #[serde(rename = "task_id")]
    /// Unique task identifier.
    pub task_id: String,
    /// Human-readable task name.
    pub name: String,
    /// Current task status (e.g., "Running", "Completed").
    pub status: String,
}

/// Minimal thread info returned by Gateway `list_threads`.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct ThreadSummary {
    /// Thread identifier.
    pub thread_id: String,
    /// Human-readable title.
    pub title: Option<String>,
    /// Last update timestamp.
    #[serde(default)]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

/// Resolve the Gateway URL from the environment.
pub fn resolve_gateway_url() -> String {
    std::env::var("CLARITY_GATEWAY_URL").unwrap_or_else(|_| GATEWAY_URL.to_string())
}

/// Send a single WebSocket request to the Gateway and return the first
/// non-welcome response.
///
/// ponytail: opens a fresh connection per request. If polling volume becomes
/// high, reuse a long-lived WebSocket instead.
async fn gateway_ws_request(
    gateway_url: &str,
    request: serde_json::Value,
) -> anyhow::Result<serde_json::Value> {
    let ws_url = gateway_ws_url(gateway_url);
    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url).await?;
    let (mut write, mut read) = ws_stream.split();

    // The Gateway sends a welcome frame immediately after the handshake.
    let welcome = read
        .next()
        .await
        .ok_or_else(|| anyhow::anyhow!("connection closed before welcome"))??;
    let welcome_text = welcome.to_text()?;
    let welcome: serde_json::Value = serde_json::from_str(welcome_text)?;
    if welcome.get("type").and_then(|v| v.as_str()) != Some("welcome") {
        return Err(anyhow::anyhow!("expected welcome, got {}", welcome_text));
    }

    write
        .send(tokio_tungstenite::tungstenite::Message::Text(
            request.to_string(),
        ))
        .await
        .map_err(|e| anyhow::anyhow!("send request: {}", e))?;

    while let Some(msg) = read.next().await {
        let msg = msg?;
        if let tokio_tungstenite::tungstenite::Message::Text(text) = msg {
            let value: serde_json::Value = serde_json::from_str(&text)?;
            if value.get("type").and_then(|v| v.as_str()) == Some("welcome") {
                continue;
            }
            return Ok(value);
        }
    }

    Err(anyhow::anyhow!("connection closed before response"))
}

/// Convert a Gateway error response into an `anyhow::Error`.
fn check_gateway_error(value: &serde_json::Value) -> anyhow::Result<()> {
    if value.get("type").and_then(|v| v.as_str()) == Some("error") {
        let msg = value
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown error");
        return Err(anyhow::anyhow!("Gateway error: {}", msg));
    }
    Ok(())
}

/// Convert an HTTP(S) Gateway URL into the canonical WebSocket endpoint.
pub fn gateway_ws_url(url: &str) -> String {
    let mut url = url.to_string();
    if url.starts_with("http://") {
        url = url.replacen("http://", "ws://", 1);
    } else if url.starts_with("https://") {
        url = url.replacen("https://", "wss://", 1);
    }
    format!("{}/ws", url.trim_end_matches('/'))
}

/// Convert an HTTP(S) or WebSocket URL into the OpenClaw JSON-RPC endpoint.
///
/// Kimi Desktop's OpenClaw Gateway speaks on the root WebSocket path
/// (`ws://127.0.0.1:18679`), while `clarity-gateway` exposes its compatible
/// endpoint under `/openclaw/ws` (`ws://127.0.0.1:18790/openclaw/ws`). This
/// helper normalizes a user-supplied Gateway URL so that:
///
/// - URLs already ending in `/openclaw/ws` are preserved.
/// - URLs pointing at the default Clarity Gateway port (`18790`) are mounted
///   under `/openclaw/ws`.
/// - Other URLs (e.g. Kimi Desktop on `18679`) are kept on their original path
///   after scheme conversion.
pub fn openclaw_ws_url(url: &str) -> String {
    let mut url = url.to_string();
    if url.starts_with("http://") {
        url = url.replacen("http://", "ws://", 1);
    } else if url.starts_with("https://") {
        url = url.replacen("https://", "wss://", 1);
    }
    if url.ends_with("/openclaw/ws") {
        return url;
    }
    let trimmed = url.trim_end_matches('/');
    if trimmed.contains(":18790") && !trimmed.contains("/openclaw/ws") {
        format!("{}/openclaw/ws", trimmed)
    } else {
        trimmed.to_string()
    }
}

/// Send a quick chat message to the Gateway over WebSocket (non-streaming).
///
/// Claw speaks Gateway WebSocket only; this function intentionally does not
/// fall back to the HTTP `/v1/chat/completions` endpoint.
pub async fn quick_chat(gateway_url: &str, input: &str) -> anyhow::Result<String> {
    let request = serde_json::json!({
        "type": "chat",
        "message": input,
        "use_wire": false,
    });
    let value = gateway_ws_request(gateway_url, request).await?;
    check_gateway_error(&value)?;
    let reply = value["message"].as_str().unwrap_or("(no response)");
    Ok(reply.to_string())
}

/// Poll the Gateway task list over WebSocket.
pub async fn poll_tasks(gateway_url: &str) -> anyhow::Result<Vec<TaskSummary>> {
    let request = serde_json::json!({ "type": "list_tasks" });
    let value = gateway_ws_request(gateway_url, request).await?;
    check_gateway_error(&value)?;
    let tasks = value
        .get("tasks")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| serde_json::from_value::<TaskSummary>(v.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    Ok(tasks)
}

/// Create a background task via Gateway WebSocket.
pub async fn create_remote_task(
    gateway_url: &str,
    name: &str,
    prompt: &str,
) -> anyhow::Result<String> {
    let request = serde_json::json!({
        "type": "create_task",
        "name": name,
        "prompt": prompt,
        "max_iterations": 10,
    });
    let value = gateway_ws_request(gateway_url, request).await?;
    check_gateway_error(&value)?;
    let task_id = value["task_id"].as_str().unwrap_or("unknown").to_string();
    Ok(task_id)
}

/// Poll the Gateway thread list over WebSocket.
pub async fn poll_threads(gateway_url: &str) -> anyhow::Result<Vec<ThreadSummary>> {
    let request = serde_json::json!({
        "type": "list_threads",
        "limit": 10,
    });
    let value = gateway_ws_request(gateway_url, request).await?;
    check_gateway_error(&value)?;
    let threads = value
        .get("data")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| serde_json::from_value::<ThreadSummary>(v.clone()).ok())
                .collect()
        })
        .unwrap_or_default();
    Ok(threads)
}

/// Create a new thread via Gateway WebSocket.
pub async fn create_remote_thread(
    gateway_url: &str,
    title: Option<&str>,
) -> anyhow::Result<String> {
    let request = serde_json::json!({
        "type": "create_thread",
        "title": title,
    });
    let value = gateway_ws_request(gateway_url, request).await?;
    check_gateway_error(&value)?;
    let thread_id = value["thread_id"].as_str().unwrap_or("unknown").to_string();
    Ok(thread_id)
}

/// Register this Claw instance as a device with the Gateway.
/// Returns the device id on first registration.
pub async fn register_device(gateway_url: &str) -> anyhow::Result<String> {
    let hostname = get_hostname();
    let device_id = format!("claw-{}", hostname);

    let request = serde_json::json!({
        "type": "register_device",
        "id": device_id,
        "name": hostname,
        "host": hostname,
        "version": env!("CARGO_PKG_VERSION"),
    });
    let value = gateway_ws_request(gateway_url, request).await?;
    check_gateway_error(&value)?;

    tracing::info!(
        device_id = %device_id,
        gateway_url = %gateway_url,
        "Claw device registered with Gateway"
    );
    Ok(device_id)
}

/// Send a heartbeat to keep the device alive in the Gateway registry.
pub async fn send_heartbeat(gateway_url: &str, device_id: &str) -> anyhow::Result<()> {
    let request = serde_json::json!({
        "type": "heartbeat",
        "id": device_id,
        "name": get_hostname(),
        "host": get_hostname(),
        "version": env!("CARGO_PKG_VERSION"),
    });
    let value = gateway_ws_request(gateway_url, request).await?;
    check_gateway_error(&value)?;

    tracing::debug!(device_id = %device_id, "Claw heartbeat sent");
    Ok(())
}

/// Best-effort hostname resolution. Falls back to "unknown" if the OS
/// doesn't expose a hostname.
fn get_hostname() -> String {
    // SAFETY: these env-var reads are infallible — None maps to the default.
    if cfg!(target_os = "windows") {
        std::env::var("COMPUTERNAME").unwrap_or_else(|_| "unknown".into())
    } else {
        std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown".into())
    }
}

// ------------------------------------------------------------------
// Tray-only helpers
// ------------------------------------------------------------------

#[cfg(feature = "tray")]
/// Format the tray tooltip from task and thread counts.
pub fn format_tooltip(
    running: usize,
    pending: usize,
    total_tasks: usize,
    recent_threads: usize,
) -> String {
    let task_part = if total_tasks == 0 {
        "no tasks".to_string()
    } else {
        format!(
            "{} running, {} pending ({} tasks)",
            running, pending, total_tasks
        )
    };
    let thread_part = if recent_threads == 0 {
        "no recent threads".to_string()
    } else {
        format!("{} recent threads", recent_threads)
    };
    format!("Clarity Claw — {} | {}", task_part, thread_part)
}

#[cfg(feature = "tray")]
/// Classify a task status into a notification summary.
pub fn classify_task_status(status: &str) -> (&'static str, Option<notify_rust::Urgency>) {
    match status {
        "Completed" => ("✅ Task completed", None),
        "Failed" => ("❌ Task failed", Some(notify_rust::Urgency::Critical)),
        "Cancelled" => ("🚫 Task cancelled", None),
        _ => ("Task finished", None),
    }
}

// ------------------------------------------------------------------
// Tests
// ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn set_env(key: &str, value: &str) {
        // SAFETY: test-only helper; env vars are manipulated in single-threaded test context.
        unsafe { std::env::set_var(key, value) };
    }

    fn remove_env(key: &str) {
        // SAFETY: test-only helper; env vars are manipulated in single-threaded test context.
        unsafe { std::env::remove_var(key) };
    }

    #[test]
    fn test_resolve_gateway_url() {
        // Default (no env var)
        remove_env("CLARITY_GATEWAY_URL");
        assert_eq!(resolve_gateway_url(), GATEWAY_URL);

        // From env var
        set_env("CLARITY_GATEWAY_URL", "http://custom:8080");
        assert_eq!(resolve_gateway_url(), "http://custom:8080");
        remove_env("CLARITY_GATEWAY_URL");
    }

    #[test]
    fn test_task_summary_deserialization() {
        let json = r#"{"task_id":"abc-123","name":"test task","status":"Running"}"#;
        let summary: TaskSummary = serde_json::from_str(json).unwrap();
        assert_eq!(summary.task_id, "abc-123");
        assert_eq!(summary.name, "test task");
        assert_eq!(summary.status, "Running");
    }

    #[test]
    fn test_thread_summary_deserialization() {
        let json =
            r#"{"thread_id":"th-abc","title":"My thread","updated_at":"2026-06-15T02:00:00Z"}"#;
        let summary: ThreadSummary = serde_json::from_str(json).unwrap();
        assert_eq!(summary.thread_id, "th-abc");
        assert_eq!(summary.title, Some("My thread".to_string()));
        assert!(summary.updated_at.is_some());
    }

    #[test]
    fn test_gateway_ws_url_converts_http_and_appends_ws() {
        assert_eq!(
            gateway_ws_url("http://127.0.0.1:18790"),
            "ws://127.0.0.1:18790/ws"
        );
        assert_eq!(
            gateway_ws_url("https://example.com:18790/"),
            "wss://example.com:18790/ws"
        );
        assert_eq!(
            gateway_ws_url("ws://127.0.0.1:18790"),
            "ws://127.0.0.1:18790/ws"
        );
    }

    #[test]
    fn test_openclaw_ws_url_mounts_clarity_gateway_under_openclaw_path() {
        // HTTP/WS scheme conversion and /openclaw/ws mount for Clarity Gateway.
        assert_eq!(
            openclaw_ws_url("http://127.0.0.1:18790"),
            "ws://127.0.0.1:18790/openclaw/ws"
        );
        assert_eq!(
            openclaw_ws_url("ws://127.0.0.1:18790/"),
            "ws://127.0.0.1:18790/openclaw/ws"
        );
        // Already-complete URL is preserved.
        assert_eq!(
            openclaw_ws_url("ws://127.0.0.1:18790/openclaw/ws"),
            "ws://127.0.0.1:18790/openclaw/ws"
        );
    }

    #[test]
    fn test_openclaw_ws_url_preserves_kimi_desktop_root_path() {
        // Kimi Desktop OpenClaw Gateway speaks on the root WebSocket path.
        assert_eq!(
            openclaw_ws_url("http://127.0.0.1:18679"),
            "ws://127.0.0.1:18679"
        );
        assert_eq!(
            openclaw_ws_url("ws://127.0.0.1:18679/"),
            "ws://127.0.0.1:18679"
        );
    }

    #[cfg(feature = "tray")]
    mod tray_helpers {
        use super::*;

        #[test]
        fn test_format_tooltip_idle() {
            assert_eq!(
                format_tooltip(0, 0, 0, 0),
                "Clarity Claw — no tasks | no recent threads"
            );
        }

        #[test]
        fn test_format_tooltip_with_tasks() {
            assert_eq!(
                format_tooltip(2, 1, 3, 0),
                "Clarity Claw — 2 running, 1 pending (3 tasks) | no recent threads"
            );
        }

        #[test]
        fn test_format_tooltip_with_threads() {
            assert_eq!(
                format_tooltip(0, 0, 0, 5),
                "Clarity Claw — no tasks | 5 recent threads"
            );
        }

        #[test]
        fn test_classify_task_status() {
            assert_eq!(
                classify_task_status("Completed"),
                ("✅ Task completed", None)
            );
            assert_eq!(
                classify_task_status("Failed"),
                ("❌ Task failed", Some(notify_rust::Urgency::Critical))
            );
            assert_eq!(
                classify_task_status("Cancelled"),
                ("🚫 Task cancelled", None)
            );
            assert_eq!(classify_task_status("Unknown"), ("Task finished", None));
        }
    }
}
