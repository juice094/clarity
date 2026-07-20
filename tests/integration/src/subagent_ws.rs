//! Gateway subagent WebSocket streaming integration tests.
//!
//! These tests start a real TCP server and exercise the WebSocket subagent
//! streaming endpoints end-to-end.

use std::sync::Arc;
use std::time::Duration;

use clarity_core::agent::{Agent, AgentConfig, MockLlm};
use clarity_core::background::BackgroundTaskManager;
use clarity_core::registry::ToolRegistry;
use clarity_gateway::server::{AppState, create_api_router};
use futures::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::tungstenite::Message;

async fn test_state() -> Arc<AppState> {
    let registry = ToolRegistry::with_builtin_tools();
    let config = AgentConfig::new()
        .with_max_iterations(5)
        .with_read_only(false);
    let agent = Arc::new(Agent::with_config(registry, config).with_llm(Arc::new(MockLlm)));

    let temp = tempfile::tempdir().unwrap();
    let task_manager = Arc::new(BackgroundTaskManager::new(
        temp.path().join("store"),
        temp.path().join("work"),
        temp.path().join("context"),
    ));

    Arc::new(
        AppState::new_with_home(agent, task_manager, temp.path())
            .await
            .expect("failed to create app state"),
    )
}

async fn connect_and_wait_welcome(
    port: u16,
) -> tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>> {
    let url = format!("ws://127.0.0.1:{}/ws", port);
    let (mut ws, _) = tokio_tungstenite::connect_async(&url).await.unwrap();

    let welcome = ws.next().await.unwrap().unwrap();
    let welcome: serde_json::Value = match welcome {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        other => panic!("expected text welcome, got {:?}", other),
    };
    assert_eq!(welcome["type"], "welcome");

    ws
}

async fn send_json(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    value: serde_json::Value,
) {
    ws.send(Message::Text(value.to_string())).await.unwrap();
}

async fn recv_json(
    ws: &mut tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
) -> serde_json::Value {
    let msg = ws.next().await.unwrap().unwrap();
    match msg {
        Message::Text(text) => serde_json::from_str(&text).unwrap(),
        other => panic!("expected text message, got {:?}", other),
    }
}

#[tokio::test]
async fn test_ws_list_subagent_types() {
    let state = test_state().await;
    let app = create_api_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let mut ws = connect_and_wait_welcome(port).await;

    send_json(&mut ws, json!({"type": "list_subagent_types"})).await;
    let response = recv_json(&mut ws).await;
    assert_eq!(response["type"], "subagent_types");
    let types = response["types"].as_array().unwrap();
    assert!(!types.is_empty());
    assert!(types.iter().any(|t| t["name"] == "coder"));

    let _ = ws.close(None).await;
    server.abort();
}

#[tokio::test]
async fn test_ws_run_subagent_stream_returns_result() {
    let state = test_state().await;
    let app = create_api_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let mut ws = connect_and_wait_welcome(port).await;

    send_json(
        &mut ws,
        json!({
            "type": "run_subagent_stream",
            "description": "ws-stream-test",
            "agent_type": "coder",
            "prompt": "Say hello",
            "max_iterations": 1
        }),
    )
    .await;

    let started = recv_json(&mut ws).await;
    assert_eq!(started["type"], "subagent_run_started");
    assert!(!started["run_id"].as_str().unwrap().is_empty());

    // Drain progress events until the final result arrives.
    let result = tokio::time::timeout(Duration::from_secs(30), async {
        loop {
            let msg = recv_json(&mut ws).await;
            if msg["type"] == "subagent_result" || msg["type"] == "error" {
                break msg;
            }
            assert_eq!(msg["type"], "subagent_progress");
        }
    })
    .await
    .expect("subagent stream timed out");
    assert_eq!(result["type"], "subagent_result");
    assert_eq!(result["agent_type"], "coder");
    assert_eq!(result["status"], "completed");

    let _ = ws.close(None).await;
    server.abort();
}

#[tokio::test]
async fn test_ws_run_subagent_non_streaming() {
    let state = test_state().await;
    let app = create_api_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move {
        let _ = axum::serve(listener, app).await;
    });

    let mut ws = connect_and_wait_welcome(port).await;

    send_json(
        &mut ws,
        json!({
            "type": "run_subagent",
            "description": "ws-test",
            "agent_type": "review",
            "prompt": "Review this code",
            "max_iterations": 1,
            "read_only": true
        }),
    )
    .await;

    let result = tokio::time::timeout(Duration::from_secs(30), recv_json(&mut ws))
        .await
        .expect("subagent request timed out");
    assert_eq!(result["type"], "subagent_result");
    assert_eq!(result["agent_type"], "review");

    let _ = ws.close(None).await;
    server.abort();
}
