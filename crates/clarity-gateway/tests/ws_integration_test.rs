#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
use clarity_core::agent::{Agent, AgentConfig, MockLlm};
use clarity_core::background::BackgroundTaskManager;
use clarity_core::registry::ToolRegistry;
use clarity_gateway::server::{AppState, create_api_router};
use futures::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

fn create_test_agent() -> Arc<Agent> {
    let registry = ToolRegistry::with_builtin_tools();
    let config = AgentConfig::new()
        .with_max_iterations(5)
        .with_read_only(false);
    let agent = Agent::with_config(registry, config).with_llm(Arc::new(MockLlm));
    Arc::new(agent)
}

fn create_test_task_manager() -> Arc<BackgroundTaskManager> {
    let temp = std::env::temp_dir().join(format!("clarity-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp);
    Arc::new(BackgroundTaskManager::new(
        temp.join("store"),
        temp.join("work"),
        temp.join("context"),
    ))
}

#[tokio::test]
async fn test_websocket_upgrade_and_ping_pong() {
    let state = Arc::new(
        AppState::new(create_test_agent(), create_test_task_manager())
            .await
            .expect("failed to create app state"),
    );
    let app = create_api_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let url = format!("ws://{}/ws", addr);
    let (mut ws_stream, response) = connect_async(&url).await.unwrap();

    assert_eq!(response.status(), 101);

    // Skip welcome message
    let _ = ws_stream.next().await.unwrap().unwrap();

    // Send Ping
    let ping = serde_json::json!({"type": "ping"});
    ws_stream
        .send(Message::Text(ping.to_string()))
        .await
        .unwrap();

    // Receive Pong
    let msg = ws_stream.next().await.unwrap().unwrap();
    let text = msg.to_text().unwrap();
    let resp: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(resp["type"], "pong");

    ws_stream.close(None).await.unwrap();
}

#[tokio::test]
async fn test_websocket_chat() {
    let state = Arc::new(
        AppState::new(create_test_agent(), create_test_task_manager())
            .await
            .expect("failed to create app state"),
    );
    let app = create_api_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let url = format!("ws://{}/ws", addr);
    let (mut ws_stream, _) = connect_async(&url).await.unwrap();

    // Skip welcome message
    let _ = ws_stream.next().await.unwrap().unwrap();

    // Send Chat
    let chat = serde_json::json!({
        "type": "chat",
        "message": "Hello"
    });
    ws_stream
        .send(Message::Text(chat.to_string()))
        .await
        .unwrap();

    // Receive response
    let msg = ws_stream.next().await.unwrap().unwrap();
    let text = msg.to_text().unwrap();
    let resp: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(resp["type"], "chat");
    assert!(resp.get("message").is_some());

    ws_stream.close(None).await.unwrap();
}

#[tokio::test]
async fn test_websocket_get_history() {
    let state = Arc::new(
        AppState::new(create_test_agent(), create_test_task_manager())
            .await
            .expect("failed to create app state"),
    );
    let app = create_api_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let url = format!("ws://{}/ws", addr);
    let (mut ws_stream, _) = connect_async(&url).await.unwrap();

    // Skip welcome message
    let _ = ws_stream.next().await.unwrap().unwrap();

    // Send a chat message first
    let chat = serde_json::json!({
        "type": "chat",
        "message": "Hello history"
    });
    ws_stream
        .send(Message::Text(chat.to_string()))
        .await
        .unwrap();

    // Consume the chat response
    let _ = ws_stream.next().await.unwrap().unwrap();

    // Send GetHistory
    let get_history = serde_json::json!({"type": "get_history"});
    ws_stream
        .send(Message::Text(get_history.to_string()))
        .await
        .unwrap();

    // Receive history response
    let msg = ws_stream.next().await.unwrap().unwrap();
    let text = msg.to_text().unwrap();
    let resp: serde_json::Value = serde_json::from_str(text).unwrap();
    assert_eq!(resp["type"], "history");
    assert!(resp.get("messages").is_some());
    assert!(resp["messages"].is_array());
    // Should have at least 2 messages (user + assistant)
    assert!(resp["messages"].as_array().unwrap().len() >= 2);

    ws_stream.close(None).await.unwrap();
}

#[tokio::test]
async fn test_websocket_chat_wire_streaming_uses_envelope() {
    let state = Arc::new(
        AppState::new(create_test_agent(), create_test_task_manager())
            .await
            .expect("failed to create app state"),
    );
    let app = create_api_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let url = format!("ws://{}/ws", addr);
    let (mut ws_stream, _) = connect_async(&url).await.unwrap();

    // Skip welcome message
    let _ = ws_stream.next().await.unwrap().unwrap();

    // Send Chat with wire streaming enabled
    let chat = serde_json::json!({
        "type": "chat",
        "message": "Hello",
        "use_wire": true
    });
    ws_stream
        .send(Message::Text(chat.to_string()))
        .await
        .unwrap();

    // Collect streamed envelopes until no more arrive for a short window.
    let mut envelopes = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    while tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(std::time::Duration::from_millis(200), ws_stream.next()).await {
            Ok(Some(Ok(Message::Text(text)))) => {
                let value: serde_json::Value = serde_json::from_str(&text).unwrap();
                envelopes.push(value);
            }
            Ok(Some(Ok(Message::Close(_)))) | Ok(None) => break,
            _ => break,
        }
    }

    // Every streaming message must be a WsResponse envelope.
    assert!(
        !envelopes.is_empty(),
        "expected at least one streaming envelope"
    );
    for envelope in &envelopes {
        assert_eq!(
            envelope["type"], "wire_message",
            "streaming messages must be wrapped in WsResponse::WireMessage: {:?}",
            envelope
        );
        assert!(
            envelope.get("payload").is_some(),
            "wire_message envelope must contain payload"
        );
    }

    // At least one payload should be a recognized WireMessage type.
    let payload_types: Vec<&str> = envelopes
        .iter()
        .filter_map(|e| e["payload"]["type"].as_str())
        .collect();
    assert!(
        payload_types
            .iter()
            .any(|t| *t == "turn_begin" || *t == "content_part"),
        "expected turn_begin or content_part in payloads, got {:?}",
        payload_types
    );

    ws_stream.close(None).await.unwrap();
}
