//! ACP bridge integration tests.
//!
//! These tests use lightweight mock WebSocket servers for both the Kimi cloud
//! ACP endpoint and the local Clarity Gateway, then run the bridge client
//! between them and verify bidirectional message relay.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use clarity_claw::acp_bridge::{
    AcpBridgeConfig, LocalBackend, run_acp_gateway_bridge, run_acp_gateway_bridge_with_options,
};
use clarity_contract::retry::RetryConfig;
use futures::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{accept_async, tungstenite::Message};

/// Start a mock ACP server that sends `initial_event` to the first connected
/// client and records any messages received from that client.
async fn start_mock_acp_server(
    initial_event: serde_json::Value,
) -> (SocketAddr, tokio::sync::mpsc::UnboundedReceiver<Message>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (mut ws_write, mut ws_read) = accept_async(stream).await.unwrap().split();

        // Wait briefly for the bridge handshake, then send the cloud event.
        tokio::time::sleep(Duration::from_millis(100)).await;
        let _ = ws_write
            .send(Message::Text(initial_event.to_string()))
            .await;

        // Record anything the bridge sends back to the cloud.
        while let Some(Ok(msg)) = ws_read.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
            if matches!(msg, Message::Text(_) | Message::Binary(_)) {
                let _ = tx.send(msg);
            }
        }
    });

    (addr, rx)
}

/// Start a mock Gateway server that replies to chat requests with a fixed
/// response and records any requests received.
async fn start_mock_gateway_server(
    reply: serde_json::Value,
) -> (SocketAddr, tokio::sync::mpsc::UnboundedReceiver<Message>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Message>();

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (mut ws_write, mut ws_read) = accept_async(stream).await.unwrap().split();

        // Send welcome frame.
        let _ = ws_write
            .send(Message::Text(r#"{"type":"welcome"}"#.into()))
            .await;

        while let Some(Ok(msg)) = ws_read.next().await {
            if matches!(msg, Message::Close(_)) {
                break;
            }
            if matches!(msg, Message::Text(_) | Message::Binary(_)) {
                let _ = tx.send(msg.clone());
                if let Ok(text) = msg.to_text() {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
                        if value.get("type").and_then(|t| t.as_str()) == Some("chat") {
                            let _ = ws_write.send(Message::Text(reply.to_string())).await;
                        }
                    }
                }
            }
        }
    });

    (addr, rx)
}

#[tokio::test]
async fn test_acp_bridge_relays_payload_message_to_gateway() {
    let initial = serde_json::json!({
        "chat_id": "chat-1",
        "payload": { "message": "hello from cloud" },
    });
    let (acp_addr, mut acp_rx) = start_mock_acp_server(initial).await;
    let (gw_addr, mut gw_rx) =
        start_mock_gateway_server(serde_json::json!({"type":"chat","message":"gateway reply"}))
            .await;

    let config = AcpBridgeConfig {
        mode: "acp".into(),
        url: format!("ws://{}", acp_addr),
        kimi_api_host: "http://localhost".into(),
        kimi_file_download_dir: std::env::temp_dir(),
        token: "test-token".into(),
        ..Default::default()
    };

    // Give both mock servers a moment to start accepting before the bridge
    // attempts the handshakes.
    tokio::time::sleep(Duration::from_millis(200)).await;

    let bridge_handle = tokio::spawn(async move {
        run_acp_gateway_bridge(
            &config,
            &format!("http://{}", gw_addr),
            LocalBackend::ClarityGateway,
        )
        .await
        .ok()
    });

    // Verify the gateway received the forwarded chat request.
    let gw_msg = tokio::time::timeout(Duration::from_secs(5), gw_rx.recv())
        .await
        .expect("gateway receive timed out")
        .expect("gateway server closed");
    let gw_text = gw_msg.to_text().unwrap();
    let gw_json: serde_json::Value = serde_json::from_str(gw_text).unwrap();
    assert_eq!(gw_json["type"], "chat");
    assert_eq!(gw_json["message"], "hello from cloud");

    // Verify the ACP cloud received the Gateway response forwarded back.
    // Drain the initial ping message first.
    let acp_msg = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let msg = acp_rx.recv().await.expect("acp server closed");
            let text = msg.to_text().unwrap();
            let value: serde_json::Value = serde_json::from_str(text).unwrap();
            if value.get("method").and_then(|m| m.as_str()) == Some("ping") {
                continue;
            }
            break msg;
        }
    })
    .await
    .expect("acp receive timed out");
    let acp_text = acp_msg.to_text().unwrap();
    let acp_json: serde_json::Value = serde_json::from_str(acp_text).unwrap();
    assert_eq!(acp_json["method"], "send_message_stream");
    assert_eq!(acp_json["chat_id"], "chat-1");

    bridge_handle.abort();
}

#[tokio::test]
async fn test_acp_bridge_extracts_flat_text() {
    let initial = serde_json::json!({
        "chat_id": "chat-2",
        "text": "flat text message",
    });
    let (acp_addr, mut acp_rx) = start_mock_acp_server(initial).await;
    let (gw_addr, mut gw_rx) =
        start_mock_gateway_server(serde_json::json!({"type":"chat","message":"ok"})).await;

    let config = AcpBridgeConfig {
        mode: "acp".into(),
        url: format!("ws://{}", acp_addr),
        kimi_api_host: "http://localhost".into(),
        kimi_file_download_dir: std::env::temp_dir(),
        token: "test-token".into(),
        ..Default::default()
    };

    tokio::time::sleep(Duration::from_millis(200)).await;

    let bridge_handle = tokio::spawn(async move {
        run_acp_gateway_bridge(
            &config,
            &format!("http://{}", gw_addr),
            LocalBackend::ClarityGateway,
        )
        .await
        .ok()
    });

    let gw_msg = tokio::time::timeout(Duration::from_secs(5), gw_rx.recv())
        .await
        .expect("gateway receive timed out")
        .expect("gateway server closed");
    let gw_json: serde_json::Value = serde_json::from_str(gw_msg.to_text().unwrap()).unwrap();
    assert_eq!(gw_json["message"], "flat text message");

    bridge_handle.abort();
    let _ = acp_rx.recv().await;
}

