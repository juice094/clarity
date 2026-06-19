//! Raw WebSocket probe + message sender to a remote OpenClaw Gateway.
//!
//! This example intentionally omits the Origin header, because some remote
//! Gateways' `gateway.controlUi.allowedOrigins` do not include
//! `app://kimi-desktop` from a non-local client.
//!
//! Usage:
//!   OPENCLAW_REMOTE_URL=ws://host:18789 \
//!   OPENCLAW_REMOTE_TOKEN=<token> \
//!     cargo run -p clarity-openclaw --example remote_raw

use futures_util::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::{Message, http::Request};

#[tokio::main]
async fn main() {
    let gateway_url = std::env::var("OPENCLAW_REMOTE_URL")
        .expect("set OPENCLAW_REMOTE_URL env var, e.g. ws://host:18789");
    let token = std::env::var("OPENCLAW_REMOTE_TOKEN").expect("set OPENCLAW_REMOTE_TOKEN env var");
    let session_key = "agent:main:main";

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
        // Some remote Gateways reject synthetic non-local Origins.
        // .header("Origin", "app://kimi-desktop")
        .body(())
        .unwrap();

    let (mut ws_stream, response) = tokio_tungstenite::connect_async(request)
        .await
        .expect("WebSocket connect failed");

    println!("HTTP status: {}", response.status());
    for (k, v) in response.headers() {
        println!("  {}: {:?}", k, v);
    }

    let connect = serde_json::json!({
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
            "scopes": [
                "operator.admin",
                "operator.read",
                "operator.write",
                "operator.approvals",
                "operator.pairing",
                "operator.talk.secrets"
            ],
            "auth": { "token": token }
        }
    });

    println!("\n>>> {}", connect);
    ws_stream
        .send(Message::Text(connect.to_string()))
        .await
        .unwrap();

    let timeout = tokio::time::Duration::from_secs(10);
    loop {
        match tokio::time::timeout(timeout, ws_stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                println!("<<< {}", text);
                if text.contains("hello-ok") || text.contains("\"ok\":true") {
                    break;
                }
                if text.contains("\"ok\":false") {
                    eprintln!("Authentication failed.");
                    return;
                }
            }
            Ok(Some(Ok(other))) => println!("<<< {:?}", other),
            Ok(Some(Err(e))) => {
                eprintln!("WS error: {}", e);
                return;
            }
            Ok(None) => {
                println!("Connection closed");
                return;
            }
            Err(_) => {
                println!("Timeout");
                return;
            }
        }
    }

    let message = format!(
        "Hello from Clarity.\n\n- Gateway: {}\n- sessionKey: {}\n- Sent at: {}",
        gateway_url,
        session_key,
        chrono::Utc::now().to_rfc3339()
    );

    let send = serde_json::json!({
        "type": "req",
        "id": "2",
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
