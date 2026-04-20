//! Gateway API integration tests — error-path & webhook coverage
//!
//! Complements `http_integration_test.rs` (happy-path) with edge-case
//! and webhook-specific assertions. Uses Tower `ServiceExt::oneshot` to
//! avoid real network ports.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

use clarity_core::background::BackgroundTaskManager;
use clarity_core::registry::ToolRegistry;
use clarity_gateway::server::{create_api_router, AppState};

fn test_agent() -> Arc<clarity_core::Agent> {
    let registry = ToolRegistry::with_builtin_tools();
    Arc::new(clarity_core::Agent::new(registry))
}

fn test_task_manager() -> Arc<BackgroundTaskManager> {
    let dir = tempfile::tempdir().unwrap();
    Arc::new(BackgroundTaskManager::new(
        dir.path().join("tasks"),
        dir.path().join("work"),
        dir.path().join("context"),
    ))
}

async fn test_app_state() -> Arc<AppState> {
    let agent = test_agent();
    let tm = test_task_manager();
    Arc::new(AppState::new(agent, tm).await)
}

async fn read_json_body(res: axum::response::Response) -> serde_json::Value {
    let body = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&body).unwrap()
}

// ============================================================================
// Chat Completions — error paths
// ============================================================================

#[tokio::test]
async fn test_chat_completions_missing_user_message() {
    let state = test_app_state().await;
    let app = create_api_router(state);

    let req_body = serde_json::json!({
        "model": "test-model",
        "messages": [{"role": "system", "content": "You are helpful"}]
    });

    let res = app
        .oneshot(
            Request::builder()
                .uri("/v1/chat/completions")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(req_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = read_json_body(res).await;
    assert!(body["error"].as_str().unwrap().contains("No user message"));
}

#[tokio::test]
async fn test_chat_completions_no_llm_provider() {
    // Agent has no LLM configured → controller emits Error → handler returns 500.
    let state = test_app_state().await;
    let app = create_api_router(state);

    let req_body = serde_json::json!({
        "model": "test-model",
        "messages": [{"role": "user", "content": "Hello"}]
    });

    let res = app
        .oneshot(
            Request::builder()
                .uri("/v1/chat/completions")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(req_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = read_json_body(res).await;
    assert!(body["error"].as_str().unwrap().contains("Agent execution error"));
}

#[tokio::test]
async fn test_chat_completions_session_id_roundtrip() {
    // Ensures the handler does not panic when session_id is supplied,
    // even if the session does not exist and no LLM is configured.
    let state = test_app_state().await;
    let app = create_api_router(state);

    let req_body = serde_json::json!({
        "model": "test-model",
        "messages": [{"role": "user", "content": "Hello"}],
        "session_id": "test-session-42"
    });

    let res = app
        .oneshot(
            Request::builder()
                .uri("/v1/chat/completions")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(req_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // We only care that it doesn't panic; the exact status depends on LLM.
    let _body = read_json_body(res).await;
}

// ============================================================================
// Webhook Router Tests
// ============================================================================

use clarity_gateway::channels::webhook::{WebhookChannel, WebhookRequest, WebhookResponse};
use clarity_gateway::channels::ChannelConfig;

#[tokio::test]
async fn test_webhook_empty_message() {
    let agent = test_agent();
    let config = ChannelConfig::new().enabled();
    let channel = WebhookChannel::new(config);
    let router = channel.create_router(agent).unwrap();

    let req_body = serde_json::to_string(&WebhookRequest {
        message: Some("".to_string()),
        user_id: None,
        username: None,
        metadata: None,
        text: None,
        content: None,
        msg_type: None,
    })
    .unwrap();

    let res = router
        .oneshot(
            Request::builder()
                .uri("/webhook")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(req_body))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body: WebhookResponse = serde_json::from_slice(
        &res.into_body().collect().await.unwrap().to_bytes(),
    )
    .unwrap();
    assert_eq!(body.success, false);
    assert!(body.error.as_ref().unwrap().contains("Empty message"));
}
