use axum::body::Body;
use axum::http::{Request, StatusCode};
use clarity_core::agent::{Agent, AgentConfig, MockLlm};
use clarity_core::background::BackgroundTaskManager;
use clarity_core::registry::ToolRegistry;
use clarity_gateway::server::{create_admin_router, create_api_router, AppState};
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

fn create_test_task_manager() -> Arc<BackgroundTaskManager> {
    let temp = std::env::temp_dir().join(format!("clarity-test-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&temp);
    let registry = ToolRegistry::with_builtin_tools();
    let llm = Arc::new(MockLlm);
    let executor = Arc::new(
        clarity_core::background::agent_executor::DefaultAgentTaskExecutor::new(
            llm,
            registry,
            &temp.join("work"),
        ),
    );
    Arc::new(
        BackgroundTaskManager::new(
            &temp.join("store"),
            &temp.join("work"),
            &temp.join("context"),
        )
        .with_agent_executor(executor),
    )
}

#[tokio::test]
async fn test_health_check() {
    let state = Arc::new(AppState::new(create_test_agent(), create_test_task_manager()).await);
    let app = create_api_router(state);

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
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
async fn test_chat_completions() {
    let state = Arc::new(AppState::new(create_test_agent(), create_test_task_manager()).await);
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
    let state = Arc::new(AppState::new(create_test_agent(), create_test_task_manager()).await);
    let app = create_admin_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/stats")
                .body(Body::empty())
                .unwrap(),
        )
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
    let state = Arc::new(AppState::new(create_test_agent(), create_test_task_manager()).await);
    let app = create_admin_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/tools")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(body.get("tools").is_some());
    assert!(body["tools"].is_array());
}

#[tokio::test]
async fn test_list_sessions() {
    let state = Arc::new(AppState::new(create_test_agent(), create_test_task_manager()).await);
    let app = create_admin_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/sessions")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(body.get("sessions").is_some());
    assert!(body["sessions"].is_array());
}

#[tokio::test]
async fn test_session_crud() {
    let state = Arc::new(AppState::new(create_test_agent(), create_test_task_manager()).await);

    // Create a session via the store directly
    state
        .session_store
        .create_session("test-session-123")
        .await
        .unwrap();
    state
        .session_store
        .append_message(
            "test-session-123",
            &clarity_gateway::session_store::SessionMessage::new("user", "Hello"),
        )
        .await
        .unwrap();

    // Get session
    let admin_app = create_admin_router(state.clone());
    let response = admin_app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/sessions/test-session-123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["session_id"], "test-session-123");
    assert!(body["messages"].is_array());
    assert_eq!(body["messages"].as_array().unwrap().len(), 1);

    // Delete session
    let response = admin_app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/sessions/test-session-123")
                .method("DELETE")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["deleted"], true);

    // Verify deletion
    let response = admin_app
        .oneshot(
            Request::builder()
                .uri("/api/sessions/test-session-123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    // Getting a deleted session returns empty messages, not an error
    assert_eq!(response.status(), StatusCode::OK);
}

// ============================================================================
// Background Tasks
// ============================================================================

#[tokio::test]
async fn test_task_lifecycle() {
    let state = Arc::new(AppState::new(create_test_agent(), create_test_task_manager()).await);
    let app = create_api_router(state.clone());

    // 1. Create a task
    let req_body = serde_json::json!({
        "name": "integration-test-task",
        "prompt": "Say hello",
        "max_iterations": 3
    });

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/v1/tasks")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(req_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let task_id = body["task_id"].as_str().unwrap();
    assert_eq!(body["status"], "Pending");

    // 2. Get the task
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/tasks/{}", task_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["task_id"], task_id);
    assert_eq!(body["name"], "integration-test-task");
    assert_eq!(body["prompt"], "Say hello");

    // 3. Cancel the task
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/tasks/{}", task_id))
                .method("DELETE")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["cancelled"], true);
}

#[tokio::test]
async fn test_get_nonexistent_task() {
    let state = Arc::new(AppState::new(create_test_agent(), create_test_task_manager()).await);
    let app = create_api_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/v1/tasks/nonexistent-task-123")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

// ============================================================================
// Admin — Models & Provider
// ============================================================================

#[tokio::test]
async fn test_admin_models() {
    let state = Arc::new(AppState::new(create_test_agent(), create_test_task_manager()).await);
    let app = create_admin_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/models")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(body.get("models").is_some());
    assert!(body["models"].is_array());
}

#[tokio::test]
async fn test_admin_switch_provider_invalid() {
    let state = Arc::new(AppState::new(create_test_agent(), create_test_task_manager()).await);
    let app = create_admin_router(state);

    let req_body = serde_json::json!({"provider": "nonexistent-provider-12345"});

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/provider")
                .method("POST")
                .header("content-type", "application/json")
                .body(Body::from(req_body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    // Invalid provider should return 400
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(body.get("message").is_some());
}
