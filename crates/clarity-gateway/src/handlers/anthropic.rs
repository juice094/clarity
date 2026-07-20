//! Anthropic Messages API-compatible handler.
//!
//! This handler exposes `POST /v1/messages` and delegates request/response
//! conversion to `clarity_llm::anthropic::AnthropicAdapter`. The underlying
//! LLM is the same provider currently configured for the Gateway agent,
//! making this a protocol facade rather than a direct Anthropic API client.
//!
//! Enabled by the `anthropic-api` feature.

use axum::{Json, extract::State, http::StatusCode, response::IntoResponse};
use clarity_llm::anthropic::{AnthropicAdapter, AnthropicRequest};
use serde_json::json;
use std::sync::Arc;
use tracing::warn;

use crate::server::AppState;

/// Handle `POST /v1/messages` in Anthropic Messages API format.
///
/// The request is parsed into an `AnthropicRequest`, forwarded through
/// `AnthropicAdapter` wrapping the Gateway's active LLM provider, and the
/// resulting `AnthropicResponse` is serialized back to JSON.
pub async fn messages(State(state): State<Arc<AppState>>, body: String) -> impl IntoResponse {
    let req: AnthropicRequest = match serde_json::from_str(&body) {
        Ok(r) => r,
        Err(e) => {
            warn!("Anthropic request deserialization error: {e}");
            return (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({
                    "error": {
                        "type": "invalid_request_error",
                        "message": format!("deserialization: {e}")
                    }
                })),
            )
                .into_response();
        }
    };

    let provider = match state.agent.llm() {
        Some(p) => p,
        None => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(json!({
                    "error": {
                        "type": "api_error",
                        "message": "No LLM provider configured"
                    }
                })),
            )
                .into_response();
        }
    };

    let adapter = AnthropicAdapter::new(provider);
    match adapter.complete(req).await {
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(e) => {
            warn!("Anthropic adapter completion error: {e}");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "error": {
                        "type": "api_error",
                        "message": "Internal server error"
                    }
                })),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use clarity_contract::LlmProvider;
    use clarity_core::agent::{Agent, AgentConfig, MockLlm};
    use clarity_core::background::BackgroundTaskManager;
    use clarity_core::registry::ToolRegistry;
    use http_body_util::BodyExt;
    use serde_json::Value;
    use std::sync::Arc;
    use tower::util::ServiceExt;

    use crate::server::{AppState, create_api_router};

    async fn test_state_with_mock() -> Arc<AppState> {
        let registry = ToolRegistry::with_builtin_tools();
        let config = AgentConfig::new()
            .with_max_iterations(5)
            .with_read_only(false);
        let agent = Arc::new(
            Agent::with_config(registry, config)
                .with_llm(Arc::new(MockLlm) as Arc<dyn LlmProvider>),
        );

        let temp = std::env::temp_dir().join(format!(
            "clarity-gateway-anthropic-test-{}",
            std::process::id()
        ));
        let _ = std::fs::create_dir_all(&temp);
        let task_manager = Arc::new(BackgroundTaskManager::new(
            temp.join("store"),
            temp.join("work"),
            temp.join("context"),
        ));

        Arc::new(
            AppState::new_with_home(agent, task_manager, temp)
                .await
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn messages_endpoint_returns_assistant_role() {
        let state = test_state_with_mock().await;
        let app = create_api_router(state);

        let body = json!({
            "model": "claude-test",
            "max_tokens": 1024,
            "messages": [{"role": "user", "content": "Hello"}]
        })
        .to_string();

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/messages")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body_json: Value = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(body_json["role"], "assistant");
        assert_eq!(body_json["type"], "message");
    }

    #[tokio::test]
    async fn messages_endpoint_rejects_invalid_json() {
        let state = test_state_with_mock().await;
        let app = create_api_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/messages")
                    .header("content-type", "application/json")
                    .body(Body::from("not json"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }
}