/// Start a mock ACP server that accepts multiple connections, sends `event` on
/// each new connection, and records messages from the bridge.
async fn start_mock_acp_server_multi(
    event: serde_json::Value,
) -> (
    SocketAddr,
    tokio::sync::mpsc::UnboundedReceiver<Message>,
    Arc<AtomicUsize>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = count.clone();

    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            count_clone.fetch_add(1, Ordering::SeqCst);
            let tx = tx.clone();
            let event = event.clone();
            tokio::spawn(async move {
                let (mut ws_write, mut ws_read) = accept_async(stream).await.unwrap().split();
                tokio::time::sleep(Duration::from_millis(50)).await;
                let _ = ws_write.send(Message::Text(event.to_string())).await;
                while let Some(Ok(msg)) = ws_read.next().await {
                    if matches!(msg, Message::Close(_)) {
                        break;
                    }
                    if matches!(msg, Message::Text(_) | Message::Binary(_)) {
                        let _ = tx.send(msg);
                    }
                }
            });
        }
    });

    (addr, rx, count)
}

/// Start a mock Gateway server that accepts multiple connections, replies to
/// the first chat on each connection, then closes to force a bridge reconnect.
async fn start_mock_gateway_server_flaky(
    reply: serde_json::Value,
) -> (
    SocketAddr,
    tokio::sync::mpsc::UnboundedReceiver<Message>,
    Arc<AtomicUsize>,
) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Message>();
    let count = Arc::new(AtomicUsize::new(0));
    let count_clone = count.clone();

    tokio::spawn(async move {
        loop {
            let (stream, _) = listener.accept().await.unwrap();
            count_clone.fetch_add(1, Ordering::SeqCst);
            let tx = tx.clone();
            let reply = reply.clone();
            tokio::spawn(async move {
                let (mut ws_write, mut ws_read) = accept_async(stream).await.unwrap().split();
                let _ = ws_write
                    .send(Message::Text(r#"{"type":"welcome"}"#.into()))
                    .await;
                while let Some(Ok(msg)) = ws_read.next().await {
                    if matches!(msg, Message::Close(_)) {
                        break;
                    }
                    if matches!(msg, Message::Text(_) | Message::Binary(_)) {
                        let _ = tx.send(msg.clone());
                        if let Ok(text) = msg.to_text() {
                            if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
                                if value.get("type").and_then(|t| t.as_str()) == Some("chat") {
                                    let _ = ws_write.send(Message::Text(reply.to_string())).await;
                                    tokio::time::sleep(Duration::from_millis(50)).await;
                                    break;
                                }
                            }
                        }
                    }
                }
            });
        }
    });

    (addr, rx, count)
}

#[tokio::test]
async fn test_acp_bridge_reconnects_after_gateway_disconnect() {
    let event = serde_json::json!({
        "chat_id": "chat-reconnect",
        "payload": { "message": "hello again" },
    });
    let (acp_addr, mut acp_rx, _acp_count) = start_mock_acp_server_multi(event).await;
    let reply = serde_json::json!({"type":"chat","message":"gateway reply after reconnect"});
    let (gw_addr, _gw_rx, gw_count) = start_mock_gateway_server_flaky(reply).await;

    let config = AcpBridgeConfig {
        mode: "acp".into(),
        url: format!("ws://{}", acp_addr),
        kimi_api_host: "http://localhost".into(),
        kimi_file_download_dir: std::env::temp_dir(),
        token: "test-token".into(),
        ..Default::default()
    };

    let (_shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    let bridge_handle = tokio::spawn(async move {
        run_acp_gateway_bridge_with_options(
            &config,
            &format!("http://{}", gw_addr),
            LocalBackend::ClarityGateway,
            shutdown_rx,
            RetryConfig {
                max_retries: 10,
                initial_backoff_ms: 50,
                max_backoff_ms: 200,
                backoff_multiplier: 2.0,
            },
            None,
            None,
        )
        .await
        .ok()
    });

    // Wait long enough for the initial connection plus at least one reconnect.
    tokio::time::sleep(Duration::from_millis(900)).await;

    let gw_connections = gw_count.load(Ordering::SeqCst);
    assert!(
        gw_connections >= 2,
        "expected at least 2 Gateway connections, got {}",
        gw_connections
    );

    // Drain ACP responses; we should see send_message_stream from multiple
    // Gateway connections.
    let mut responses = 0;
    let deadline = tokio::time::Instant::now() + Duration::from_millis(300);
    while tokio::time::Instant::now() < deadline {
        if let Ok(msg) = acp_rx.try_recv() {
            if let Ok(text) = msg.to_text() {
                if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
                    if value.get("method").and_then(|m| m.as_str()) == Some("send_message_stream") {
                        responses += 1;
                    }
                }
            }
        } else {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }
    assert!(
        responses >= 2,
        "expected at least 2 ACP send_message_stream responses, got {}",
        responses
    );

    bridge_handle.abort();
}
