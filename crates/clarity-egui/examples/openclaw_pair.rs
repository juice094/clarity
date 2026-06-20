//! Helper to pair Clarity with a local OpenClaw/KimiClaw Gateway.
//!
//! Usage:
//!   cargo run -p clarity-egui --example openclaw_pair -- ws://127.0.0.1:18679 <gateway_token>
//!
//! The example loads (or generates) a Clarity device identity, connects to the
//! Gateway with the plain gateway token, sends `device.pair.request`, and waits
//! for the user to approve the pairing in the KimiClaw UI. Once approved, it
//! prints the device token and saves it to `~/.clarity/claw-device-token.json`.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: openclaw_pair <gateway_url> <gateway_token>");
        std::process::exit(1);
    }
    let gateway_url = &args[1];
    let gateway_token = &args[2];

    let device = clarity_openclaw::DeviceIdentity::load_or_generate()
        .expect("failed to load/generate device identity");

    println!("Device ID:    {}", device.device_id());
    println!("Public Key:   {}", device.public_key());
    println!();
    println!("Connecting to {} to request pairing...", gateway_url);

    let client = clarity_openclaw::ClawClient::connect(gateway_url, gateway_token);

    let scopes = vec![
        "operator.admin".into(),
        "operator.read".into(),
        "operator.write".into(),
        "operator.approvals".into(),
        "operator.pairing".into(),
        "operator.talk.secrets".into(),
    ];

    client.request_pairing(
        &device.device_id(),
        &device.public_key(),
        "openclaw-control-ui",
        "webchat",
        "windows",
        "operator",
        &scopes,
    );

    println!("Pairing request sent. Approve it in the KimiClaw UI now.");
    println!("Waiting up to 120 seconds for approval...");

    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    while std::time::Instant::now() < deadline {
        for resp in client.drain() {
            match resp {
                clarity_openclaw::client::ClawResponse::Connected { .. } => {
                    println!("Connected to Gateway.");
                }
                clarity_openclaw::client::ClawResponse::PairingResult {
                    device_id,
                    approved,
                    token,
                    scopes,
                } => {
                    if approved {
                        if let Some(token) = token {
                            println!("Pairing approved!");
                            println!("Device ID:    {}", device_id);
                            println!("Device Token: {}", token);
                            println!("Scopes:       {:?}", scopes);
                            save_device_token(&device_id, &token, gateway_url);
                            return;
                        }
                    } else {
                        println!("Pairing pending for device {}. Still waiting...", device_id);
                    }
                }
                clarity_openclaw::client::ClawResponse::Event {
                    event_type,
                    payload,
                } => {
                    println!("Gateway event: {} -> {}", event_type, payload);
                }
                clarity_openclaw::client::ClawResponse::Reply {
                    id,
                    method: _,
                    ok,
                    payload,
                } => {
                    println!("Gateway reply: id={} ok={} payload={}", id, ok, payload);
                }
                clarity_openclaw::client::ClawResponse::Error(e) => {
                    eprintln!("Error: {}", e);
                }
                _ => {}
            }
        }
        thread::sleep(Duration::from_millis(500));
    }

    eprintln!("Timed out waiting for pairing approval.");
    std::process::exit(1);
}

fn save_device_token(device_id: &str, token: &str, gateway_url: &str) {
    let data_dir = dirs::data_dir().expect("cannot determine data directory");
    let dir = data_dir.join("clarity");
    std::fs::create_dir_all(&dir).expect("create clarity data dir");
    let path = dir.join("claw-device-token.json");
    let value = serde_json::json!({
        "device_id": device_id,
        "token": token,
        "gateway_url": gateway_url
    });
    std::fs::write(&path, serde_json::to_string_pretty(&value).unwrap())
        .expect("write device token");
    println!("Saved device token to: {}", path.display());
}
