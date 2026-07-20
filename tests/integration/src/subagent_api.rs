//! Gateway subagent REST API integration tests.
//!
//! These tests build the Axum API router directly and exercise the
//! `/api/v1/subagents/*` endpoints end-to-end without starting a real TCP server.

use std::sync::Arc;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use clarity_core::agent::{Agent, AgentConfig, MockLlm};
use clarity_core::background::BackgroundTaskManager;
use clarity_core::registry::ToolRegistry;
use clarity_gateway::server::{AppState, create_api_router};
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

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

async fn response_json(res: axum::response::Response) -> serde_json::Value {
    let status = res.status();
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_else(|_| {
        json!({
            "status": status.as_u16(),
            "raw": String::from_utf8_lossy(&bytes).to_string(),
        })
    });
    if let Some(obj) = value.as_object_mut() {
        obj.insert("_status".to_string(), json!(status.as_u16()));
    }
    value
}

#[tokio::test]
async fn test_subagent_types_lists_builtins() {
    let state = test_state().await;
    let app = create_api_router(state);

    let req = Request::builder()
        .method("GET")
        .uri("/api/v1/subagents/types")
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = response_json(res).await;
    let types = body["types"].as_array().unwrap();
    assert!(!types.is_empty());
    assert!(types.iter().any(|t| t["name"] == "coder"));
    assert!(types.iter().any(|t| t["name"] == "review"));
}

#[tokio::test]
async fn test_subagent_run_success() {
    let state = test_state().await;
    let app = create_api_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/subagents/run")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "description": "integration-test",
                "agent_type": "coder",
                "prompt": "Say hello",
                "max_iterations": 1
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = response_json(res).await;
    assert_eq!(body["agent_type"], "coder");
    assert!(!body["agent_id"].as_str().unwrap().is_empty());
    assert_eq!(body["status"], "completed");
}

#[tokio::test]
async fn test_subagent_run_rejects_empty_prompt() {
    let state = test_state().await;
    let app = create_api_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/subagents/run")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "description": "integration-test",
                "agent_type": "coder",
                "prompt": ""
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = response_json(res).await;
    assert!(body["error"].as_str().is_some());
}

#[tokio::test]
async fn test_subagent_run_parallel_success() {
    let state = test_state().await;
    let app = create_api_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/subagents/run/parallel")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "tasks": [
                    {"description": "t1", "agent_type": "coder", "prompt": "p1", "max_iterations": 1},
                    {"description": "t2", "agent_type": "review", "prompt": "p2", "max_iterations": 1}
                ],
                "max_concurrency": 2,
                "cancel_on_error": false
            })
            .to_string(),
        ))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = response_json(res).await;
    assert_eq!(body["results"].as_array().unwrap().len(), 2);
    assert_eq!(body["success_rate"], 1.0);
    assert!(body["total_elapsed_ms"].as_u64().is_some());
}

#[tokio::test]
async fn test_subagent_run_parallel_rejects_empty_tasks() {
    let state = test_state().await;
    let app = create_api_router(state);

    let req = Request::builder()
        .method("POST")
        .uri("/api/v1/subagents/run/parallel")
        .header("content-type", "application/json")
        .body(Body::from(json!({"tasks": []}).to_string()))
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let body = response_json(res).await;
    assert_eq!(body["error"], "No tasks provided");
}
