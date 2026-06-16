//! Gateway V2 thread API integration tests.
//!
//! These tests build the Axum API router directly and exercise the
//! `/api/v2/threads` endpoints end-to-end without starting a real TCP server.

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
async fn test_thread_create_and_list() {
    let state = test_state().await;
    let app = create_api_router(state);

    let create = Request::builder()
        .method("POST")
        .uri("/api/v2/threads")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "title": "integration test thread" }).to_string(),
        ))
        .unwrap();

    let res = app.clone().oneshot(create).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = response_json(res).await;
    let thread_id = body["thread_id"].as_str().unwrap();

    let list = Request::builder()
        .method("GET")
        .uri("/api/v2/threads?limit=10")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(list).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = response_json(res).await;
    let threads = body["data"].as_array().unwrap();
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0]["thread_id"], thread_id);
    assert_eq!(threads[0]["title"], "integration test thread");
}

#[tokio::test]
async fn test_thread_chat_non_streaming_persists_turn() {
    let state = test_state().await;
    let app = create_api_router(state.clone());

    // Create a thread.
    let create = Request::builder()
        .method("POST")
        .uri("/api/v2/threads")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "title": "chat test" }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(create).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = response_json(res).await;
    let thread_id = body["thread_id"].as_str().unwrap();

    // Send a non-streaming chat request.
    let chat = Request::builder()
        .method("POST")
        .uri(format!("/api/v2/threads/{}/chat", thread_id))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "model": "default",
                "messages": [{"role": "user", "content": "hello"}],
                "stream": false
            })
            .to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(chat).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = response_json(res).await;
    let content = body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap_or_else(|| panic!("unexpected chat response: {body}",));
    assert!(content.contains("mock response"));

    // Verify history now contains user + assistant messages.
    let history = Request::builder()
        .method("GET")
        .uri(format!(
            "/api/v2/threads/{}?include_history=true",
            thread_id
        ))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(history).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = response_json(res).await;
    let items = body["history"].as_array().unwrap();
    let roles: Vec<&str> = items
        .iter()
        .filter_map(|item| {
            if item["type"] == "response_item" && item["payload"]["type"] == "message" {
                item["payload"]["data"]["role"].as_str()
            } else {
                None
            }
        })
        .collect();
    assert!(roles.contains(&"user"));
    assert!(roles.contains(&"assistant"));
}

#[tokio::test]
async fn test_thread_chat_invalid_thread_returns_404() {
    let state = test_state().await;
    let app = create_api_router(state);

    let chat = Request::builder()
        .method("POST")
        .uri("/api/v2/threads/00000000-0000-0000-0000-000000000000/chat")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "model": "default",
                "messages": [{"role": "user", "content": "hello"}],
                "stream": false
            })
            .to_string(),
        ))
        .unwrap();
    let res = app.oneshot(chat).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_thread_fork() {
    let state = test_state().await;
    let app = create_api_router(state.clone());

    let create = Request::builder()
        .method("POST")
        .uri("/api/v2/threads")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "title": "fork source" }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(create).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = response_json(res).await;
    let source_id = body["thread_id"].as_str().unwrap();

    let fork = Request::builder()
        .method("POST")
        .uri(format!("/api/v2/threads/{}/fork", source_id))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "snapshot": { "kind": "interrupted" } }).to_string(),
        ))
        .unwrap();
    let res = app.oneshot(fork).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = response_json(res).await;
    let new_id = body["thread_id"].as_str().unwrap();
    assert_ne!(new_id, source_id);
}

#[tokio::test]
async fn test_thread_update_metadata() {
    let state = test_state().await;
    let app = create_api_router(state.clone());

    let create = Request::builder()
        .method("POST")
        .uri("/api/v2/threads")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "title": "before update" }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(create).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = response_json(res).await;
    let thread_id = body["thread_id"].as_str().unwrap();

    let update = Request::builder()
        .method("PATCH")
        .uri(format!("/api/v2/threads/{}", thread_id))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "title": "updated title", "archived": true }).to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(update).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = response_json(res).await;
    assert_eq!(body["title"], "updated title");
    assert_eq!(body["archived"], true);

    let get = Request::builder()
        .method("GET")
        .uri(format!("/api/v2/threads/{}", thread_id))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(get).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = response_json(res).await;
    assert_eq!(body["title"], "updated title");
    assert_eq!(body["archived"], true);
}

