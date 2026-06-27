//! Claw Mesh role-context sync handler.
//!
//! Exposes a lightweight HTTP endpoint that Claw daemons poll to synchronise
//! their local view of a shared *role context* (a stream of events keyed by
//! `role_id`). The handler delegates to the Gateway's in-process
//! `RoleContextStore`, which is also the backing store for the WebSocket
//! `SyncRoleContext` flow.

use crate::server::AppState;
use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

// ── Request / response types ─────────────────────────────────────────────────

/// Payload for `POST /api/v1/claw/sync`.
#[derive(Debug, Deserialize)]
pub struct SyncRoleContextRequest {
    /// Role to synchronise (e.g. `"operator"`).
    pub role_id: String,
    /// Last event id known to the requesting client. Only events *after* this
    /// id are returned.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub since_event_id: Option<String>,
    /// Device id of the requesting Claw instance (used for presence tracking).
    pub device_id: String,
}

/// Response for `POST /api/v1/claw/sync`.
#[derive(Debug, Serialize)]
pub struct SyncRoleContextResponse {
    /// Role that was synchronised.
    pub role_id: String,
    /// Events that the client is missing.
    pub events: Vec<clarity_contract::ClawContextEvent>,
    /// Cursor for deduplication on the next request (the last returned
    /// `event_id`, if any).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
    /// Device ids currently online for this role.
    pub online_devices: Vec<String>,
    /// Server timestamp (useful for debugging clock skew).
    pub server_ts: String,
}

// ── Handler ──────────────────────────────────────────────────────────────────

/// `POST /api/v1/claw/sync` — synchronise a role context.
pub async fn sync_role_context(
    State(state): State<Arc<AppState>>,
    Json(payload): Json<SyncRoleContextRequest>,
) -> Result<Json<SyncRoleContextResponse>, StatusCode> {
    if payload.role_id.is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }

    // Record device presence so MeshClient peers can see who is online.
    if !payload.device_id.is_empty()
        && let Err(e) = state
            .role_context_store
            .record_device_presence(&payload.role_id, &payload.device_id)
            .await
    {
        tracing::warn!(error = %e, "claw_sync: failed to record device presence");
    }

    let events = state
        .role_context_store
        .list_events(&payload.role_id, payload.since_event_id.as_deref())
        .await
        .map_err(|e| {
            tracing::warn!(error = %e, "claw_sync: failed to list events");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    let online_devices = state
        .role_context_store
        .online_devices(&payload.role_id)
        .await
        .unwrap_or_default();

    let next_cursor = events.last().map(|e| e.event_id.clone());

    Ok(Json(SyncRoleContextResponse {
        role_id: payload.role_id,
        events,
        next_cursor,
        online_devices,
        server_ts: Utc::now().to_rfc3339(),
    }))
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use clarity_contract::{ClawContextEvent, ContextEventKind};
    use clarity_core::agent::{Agent, AgentConfig, MockLlm};
    use clarity_core::background::BackgroundTaskManager;
    use clarity_core::registry::ToolRegistry;
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::util::ServiceExt;

    async fn test_state() -> Arc<AppState> {
        let registry = ToolRegistry::with_builtin_tools();
        let config = AgentConfig::new()
            .with_max_iterations(2)
            .with_read_only(false);
        let agent = Arc::new(Agent::with_config(registry, config).with_llm(Arc::new(MockLlm)));

        let temp = std::env::temp_dir().join(format!("claw-sync-test-{}", std::process::id()));
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

    fn sync_router(state: Arc<AppState>) -> axum::Router {
        axum::Router::new()
            .route("/api/v1/claw/sync", axum::routing::post(sync_role_context))
            .with_state(state)
    }

    #[tokio::test]
    async fn test_sync_empty_role() {
        let state = test_state().await;
        let app = sync_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/claw/sync")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"role_id":"operator","device_id":"dev-1"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["role_id"], "operator");
        assert!(json["events"].as_array().unwrap().is_empty());
        assert_eq!(json["next_cursor"], serde_json::Value::Null);
    }

    #[tokio::test]
    async fn test_sync_with_events() {
        let state = test_state().await;

        // Pre-seed an event.
        let event = ClawContextEvent {
            event_id: "ev-1".into(),
            origin_device: "dev-a".into(),
            origin_clock: 1,
            kind: ContextEventKind::AppendMessage {
                role: "user".into(),
                content: "hello from claw".into(),
            },
        };
        state
            .role_context_store
            .append_event("bot-role", &event)
            .await
            .unwrap();

        let app = sync_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/claw/sync")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"role_id":"bot-role","device_id":"claw-dev"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["role_id"], "bot-role");
        let events = json["events"].as_array().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["event_id"], "ev-1");
        assert_eq!(json["next_cursor"], "ev-1");
        assert!(json.get("server_ts").is_some());
    }

    #[tokio::test]
    async fn test_sync_since_cursor() {
        let state = test_state().await;

        let e1 = ClawContextEvent {
            event_id: "ev-1".into(),
            origin_device: "dev-a".into(),
            origin_clock: 1,
            kind: ContextEventKind::AppendMessage {
                role: "user".into(),
                content: "first".into(),
            },
        };
        let e2 = ClawContextEvent {
            event_id: "ev-2".into(),
            origin_device: "dev-b".into(),
            origin_clock: 2,
            kind: ContextEventKind::AppendMessage {
                role: "assistant".into(),
                content: "second".into(),
            },
        };
        state
            .role_context_store
            .append_event("role-x", &e1)
            .await
            .unwrap();
        state
            .role_context_store
            .append_event("role-x", &e2)
            .await
            .unwrap();

        let app = sync_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/claw/sync")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        r#"{"role_id":"role-x","since_event_id":"ev-1","device_id":"dev-c"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let events = json["events"].as_array().unwrap();
        assert_eq!(
            events.len(),
            1,
            "only the event after ev-1 should be returned"
        );
        assert_eq!(events[0]["event_id"], "ev-2");
    }

    #[tokio::test]
    async fn test_sync_missing_role_id_is_rejected() {
        let state = test_state().await;
        let app = sync_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/claw/sync")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"role_id":"","device_id":"dev-1"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_sync_records_device_presence() {
        let state = test_state().await;

        let app = sync_router(state.clone());

        let _ = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/claw/sync")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"role_id":"op","device_id":"dev-presence"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        let online = state.role_context_store.online_devices("op").await.unwrap();
        assert!(online.contains(&"dev-presence".to_string()));
    }
}
