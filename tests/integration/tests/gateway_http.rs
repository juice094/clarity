use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use clarity_core::background::BackgroundTaskManager;
use clarity_gateway::{handlers, server::AppState, ws::ws_handler};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Build the same API router that the gateway uses, but wired to a fresh
/// `AppState` so we can exercise it without spawning a real server.
async fn test_api_router() -> Router {
    let registry = clarity_core::registry::ToolRegistry::with_builtin_tools();
    let config = clarity_core::agent::AgentConfig::new();
    let agent = Arc::new(clarity_core::agent::Agent::with_config(registry, config));
    let temp = std::env::temp_dir().join(format!("clarity-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp);
    let task_manager = Arc::new(BackgroundTaskManager::new(
        temp.join("store"),
        temp.join("work"),
        temp.join("context"),
    ));
    let state = Arc::new(AppState::new(agent, task_manager).await);
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    Router::new()
        .route("/health", get(handlers::health_check))
        .route("/v1/chat/completions", post(handlers::chat_completions))
        .route("/ws", get(ws_handler))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

/// Scenario D — Health check should return 200 with a JSON payload.
#[tokio::test]
async fn test_gateway_health_check() {
    let app = test_api_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(json["status"], "healthy");
}

/// Scenario D — A request without a user message should return 400.
/// This exercises the handler input validation without requiring a live LLM.
#[tokio::test]
async fn test_gateway_chat_completions_missing_user_message() {
    let app = test_api_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/chat/completions")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"model":"test","messages":[{"role":"system","content":"hi"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// Scenario D — A well-formed request should reach the handler without
/// panicking. Because the gateway currently instantiates a real LLM via
/// `LlmFactory::kimi()`, it returns 500 when no API key is configured.
/// This test captures that behaviour so we notice if the handler panics
/// or the contract changes.
#[tokio::test]
async fn test_gateway_chat_completions_well_formed_request() {
    let app = test_api_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/chat/completions")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"model":"test","messages":[{"role":"user","content":"hello"}]}"#,
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    // Currently returns 500 because no KIMI_API_KEY is set.
    // The important thing is that it does NOT panic.
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

/// Gateway WebSocket should forward WireMessages when `use_wire` is true.
#[tokio::test]
async fn test_gateway_websocket_wire_forwarding() {
    use clarity_core::agent::{Agent, AgentConfig, MockLlm};
    use clarity_core::registry::ToolRegistry;
    use clarity_gateway::server::create_api_router;
    use futures::{SinkExt, StreamExt};
    use std::sync::Arc;
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

    let registry = ToolRegistry::with_builtin_tools();
    let config = AgentConfig::new()
        .with_max_iterations(5)
        .with_read_only(false);
    let agent = Arc::new(Agent::with_config(registry, config).with_llm(Arc::new(MockLlm)));
    let temp = std::env::temp_dir().join(format!("clarity-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp);
    let task_manager = Arc::new(BackgroundTaskManager::new(
        temp.join("store"),
        temp.join("work"),
        temp.join("context"),
    ));
    let state = Arc::new(clarity_gateway::server::AppState::new(agent, task_manager).await);
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
    let welcome = ws_stream.next().await.unwrap().unwrap();
    let welcome_text = welcome.to_text().unwrap();
    let welcome_json: serde_json::Value = serde_json::from_str(welcome_text).unwrap();
    assert_eq!(welcome_json["type"], "welcome");

    // Send Chat request with use_wire
    let chat = serde_json::json!({
        "type": "chat",
        "message": "Hello",
        "use_wire": true
    });
    ws_stream
        .send(Message::Text(chat.to_string()))
        .await
        .unwrap();

    // Collect wire messages
    let mut wire_types = Vec::new();
    loop {
        let msg = ws_stream.next().await.unwrap().unwrap();
        let text = msg.to_text().unwrap();
        let json: serde_json::Value = serde_json::from_str(text).unwrap();
        let msg_type = json["type"].as_str().unwrap().to_string();
        wire_types.push(msg_type.clone());
        if msg_type == "turn_end" {
            break;
        }
    }

    assert!(
        wire_types.contains(&"turn_begin".to_string()),
        "expected turn_begin in {:?}",
        wire_types
    );
    assert!(
        wire_types.contains(&"content_part".to_string()),
        "expected content_part in {:?}",
        wire_types
    );
    assert_eq!(wire_types.last().unwrap(), "turn_end");

    ws_stream.close(None).await.unwrap();
}

/// TUI -> Gateway WebSocket wire forwarding using clarity_wire::Wire.
/// Verifies that a Wire can be bridged to the Gateway WebSocket and
/// WireMessages flow end-to-end.
#[tokio::test]
async fn test_tui_wire_to_gateway_websocket() {
    use clarity_core::agent::{Agent, AgentConfig, MockLlm};
    use clarity_core::registry::ToolRegistry;
    use clarity_gateway::server::create_api_router;
    use clarity_wire::{Wire, WireMessage};
    use futures::{SinkExt, StreamExt};
    use std::sync::Arc;
    use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};

    let registry = ToolRegistry::with_builtin_tools();
    let config = AgentConfig::new()
        .with_max_iterations(5)
        .with_read_only(false);
    let agent = Arc::new(Agent::with_config(registry, config).with_llm(Arc::new(MockLlm)));
    let temp = std::env::temp_dir().join(format!("clarity-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp);
    let task_manager = Arc::new(BackgroundTaskManager::new(
        temp.join("store"),
        temp.join("work"),
        temp.join("context"),
    ));
    let state = Arc::new(clarity_gateway::server::AppState::new(agent, task_manager).await);
    let app = create_api_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let url = format!("ws://{}/ws", addr);
    let (ws_stream, response) = connect_async(&url).await.unwrap();
    assert_eq!(response.status(), 101);
    let (mut ws_tx, mut ws_rx) = ws_stream.split();

    // Skip welcome message
    let welcome = ws_rx.next().await.unwrap().unwrap();
    let welcome_text = welcome.to_text().unwrap();
    let welcome_json: serde_json::Value = serde_json::from_str(welcome_text).unwrap();
    assert_eq!(welcome_json["type"], "welcome");

    // Create a Wire and bridge WebSocket -> Wire soul side
    let wire = Wire::new();
    let soul = wire.soul_side().clone();
    let mut ui_side = wire.ui_side(false);

    // Spawn a forwarder that deserializes WebSocket text into WireMessages
    tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_rx.next().await {
            if let Message::Text(text) = msg {
                if let Ok(wire_msg) = serde_json::from_str::<WireMessage>(&text) {
                    soul.send(wire_msg);
                }
            }
        }
    });

    // Send Chat request with use_wire
    let chat = serde_json::json!({
        "type": "chat",
        "message": "Hello",
        "use_wire": true
    });
    ws_tx.send(Message::Text(chat.to_string())).await.unwrap();

    // Read from the Wire UI side
    let mut wire_types = Vec::new();
    loop {
        let msg = tokio::time::timeout(tokio::time::Duration::from_secs(5), ui_side.recv())
            .await
            .expect("timeout waiting for wire message")
            .expect("wire closed");
        let msg_type = match msg {
            WireMessage::TurnBegin { .. } => "turn_begin",
            WireMessage::ContentPart { .. } => "content_part",
            WireMessage::TurnEnd => "turn_end",
            WireMessage::StepBegin { .. } => "step_begin",
            WireMessage::ToolCall { .. } => "tool_call",
            WireMessage::ToolResult { .. } => "tool_result",
            WireMessage::StatusUpdate { .. } => "status_update",
            WireMessage::Usage { .. } => "usage",
            WireMessage::CompactionBegin => "compaction_begin",
            WireMessage::CompactionEnd => "compaction_end",
            WireMessage::PlanStepBegin { .. } => "plan_step_begin",
            WireMessage::PlanStepEnd { .. } => "plan_step_end",
            WireMessage::DraftEvent { .. } => "draft_event",
            WireMessage::PlanStepSkipped { .. } => "plan_step_skipped",
        };
        wire_types.push(msg_type.to_string());
        if msg_type == "turn_end" {
            break;
        }
    }

    assert!(
        wire_types.contains(&"turn_begin".to_string()),
        "expected turn_begin in {:?}",
        wire_types
    );
    assert!(
        wire_types.contains(&"content_part".to_string()),
        "expected content_part in {:?}",
        wire_types
    );
    assert_eq!(wire_types.last().unwrap(), "turn_end");

    let _ = ws_tx.close().await;
}

/// Scenario D — Empty request body should return 400.
#[tokio::test]
async fn test_gateway_chat_completions_empty_body() {
    let app = test_api_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/chat/completions")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// Scenario D — Invalid JSON should return 400.
#[tokio::test]
async fn test_gateway_chat_completions_invalid_json() {
    let app = test_api_router().await;
    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/chat/completions")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from("not-json-at-all"))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

/// Scenario D — Admin tools list should return 200 with tool schemas.
/// Admin router requires no token when CLARITY_ADMIN_TOKEN is unset.
#[tokio::test]
async fn test_gateway_admin_tools_list() {
    std::env::remove_var("CLARITY_ADMIN_TOKEN");
    let registry = clarity_core::registry::ToolRegistry::with_builtin_tools();
    let config = clarity_core::agent::AgentConfig::new();
    let agent = Arc::new(clarity_core::agent::Agent::with_config(registry, config));
    let temp = std::env::temp_dir().join(format!("clarity-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp);
    let task_manager = Arc::new(BackgroundTaskManager::new(
        temp.join("store"),
        temp.join("work"),
        temp.join("context"),
    ));
    let state = Arc::new(clarity_gateway::server::AppState::new(agent, task_manager).await);
    let app = clarity_gateway::server::create_admin_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/tools")
                .method("GET")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(json["tools"].is_array(), "expected tools array in response");
}
