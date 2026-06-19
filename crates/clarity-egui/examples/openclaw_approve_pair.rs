//! Approve a pending OpenClaw device pairing.
//!
//! Reads the first pending request from KimiClaw's pending.json and approves it
//! using a token-only gateway connection. Requires the gateway token to have
//! sufficient privileges (local KimiClaw gateway token usually does).
//!
//! Usage:
//!   cargo run -p clarity-egui --example openclaw_approve_pair -- <gateway_url> <gateway_token>

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::thread;
use std::time::Duration;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 3 {
        eprintln!("Usage: openclaw_approve_pair <gateway_url> <gateway_token>");
        std::process::exit(1);
    }
    let gateway_url = args[1].replace("127.0.0.1", "localhost");
    let gateway_token = &args[2];

    let pending_path = dirs::home_dir()
        .expect("home dir")
        .join(".kimi_openclaw/devices/pending.json");
    let pending_raw = std::fs::read_to_string(&pending_path)
        .unwrap_or_else(|e| panic!("read {}: {}", pending_path.display(), e));
    let pending: serde_json::Value = serde_json::from_str(&pending_raw).expect("parse pending");
    let first = pending
        .as_object()
        .and_then(|m| m.values().next())
        .expect("no pending requests");
    let request_id = first["requestId"].as_str().expect("requestId");
    println!("Approving pairing request: {}", request_id);

    let client = clarity_openclaw::ClawClient::connect(&gateway_url, gateway_token);

    thread::sleep(Duration::from_secs(1));

    // Send approve request.
    client.send_raw_request(
        "approve-1",
        "device.pair.approve",
        serde_json::json!({ "requestId": request_id }),
    );

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        for resp in client.drain() {
            match resp {
                clarity_openclaw::client::ClawResponse::Connected { .. } => {
                    println!("Connected to Gateway.");
                }
                clarity_openclaw::client::ClawResponse::Reply {
                    id,
                    method: _,
                    ok,
                    payload,
                } => {
                    println!("Reply id={} ok={} payload={}", id, ok, payload);
                    if id == "approve-1" {
                        if ok {
                            println!("Pairing request approved.");
                            return;
                        } else {
                            eprintln!("Approval failed: {}", payload);
                            std::process::exit(1);
                        }
                    }
                }
                clarity_openclaw::client::ClawResponse::Error(e) => {
                    eprintln!("Error: {}", e);
                }
                _ => {}
            }
        }
        thread::sleep(Duration::from_millis(200));
    }

    eprintln!("Timed out waiting for approval response.");
    std::process::exit(1);
}