#[tokio::test]
async fn test_thread_archive_unarchive_and_list() {
    let state = test_state().await;
    let app = create_api_router(state.clone());

    async fn create_thread(app: &axum::Router, title: &str) -> String {
        let create = Request::builder()
            .method("POST")
            .uri("/api/v2/threads")
            .header("content-type", "application/json")
            .body(Body::from(json!({ "title": title }).to_string()))
            .unwrap();
        let res = app.clone().oneshot(create).await.unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);
        let body = response_json(res).await;
        body["thread_id"].as_str().unwrap().to_string()
    }

    let active_id = create_thread(&app, "active thread").await;
    let archived_id = create_thread(&app, "archived thread").await;

    let archive = Request::builder()
        .method("POST")
        .uri(format!("/api/v2/threads/{}/archive", archived_id))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(archive).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let list = Request::builder()
        .method("GET")
        .uri("/api/v2/threads?include_archived=false")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(list).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = response_json(res).await;
    let threads = body["data"].as_array().unwrap();
    assert_eq!(threads.len(), 1);
    assert_eq!(threads[0]["thread_id"], active_id);

    let list = Request::builder()
        .method("GET")
        .uri("/api/v2/threads?include_archived=true")
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(list).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = response_json(res).await;
    let threads = body["data"].as_array().unwrap();
    assert_eq!(threads.len(), 2);

    let unarchive = Request::builder()
        .method("POST")
        .uri(format!("/api/v2/threads/{}/unarchive", archived_id))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(unarchive).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = response_json(res).await;
    assert_eq!(body["archived"], false);

    let list = Request::builder()
        .method("GET")
        .uri("/api/v2/threads?include_archived=false")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(list).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = response_json(res).await;
    let threads = body["data"].as_array().unwrap();
    assert_eq!(threads.len(), 2);
}

#[tokio::test]
async fn test_thread_delete() {
    let state = test_state().await;
    let app = create_api_router(state.clone());

    let create = Request::builder()
        .method("POST")
        .uri("/api/v2/threads")
        .header("content-type", "application/json")
        .body(Body::from(json!({ "title": "to delete" }).to_string()))
        .unwrap();
    let res = app.clone().oneshot(create).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = response_json(res).await;
    let thread_id = body["thread_id"].as_str().unwrap();

    let delete = Request::builder()
        .method("DELETE")
        .uri(format!("/api/v2/threads/{}", thread_id))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(delete).await.unwrap();
    assert_eq!(res.status(), StatusCode::NO_CONTENT);

    let get = Request::builder()
        .method("GET")
        .uri(format!("/api/v2/threads/{}", thread_id))
        .body(Body::empty())
        .unwrap();
    let res = app.clone().oneshot(get).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);

    let list = Request::builder()
        .method("GET")
        .uri("/api/v2/threads")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(list).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = response_json(res).await;
    let threads = body["data"].as_array().unwrap();
    assert!(threads.is_empty());
}

#[tokio::test]
async fn test_thread_fork_truncate() {
    let state = test_state().await;
    let app = create_api_router(state.clone());

    let create = Request::builder()
        .method("POST")
        .uri("/api/v2/threads")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "title": "fork truncate source" }).to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(create).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = response_json(res).await;
    let thread_id = body["thread_id"].as_str().unwrap();

    let chat = Request::builder()
        .method("POST")
        .uri(format!("/api/v2/threads/{}/chat", thread_id))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({
                "model": "default",
                "messages": [{"role": "user", "content": "hello"}],
                "stream": false
            })
            .to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(chat).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let fork = Request::builder()
        .method("POST")
        .uri(format!("/api/v2/threads/{}/fork", thread_id))
        .header("content-type", "application/json")
        .body(Body::from(
            json!({ "snapshot": { "kind": "truncate_before_nth_user_message", "n": 1 } })
                .to_string(),
        ))
        .unwrap();
    let res = app.clone().oneshot(fork).await.unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let body = response_json(res).await;
    let new_id = body["thread_id"].as_str().unwrap();
    assert_ne!(new_id, thread_id);

    let get = Request::builder()
        .method("GET")
        .uri(format!("/api/v2/threads/{}?include_history=true", new_id))
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(get).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
    let body = response_json(res).await;
    let history = body["history"].as_array().unwrap();
    assert!(!history.is_empty());
    assert!(
        history.iter().all(|item| item["type"] == "session_meta"),
        "expected only session_meta items, got {history:?}"
    );
    assert!(
        !history.iter().any(|item| {
            item["type"] == "response_item"
                && ["user", "assistant"]
                    .contains(&item["payload"]["data"]["role"].as_str().unwrap_or(""))
        }),
        "expected no user/assistant messages after truncate, got {history:?}"
    );
}

#[tokio::test]
async fn test_thread_archive_invalid_id_returns_404() {
    let state = test_state().await;
    let app = create_api_router(state);

    let archive = Request::builder()
        .method("POST")
        .uri("/api/v2/threads/00000000-0000-0000-0000-000000000000/archive")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(archive).await.unwrap();
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_thread_delete_invalid_id_returns_400() {
    let state = test_state().await;
    let app = create_api_router(state);

    let delete = Request::builder()
        .method("DELETE")
        .uri("/api/v2/threads/not-a-uuid")
        .body(Body::empty())
        .unwrap();
    let res = app.oneshot(delete).await.unwrap();
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
}
