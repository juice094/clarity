use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;

use std::sync::Arc;

use crate::server::AppState;

// ==================== Admin: Session Management ====================

pub(crate) async fn list_sessions(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match state.session_store.list_sessions().await {
        Ok(sessions) => (StatusCode::OK, Json(json!({ "sessions": sessions }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

pub(crate) async fn get_session(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match state.session_store.load_session(&session_id).await {
        Ok(messages) => {
            let msgs: Vec<_> = messages
                .into_iter()
                .map(|m| {
                    json!({
                        "role": m.role,
                        "content": m.content,
                        "tool_calls": m.tool_calls,
                        "tool_call_id": m.tool_call_id,
                        "created_at": m.created_at.to_rfc3339(),
                    })
                })
                .collect();
            (
                StatusCode::OK,
                Json(json!({ "session_id": session_id, "messages": msgs })),
            )
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}

pub(crate) async fn delete_session(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(session_id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match state.session_store.delete_session(&session_id).await {
        Ok(true) => (StatusCode::OK, Json(json!({ "deleted": true }))),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "error": "Session not found" })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": e.to_string() })),
        ),
    }
}
