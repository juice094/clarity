//! Manual token+device handshake to a remote OpenClaw Gateway, then sessions.send.
//!
//! Flow:
//!   1. connect (token-only, no device)
//!   2. receive connect.challenge
//!   3. sign v2 payload and re-connect with device block
//!   4. send sessions.send
//!
//! Usage:
//!   OPENCLAW_REMOTE_URL=ws://host:18789 \
//!   OPENCLAW_REMOTE_TOKEN=<token> \
//!     cargo run -p clarity-openclaw --example remote_raw2

#![allow(clippy::unwrap_used, clippy::expect_used)]

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::{Message, http::Request};

#[tokio::main]
async fn main() {
    let gateway_url = std::env::var("OPENCLAW_REMOTE_URL")
        .expect("set OPENCLAW_REMOTE_URL env var, e.g. ws://host:18789");
    let token = std::env::var("OPENCLAW_REMOTE_TOKEN").expect("set OPENCLAW_REMOTE_TOKEN env var");
    let session_key = "agent:main:main";

    let device = clarity_openclaw::DeviceIdentity::load_or_generate("clarity-remote-example")
        .expect("load or generate device identity");
    println!("Device ID:  {}", device.device_id());
    println!("Public Key: {}", device.public_key());

    let host = gateway_url
        .trim_start_matches("wss://")
        .trim_start_matches("ws://")
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .to_string();

    let request = Request::builder()
        .method("GET")
        .uri(&gateway_url)
        .header("Host", &host)
        .header("Connection", "Upgrade")
        .header("Upgrade", "websocket")
        .header("Sec-WebSocket-Version", "13")
        .header("Sec-WebSocket-Key", "dGhlIHNhbXBsZSBub25jZQ==")
        .body(())
        .unwrap();

    let (mut ws_stream, response) = tokio_tungstenite::connect_async(request)
        .await
        .expect("WebSocket connect failed");
    println!("HTTP status: {}", response.status());

    let scopes = [
        "operator.admin",
        "operator.read",
        "operator.write",
        "operator.approvals",
        "operator.pairing",
        "operator.talk.secrets",
    ];

    // Step 1: connect without device.
    let connect1 = serde_json::json!({
        "type": "req",
        "id": "1",
        "method": "connect",
        "params": {
            "minProtocol": 3,
            "maxProtocol": 3,
            "client": {
                "id": "gateway-client",
                "version": env!("CARGO_PKG_VERSION"),
                "platform": "windows",
                "mode": "cli"
            },
            "role": "operator",
            "scopes": scopes,
            "auth": { "token": &token }
        }
    });
    println!("\n>>> {}", connect1);
    ws_stream
        .send(Message::Text(connect1.to_string()))
        .await
        .unwrap();

    // Step 2: wait for connect.challenge
    let timeout = tokio::time::Duration::from_secs(10);
    let nonce = loop {
        match tokio::time::timeout(timeout, ws_stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                println!("<<< {}", text);
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if val.get("event").and_then(|v| v.as_str()) == Some("connect.challenge") {
                        if let Some(n) = val
                            .get("payload")
                            .and_then(|p| p.get("nonce"))
                            .and_then(|v| v.as_str())
                        {
                            break n.to_string();
                        }
                    }
                }
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(e))) => {
                eprintln!("WS error: {}", e);
                return;
            }
            Ok(None) => {
                eprintln!("Connection closed before challenge");
                return;
            }
            Err(_) => {
                eprintln!("Challenge timeout");
                return;
            }
        }
    };
    println!("\nChallenge nonce: {}", nonce);

    // Step 3: sign and re-connect with device.
    let signed_at_ms = chrono::Utc::now().timestamp_millis();
    let sig_payload = format!(
        "v2|{}|gateway-client|cli|operator|{}|{}|{}|{}",
        device.device_id(),
        scopes.join(","),
        signed_at_ms,
        token,
        nonce
    );
    let signature = device.sign_payload(&sig_payload);
    println!("Signature payload: {}", sig_payload);
    println!("Signature: {}", signature);

    let connect2 = serde_json::json!({
        "type": "req",
        "id": "2",
        "method": "connect",
        "params": {
            "minProtocol": 3,
            "maxProtocol": 3,
            "client": {
                "id": "gateway-client",
                "version": env!("CARGO_PKG_VERSION"),
                "platform": "windows",
                "mode": "cli"
            },
            "role": "operator",
            "scopes": scopes,
            "auth": { "token": &token },
            "device": {
                "id": device.device_id(),
                "publicKey": device.public_key(),
                "signature": signature,
                "signedAt": signed_at_ms,
                "nonce": &nonce
            }
        }
    });
    println!("\n>>> {}", connect2);
    ws_stream
        .send(Message::Text(connect2.to_string()))
        .await
        .unwrap();

    // Step 4: wait for hello-ok on id 2.
    loop {
        match tokio::time::timeout(timeout, ws_stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                println!("<<< {}", text);
                if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                    if val.get("id").and_then(|v| v.as_str()) == Some("2") {
                        if val.get("ok").and_then(|v| v.as_bool()) == Some(true) {
                            println!("\nAuth ok on id=2");
                            break;
                        } else {
                            eprintln!("Auth failed on id=2");
                            return;
                        }
                    }
                }
            }
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(e))) => {
                eprintln!("WS error: {}", e);
                return;
            }
            Ok(None) => {
                eprintln!("Connection closed during auth");
                return;
            }
            Err(_) => {
                eprintln!("Auth timeout");
                return;
            }
        }
    }

    // Step 5: sessions.send
    let message = format!(
        "Hello from Clarity.\n\n- Gateway: {}\n- sessionKey: {}\n- Sent at: {}",
        gateway_url,
        session_key,
        chrono::Utc::now().to_rfc3339()
    );

    let send = serde_json::json!({
        "type": "req",
        "id": "3",
        "method": "sessions.send",
        "params": {
            "key": session_key,
            "message": message
        }
    });
    println!("\n>>> {}", send);
    ws_stream
        .send(Message::Text(send.to_string()))
        .await
        .unwrap();

    println!("\nWaiting for replies/events...");
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(20);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(tokio::time::Duration::from_secs(5), ws_stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => println!("<<< {}", text),
            Ok(Some(Ok(_))) => {}
            Ok(Some(Err(e))) => {
                eprintln!("WS error: {}", e);
                break;
            }
            Ok(None) => {
                println!("Connection closed");
                break;
            }
            Err(_) => {}
        }
    }

    println!("Done.");
}
