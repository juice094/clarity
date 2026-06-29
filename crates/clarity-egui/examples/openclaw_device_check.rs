//! Test a device-paired OpenClaw connection.
//!
//! Reads the device token saved by `openclaw_pair` and connects with device
//! auth. Tries `sessions.list` to verify scopes, then sends a hello message.
//!
//! Usage:
//!   cargo run -p clarity-egui --example openclaw_device_check

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::thread;
use std::time::Duration;

fn main() {
    let data_dir = dirs::data_dir().expect("cannot determine data directory");
    let token_path = data_dir.join("clarity/claw-device-token.json");
    let raw = std::fs::read_to_string(&token_path)
        .unwrap_or_else(|e| panic!("read {}: {}", token_path.display(), e));
    let cfg: serde_json::Value = serde_json::from_str(&raw).expect("parse token file");
    let gateway_url = cfg["gateway_url"]
        .as_str()
        .expect("gateway_url")
        .replace("127.0.0.1", "localhost");
    let device_token = cfg["token"].as_str().expect("token");

    let device = clarity_claw::DeviceIdentity::load_or_generate().expect("load device identity");

    println!("Connecting to {} with device auth...", gateway_url);
    println!("Device ID: {}", device.device_id());

    let client = clarity_claw::ClawClient::connect_with_device(&gateway_url, device, device_token);

    // Give auth time to complete, then probe sessions and send a hello.
    thread::sleep(Duration::from_secs(2));

    client.send_raw_request("sessions-list-1", "sessions.list", serde_json::json!({}));
    client.send_raw_request(
        "chat-history-1",
        "chat.history",
        serde_json::json!({"sessionKey": "agent:main:main", "limit": 5}),
    );
    client.send_message("agent:main:main", "hello from clarity device auth");
    client.send_raw_request(
        "chat-send-1",
        "chat.send",
        serde_json::json!({
            "sessionKey": "agent:main:main",
            "message": "hello from clarity device auth via chat.send",
            "deliver": false,
            "idempotencyKey": "clarity-test-1"
        }),
    );

    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while std::time::Instant::now() < deadline {
        for resp in client.drain() {
            match resp {
                clarity_claw::client::ClawResponse::Connected { .. } => {
                    println!("Device auth connected.");
                }
                clarity_claw::client::ClawResponse::SessionMessage {
                    role,
                    content,
                    finished,
                } => {
                    println!(
                        "session.message role={} finished={}: {}",
                        role, finished, content
                    );
                }
                clarity_claw::client::ClawResponse::Event {
                    event_type,
                    payload,
                } => {
                    println!("event {} -> {}", event_type, payload);
                }
                clarity_claw::client::ClawResponse::Reply {
                    id,
                    method: _,
                    ok,
                    payload,
                } => {
                    println!("reply id={} ok={} payload={}", id, ok, payload);
                }
                clarity_claw::client::ClawResponse::Error(e) => {
                    eprintln!("Error: {}", e);
                }
                _ => {}
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
}
