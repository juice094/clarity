//! Send a one-off message to a remote OpenClaw Gateway using device attestation.
//!
//! The Gateway must already accept the provided token and the device must be
//! permitted to sign in. For a pairing flow, see `remote_pair.rs`.
//!
//! Usage:
//!   OPENCLAW_REMOTE_URL=ws://host:18789 \
//!   OPENCLAW_REMOTE_TOKEN=<token> \
//!     cargo run -p clarity-openclaw --example remote_send

use clarity_openclaw::DeviceIdentity;
use clarity_openclaw::client::{ClawClient, ClawResponse};
use std::time::Duration;

fn main() {
    let gateway_url = std::env::var("OPENCLAW_REMOTE_URL")
        .expect("set OPENCLAW_REMOTE_URL env var, e.g. ws://host:18789");
    let token = std::env::var("OPENCLAW_REMOTE_TOKEN").expect("set OPENCLAW_REMOTE_TOKEN env var");
    let session_key = "agent:main:main";

    let device = DeviceIdentity::load_or_generate("clarity-remote-example")
        .expect("load or generate device identity");
    println!("Device ID:    {}", device.device_id());
    println!("Public Key:   {}", device.public_key());
    println!();

    println!("Connecting with token + device attestation...");
    let client = ClawClient::connect_with_remote_device(&gateway_url, &token, device);

    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut connected = false;
    while std::time::Instant::now() < deadline {
        for resp in client.drain() {
            println!("[auth handshake] {:?}", resp);
            if matches!(resp, ClawResponse::Connected { .. }) {
                connected = true;
                break;
            }
        }
        if connected {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    if !connected {
        eprintln!("Device-attested connect failed.");
        std::process::exit(1);
    }

    let message = format!(
        "Hello from Clarity.\n\n- Gateway: {}\n- sessionKey: {}\n- Sent at: {}",
        gateway_url,
        session_key,
        chrono::Utc::now().to_rfc3339()
    );

    client.send_raw_request(
        "send-1",
        "sessions.send",
        serde_json::json!({
            "key": session_key,
            "message": message,
        }),
    );

    println!("Sent sessions.send, waiting for replies/events...");

    let deadline = std::time::Instant::now() + Duration::from_secs(20);
    while std::time::Instant::now() < deadline {
        for resp in client.drain() {
            println!("[recv] {:?}", resp);
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    println!("Done.");
}
