//! Interactive bidirectional chat with a remote OpenClaw Gateway.
//!
//! This example uses `TokenWithDevice` authentication:
//!   1. Open WebSocket.
//!   2. Wait for `connect.challenge`.
//!   3. Sign the nonce with the local Ed25519 device key.
//!   4. Send `connect` with `device` + `auth.token` + scopes.
//!   5. Subscribe to `agent:main:main` and echo stdin input into it.
//!
//! Usage:
//!   OPENCLAW_REMOTE_URL=ws://host:18789 \
//!   OPENCLAW_REMOTE_TOKEN=<token> \
//!     cargo run -p clarity-openclaw --example remote_chat

use clarity_openclaw::client::{ClawClient, ClawResponse};
use std::io::Write;
use std::time::Duration;

fn main() {
    let gateway_url = std::env::var("OPENCLAW_REMOTE_URL")
        .expect("set OPENCLAW_REMOTE_URL env var, e.g. ws://host:18789");
    let token = std::env::var("OPENCLAW_REMOTE_TOKEN").expect("set OPENCLAW_REMOTE_TOKEN env var");
    let session_key = "agent:main:main";

    let device = clarity_openclaw::DeviceIdentity::load_or_generate("clarity-remote-example")
        .expect("load or generate device identity");
    println!("Device ID: {}", device.device_id());

    let client = ClawClient::connect_with_remote_device(&gateway_url, &token, device);

    // Wait for connection + hello-ok.
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    let mut connected = false;
    while std::time::Instant::now() < deadline {
        for resp in client.drain() {
            match resp {
                ClawResponse::Connected { gateway_url } => {
                    println!("Connected to {}", gateway_url);
                    connected = true;
                }
                ClawResponse::Error(e) => {
                    eprintln!("Connection error: {}", e);
                    std::process::exit(1);
                }
                other => println!("[handshake] {:?}", other),
            }
        }
        if connected {
            break;
        }
        std::thread::sleep(Duration::from_millis(200));
    }

    if !connected {
        eprintln!("Failed to connect/authenticate.");
        std::process::exit(1);
    }

    client.subscribe_session(session_key);
    client.subscribe_messages(session_key);
    println!(
        "Subscribed to {}. Type messages and press Enter.",
        session_key
    );

    // Spawn a background thread to drain and print events.
    let drain_client = client.clone();
    std::thread::spawn(move || {
        loop {
            for resp in drain_client.drain() {
                match resp {
                    ClawResponse::SessionMessage {
                        role,
                        content,
                        finished,
                    } => {
                        print!("{}: {}", role, content);
                        if finished {
                            println!();
                        }
                    }
                    ClawResponse::Event {
                        event_type,
                        payload,
                    } => {
                        println!("[event: {}] {}", event_type, payload);
                    }
                    ClawResponse::Error(e) => {
                        eprintln!("[error] {}", e);
                    }
                    other => println!("[resp] {:?}", other),
                }
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    });

    // Read stdin and send.
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout();
    loop {
        print!("> ");
        stdout.flush().unwrap();
        let mut line = String::new();
        if stdin.read_line(&mut line).unwrap() == 0 {
            break;
        }
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if line == "/quit" {
            break;
        }
        client.send_session_message(session_key, line);
    }

    println!("Bye.");
}
