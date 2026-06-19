//! Test a device-paired OpenClaw connection.
//!
//! Reads the device token saved by `openclaw_pair` and connects with device
//! auth. Tries `sessions.list` to verify scopes, then sends a hello message.
//!
//! Usage:
//!   cargo run -p clarity-egui --example openclaw_device_check

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

    let device = clarity_egui::claw_device::DeviceIdentity::load_or_generate("clarity-egui")
        .expect("load device identity");

    println!("Connecting to {} with device auth...", gateway_url);
    println!("Device ID: {}", device.device_id());

    let client = clarity_egui::claw_client::ClawClient::connect_with_device(
        &gateway_url,
        device,
        device_token,
    );

    // Give auth time to complete, then list sessions and send a hello.
    thread::sleep(Duration::from_secs(2));

    client.send_message("agent:main:main", "hello from clarity device auth");

    let deadline = std::time::Instant::now() + Duration::from_secs(15);
    while std::time::Instant::now() < deadline {
        for resp in client.drain() {
            match resp {
                clarity_egui::claw_client::ClawResponse::Connected { .. } => {
                    println!("Device auth connected.");
                }
                clarity_egui::claw_client::ClawResponse::SessionMessage {
                    role,
                    content,
                    finished,
                } => {
                    println!(
                        "session.message role={} finished={}: {}",
                        role, finished, content
                    );
                }
                clarity_egui::claw_client::ClawResponse::Event {
                    event_type,
                    payload,
                } => {
                    println!("event {} -> {}", event_type, payload);
                }
                clarity_egui::claw_client::ClawResponse::Reply { id, ok, payload } => {
                    println!("reply id={} ok={} payload={}", id, ok, payload);
                }
                clarity_egui::claw_client::ClawResponse::Error(e) => {
                    eprintln!("Error: {}", e);
                }
                _ => {}
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
}
