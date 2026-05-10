

pub mod chat;
pub mod admin;
pub mod tasks;
pub mod config;
pub mod files;
pub mod sessions;
pub mod mcp;
pub mod cron;
pub mod memory;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::{create_admin_router, create_api_router, AppState};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use clarity_core::agent::{Agent, AgentConfig, MockLlm};
    use clarity_core::background::BackgroundTaskManager;
    use clarity_core::registry::ToolRegistry;
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::util::ServiceExt;

    async fn test_state() -> Arc<AppState> {
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

        Arc::new(AppState::new(agent, task_manager).await)
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
                    .body(Body::from(r#"{"provider":"this_provider_does_not_exist_12345"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["message"].as_str().unwrap().contains("Failed to create provider"));
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
                    .body(Body::from(r#"{"provider":"mcp:this_command_does_not_exist_12345"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json["message"].as_str().unwrap().contains("Failed to connect MCP LLM"));
    }

    #[tokio::test]
    async fn test_admin_switch_provider_mesh_invalid() {
        // Temporarily override mesh env with invalid providers
        let _guard = std::env::var("CLARITY_MESH_PROVIDERS");
        std::env::set_var("CLARITY_MESH_PROVIDERS", "invalid_provider_1,invalid_provider_2");

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
        assert!(json["message"].as_str().unwrap().contains("Failed to create mesh"));
    }
}

