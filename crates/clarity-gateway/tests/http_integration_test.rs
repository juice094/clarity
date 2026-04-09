use axum::body::Body;
use axum::http::{Request, StatusCode};
use clarity_core::agent::{Agent, AgentConfig, MockLlm};
use clarity_core::registry::ToolRegistry;
use clarity_gateway::server::{create_api_router, create_admin_router, AppState};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::util::ServiceExt;

fn create_test_agent() -> Arc<Agent> {
    let registry = ToolRegistry::with_builtin_tools();
    let config = AgentConfig::new()
        .with_max_iterations(5)
        .with_read_only(false);
    let agent = Agent::with_config(registry, config).with_llm(Arc::new(MockLlm));
    Arc::new(agent)
}

#[tokio::test]
async fn test_health_check() {
    let state = Arc::new(AppState::new(create_test_agent()));
    let app = create_api_router(state);

    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
async fn test_chat_completions() {
    let state = Arc::new(AppState::new(create_test_agent()));
    let app = create_api_router(state);

    let request_body = serde_json::json!({
        "model": "test-model",
        "messages": [{"role": "user", "content": "Hello"}]
    });

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/chat/completions")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(request_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(body.get("choices").is_some());
    assert_eq!(body["choices"].as_array().unwrap().len(), 1);
    assert_eq!(body["choices"][0]["message"]["role"], "assistant");
}

#[tokio::test]
async fn test_admin_stats() {
    let state = Arc::new(AppState::new(create_test_agent()));
    let app = create_admin_router(state);

    let response = app
        .oneshot(Request::builder().uri("/api/stats").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(body.get("active_sessions").is_some());
    assert!(body.get("total_requests").is_some());
    assert!(body.get("uptime_seconds").is_some());
}

#[tokio::test]
async fn test_admin_tools() {
    let state = Arc::new(AppState::new(create_test_agent()));
    let app = create_admin_router(state);

    let response = app
        .oneshot(Request::builder().uri("/api/tools").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(body.get("tools").is_some());
    assert!(body["tools"].is_array());
}
