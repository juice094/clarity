//! Axum HTTP handlers for the public API and admin UI.
//!
//! Each submodule groups related routes (chat, tasks, files, config, etc.).

/// Admin/configuration handlers.
pub mod admin;
/// OpenAI-compatible chat completion handler.
pub mod chat;
/// Claw device registry handlers.
pub mod claw;
/// Claw Mesh sync handlers (Phase 7).
pub mod claw_sync;
/// Provider and alias configuration handlers.
pub mod config;
/// Scheduled cron task handlers.
pub mod cron;
/// File operation handlers.
pub mod files;
/// MCP server management handlers.
pub mod mcp;
/// Memory search handlers.
pub mod memory;
/// Session management handlers.
pub mod sessions;
/// Background and parallel task handlers.
pub mod tasks;
/// Telemetry endpoint handlers (requires the `telemetry-api` feature).
#[cfg(feature = "telemetry-api")]
pub mod telemetry;
/// Thread-scoped chat completion handler (v2 sessions).
pub mod thread_chat;
/// Thread management handlers (v2 sessions).
pub mod threads;

/// Trait abstracting the agent operations required by HTTP handlers.
///
/// Decouples handlers from the concrete `AppState` god-object so that
/// only the operations actually used by the HTTP layer are exposed.
pub(crate) trait AgentHandle {
    fn clone_agent(&self) -> clarity_core::agent::Agent;
    fn registry(&self) -> &clarity_core::registry::ToolRegistry;
    fn set_approval_mode(&self, mode: clarity_core::approval::ApprovalMode);
    fn approval_mode(&self) -> clarity_core::approval::ApprovalMode;
    fn set_llm(&self, backend: std::sync::Arc<dyn clarity_core::agent::LlmProvider>);
    fn set_provider_label<S: Into<String>>(&self, label: S);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{AppState, create_admin_router, create_api_router};

    fn set_env(key: &str, value: &str) {
        // SAFETY: test-only helper; env vars are manipulated in single-threaded test context.
        unsafe { std::env::set_var(key, value) };
    }
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use clarity_core::agent::{Agent, AgentConfig, MockLlm};
    use clarity_core::background::BackgroundTaskManager;
    use clarity_core::registry::ToolRegistry;
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::util::ServiceExt;

    pub(crate) async fn test_state() -> Arc<AppState> {
        let registry = ToolRegistry::with_builtin_tools();
        let config = AgentConfig::new()
            .with_max_iterations(5)
            .with_read_only(false);
        let agent = Arc::new(Agent::with_config(registry, config).with_llm(Arc::new(MockLlm)));

        let temp =
            std::env::temp_dir().join(format!("clarity-gateway-test-{}", std::process::id()));
        let _ = std::fs::create_dir_all(&temp);
        let task_manager = Arc::new(BackgroundTaskManager::new(
            temp.join("store"),
            temp.join("work"),
            temp.join("context"),
        ));

        Arc::new(
            AppState::new_with_home(agent, task_manager, temp.join(".clarity"))
                .await
                .unwrap(),
        )
    }

    // ==================== Security tests (preserved) ====================

    #[test]
    fn test_sanitize_path_rejects_parent_traversal() {
        let result = files::sanitize_path("../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_sanitize_path_rejects_deep_traversal() {
        let result = files::sanitize_path("src/../../../../etc/passwd");
        assert!(result.is_err());
    }

    #[test]
    fn test_sanitize_path_allows_relative() {
        let result = files::sanitize_path("src/main.rs");
        assert!(result.is_ok());
    }

    // ==================== Handler integration tests ====================

    #[tokio::test]
    async fn test_health_check() {
        let state = test_state().await;
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
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "healthy");
        assert!(json.get("version").is_some());
        assert!(json.get("timestamp").is_some());
    }

    #[tokio::test]
    async fn test_file_tree() {
        let state = test_state().await;
        let app = create_api_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/files/tree")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("tree").is_some());
    }

    #[tokio::test]
    async fn test_file_read() {
        let state = test_state().await;
        let app = create_api_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/files/read?path=Cargo.toml")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // file_read uses the built-in tool; it may succeed or error depending
        // on the working directory, but it should not panic / hang.
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            json.get("content").is_some() || json.get("error").is_some(),
            "expected content or error key, got: {}",
            json
        );
    }

    #[tokio::test]
    async fn test_admin_tools() {
        let state = test_state().await;
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
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("tools").is_some());
        assert!(json["tools"].is_array());
    }

    #[tokio::test]
    async fn test_admin_approval_mode_get_and_set() {
        let state = test_state().await;
        let app = create_admin_router(state);

        // 1. Get initial mode
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/approval-mode")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["mode"].is_string());

        // 2. Set to yolo
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/approval-mode")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"mode":"yolo"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["mode"], "yolo");

        // 3. Verify persisted
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/approval-mode")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["mode"], "yolo");
    }

    #[tokio::test]
    async fn test_admin_approval_mode_rejects_invalid() {
        let state = test_state().await;
        let app = create_admin_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/approval-mode")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"mode":"invalid"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_admin_mesh_status_inactive() {
        let state = test_state().await;
        let app = create_admin_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/mesh")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["active"], false);
        assert!(json["providers"].as_array().unwrap().is_empty());
        assert!(json["circuits"].as_object().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_admin_switch_provider_invalid_single() {
        let state = test_state().await;
        let app = create_admin_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/provider")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"provider":"this_provider_does_not_exist_12345"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            json["message"]
                .as_str()
                .unwrap()
                .contains("Failed to create provider")
        );
    }

    #[tokio::test]
    async fn test_admin_switch_provider_mcp_invalid_command() {
        let state = test_state().await;
        let app = create_admin_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/provider")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"provider":"mcp:this_command_does_not_exist_12345"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            json["message"]
                .as_str()
                .unwrap()
                .contains("Failed to connect MCP LLM")
        );
    }

    #[tokio::test]
    async fn test_admin_switch_provider_mesh_invalid() {
        // Temporarily override mesh env with invalid providers
        let _guard = std::env::var("CLARITY_MESH_PROVIDERS");
        set_env(
            "CLARITY_MESH_PROVIDERS",
            "invalid_provider_1,invalid_provider_2",
        );

        let state = test_state().await;
        let app = create_admin_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/provider")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"provider":"mesh"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(
            json["message"]
                .as_str()
                .unwrap()
                .contains("Failed to create mesh")
        );
    }

    #[tokio::test]
    async fn test_admin_config_health_returns_report() {
        let state = test_state().await;
        let app = create_admin_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/config/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("healthy").is_some());
        assert!(json.get("layers").is_some());
        assert!(json.get("issues").is_some());
    }

    #[tokio::test]
    async fn test_admin_config_validate_returns_result() {
        let state = test_state().await;
        let app = create_admin_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/config/validate")
                    .method("POST")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        // The response status depends on whether the current config is healthy.
        assert!(
            response.status() == StatusCode::OK
                || response.status() == StatusCode::UNPROCESSABLE_ENTITY,
            "unexpected status: {:?}",
            response.status()
        );
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.get("healthy").is_some());
        assert!(json.get("issue_count").is_some());
        assert!(json.get("issues").is_some());
    }
}
