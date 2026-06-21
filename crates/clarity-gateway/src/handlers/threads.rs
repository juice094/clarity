//! Thread management handlers — `/api/v2/threads`.
//!
//! These endpoints expose the `clarity-thread-store` abstraction to HTTP clients
//! and to Claw for tray monitoring.

use axum::{
    extract::{Json, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use chrono::{DateTime, Utc};
use clarity_contract::{RolloutItem, SessionSource, ThreadId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

use crate::server::AppState;

/// Request body for creating a thread.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateThreadRequest {
    /// Optional human-readable title.
    #[serde(default)]
    pub title: Option<String>,
    /// Runtime source that created the thread.
    #[serde(default)]
    pub source: Option<SessionSource>,
}

/// Request body for updating a thread.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateThreadRequest {
    /// New title, if set.
    #[serde(default)]
    pub title: Option<String>,
    /// New archived state, if set.
    #[serde(default)]
    pub archived: Option<bool>,
    /// Arbitrary extra metadata fields.
    #[serde(default)]
    pub extra: Option<HashMap<String, Value>>,
}

/// Request body for forking a thread.
#[derive(Debug, Clone, Deserialize)]
pub struct ForkThreadRequest {
    /// Snapshot mode. Defaults to current full history (`Interrupted`).
    #[serde(default)]
    pub snapshot: Option<ForkSnapshotBody>,
}

/// JSON-friendly fork snapshot mode.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ForkSnapshotBody {
    /// Fork a prefix ending strictly before the nth user message.
    TruncateBeforeNthUserMessage {
        /// 1-based user message index.
        n: usize,
    },
    /// Fork as if the source thread had been interrupted now.
    Interrupted,
}

impl From<ForkSnapshotBody> for clarity_thread_store::ForkSnapshot {
    fn from(body: ForkSnapshotBody) -> Self {
        match body {
            ForkSnapshotBody::TruncateBeforeNthUserMessage { n } => {
                Self::TruncateBeforeNthUserMessage(n)
            }
            ForkSnapshotBody::Interrupted => Self::Interrupted,
        }
    }
}

/// Query parameters for listing threads.
#[derive(Debug, Clone, Deserialize)]
pub struct ListThreadsQuery {
    /// Maximum number of threads to return.
    #[serde(default = "default_limit")]
    pub limit: usize,
    /// Whether to include archived threads.
    #[serde(default)]
    pub include_archived: bool,
    /// Opaque cursor from a previous page.
    #[serde(default)]
    pub cursor: Option<String>,
}

fn default_limit() -> usize {
    100
}

/// Query parameters for reading a single thread.
#[derive(Debug, Clone, Deserialize)]
pub struct GetThreadQuery {
    /// Include the full rollout history in the response.
    #[serde(default)]
    pub include_history: bool,
}

/// Thread summary returned in list responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadListItem {
    /// Thread identifier.
    pub thread_id: String,
    /// Session identifier.
    pub session_id: String,
    /// Human-readable title.
    pub title: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Whether the thread is archived.
    pub archived: bool,
    /// Parent thread identifier, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    /// Fork source thread identifier, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forked_from_id: Option<String>,
}

/// Paginated thread list response.
#[derive(Debug, Clone, Serialize)]
pub struct ThreadListResponse {
    /// Thread summaries for this page.
    pub data: Vec<ThreadListItem>,
    /// Cursor for the next page, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_cursor: Option<String>,
}

/// Single thread response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadResponse {
    /// Thread identifier.
    pub thread_id: String,
    /// Session identifier.
    pub session_id: String,
    /// Human-readable title.
    pub title: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
    /// Whether the thread is archived.
    pub archived: bool,
    /// Parent thread identifier, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_thread_id: Option<String>,
    /// Fork source thread identifier, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forked_from_id: Option<String>,
    /// Rollout file path, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollout_path: Option<std::path::PathBuf>,
    /// Full rollout history, if requested.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub history: Option<Vec<RolloutItem>>,
}

#[allow(clippy::result_large_err)]
fn parse_thread_id(id: &str) -> Result<ThreadId, axum::response::Response> {
    ThreadId::from_string(id).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": format!("invalid thread id: {e}") })),
        )
            .into_response()
    })
}

fn thread_not_found(id: &str) -> axum::response::Response {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({ "error": format!("thread not found: {id}") })),
    )
        .into_response()
}

fn store_error(e: impl std::fmt::Display) -> axum::response::Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Json(serde_json::json!({ "error": e.to_string() })),
    )
        .into_response()
}

pub(crate) fn summary_to_item(summary: &clarity_thread_store::ThreadSummary) -> ThreadListItem {
    ThreadListItem {
        thread_id: summary.thread_id.to_string(),
        session_id: summary.session_id.to_string(),
        title: summary.title.clone(),
        created_at: summary.created_at,
        updated_at: summary.updated_at,
        archived: summary.archived,
        parent_thread_id: summary.parent_thread_id.map(|id| id.to_string()),
        forked_from_id: summary.forked_from_id.map(|id| id.to_string()),
    }
}

pub(crate) fn stored_to_response(stored: &clarity_thread_store::StoredThread) -> ThreadResponse {
    ThreadResponse {
        thread_id: stored.thread_id.to_string(),
        session_id: stored.session_id.to_string(),
        title: stored.title.clone(),
        created_at: stored.created_at,
        updated_at: stored.updated_at,
        archived: stored.archived,
        parent_thread_id: stored.parent_thread_id.map(|id| id.to_string()),
        forked_from_id: stored.forked_from_id.map(|id| id.to_string()),
        rollout_path: stored.rollout_path.clone(),
        history: stored.history.clone().map(|h| h.items),
    }
}

/// Create a new thread.
pub async fn create_thread(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateThreadRequest>,
) -> axum::response::Response {
    let cwd = state.agent.config().working_dir.clone();
    let source = req.source.unwrap_or(SessionSource::AppServer);
    match state
        .thread_manager
        .create_thread(&cwd, "clarity-gateway", source)
        .await
    {
        Ok(thread_id) => {
            if let Some(title) = req.title {
                let _ = state
                    .thread_manager
                    .update_metadata(
                        thread_id,
                        clarity_thread_store::ThreadMetadataPatch {
                            title: Some(title),
                            archived: None,
                            extra: HashMap::new(),
                        },
                    )
                    .await;
            }
            (
                StatusCode::CREATED,
                Json(serde_json::json!({ "thread_id": thread_id.to_string() })),
            )
                .into_response()
        }
        Err(e) => store_error(e),
    }
}

/// List threads with optional pagination.
pub async fn list_threads(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ListThreadsQuery>,
) -> axum::response::Response {
    match state
        .thread_manager
        .list_threads(query.limit, query.include_archived, query.cursor)
        .await
    {
        Ok(summaries) => {
            let data: Vec<_> = summaries.iter().map(summary_to_item).collect();
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "data": data,
                    "next_cursor": None::<String>
                })),
            )
                .into_response()
        }
        Err(e) => store_error(e),
    }
}

/// Read a single thread.
pub async fn get_thread(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<GetThreadQuery>,
) -> axum::response::Response {
    let thread_id = match parse_thread_id(&id) {
        Ok(t) => t,
        Err(resp) => return resp,
    };

    match state
        .thread_manager
        .read_thread(thread_id, query.include_history)
        .await
    {
        Ok(stored) => (
            StatusCode::OK,
            Json(serde_json::json!(stored_to_response(&stored))),
        )
            .into_response(),
        Err(clarity_core::thread::ThreadManagerError::ThreadStore(
            clarity_thread_store::ThreadStoreError::NotFound { .. },
        )) => thread_not_found(&id),
        Err(e) => store_error(e),
    }
}

/// Update thread metadata.
pub async fn update_thread(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<UpdateThreadRequest>,
) -> axum::response::Response {
    let thread_id = match parse_thread_id(&id) {
        Ok(t) => t,
        Err(resp) => return resp,
    };

    let patch = clarity_thread_store::ThreadMetadataPatch {
        title: req.title,
        archived: req.archived,
        extra: req.extra.unwrap_or_default(),
    };

    match state.thread_manager.update_metadata(thread_id, patch).await {
        Ok(stored) => (
            StatusCode::OK,
            Json(serde_json::json!(stored_to_response(&stored))),
        )
            .into_response(),
        Err(clarity_core::thread::ThreadManagerError::ThreadStore(
            clarity_thread_store::ThreadStoreError::NotFound { .. },
        )) => thread_not_found(&id),
        Err(e) => store_error(e),
    }
}

/// Archive a thread.
pub async fn archive_thread(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> axum::response::Response {
    let thread_id = match parse_thread_id(&id) {
        Ok(t) => t,
        Err(resp) => return resp,
    };

    match state.thread_manager.archive(thread_id).await {
        Ok(()) => (StatusCode::NO_CONTENT, Json(serde_json::json!({}))).into_response(),
        Err(clarity_core::thread::ThreadManagerError::ThreadStore(
            clarity_thread_store::ThreadStoreError::NotFound { .. },
        )) => thread_not_found(&id),
        Err(e) => store_error(e),
    }
}

/// Unarchive a thread.
pub async fn unarchive_thread(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> axum::response::Response {
    let thread_id = match parse_thread_id(&id) {
        Ok(t) => t,
        Err(resp) => return resp,
    };

    match state.thread_manager.unarchive(thread_id).await {
        Ok(stored) => (
            StatusCode::OK,
            Json(serde_json::json!(stored_to_response(&stored))),
        )
            .into_response(),
        Err(clarity_core::thread::ThreadManagerError::ThreadStore(
            clarity_thread_store::ThreadStoreError::NotFound { .. },
        )) => thread_not_found(&id),
        Err(e) => store_error(e),
    }
}

/// Delete a thread.
pub async fn delete_thread(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> axum::response::Response {
    let thread_id = match parse_thread_id(&id) {
        Ok(t) => t,
        Err(resp) => return resp,
    };

    match state.thread_manager.delete(thread_id).await {
        Ok(()) => (StatusCode::NO_CONTENT, Json(serde_json::json!({}))).into_response(),
        Err(clarity_core::thread::ThreadManagerError::ThreadStore(
            clarity_thread_store::ThreadStoreError::NotFound { .. },
        )) => thread_not_found(&id),
        Err(e) => store_error(e),
    }
}

/// Fork a thread.
pub async fn fork_thread(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(req): Json<ForkThreadRequest>,
) -> axum::response::Response {
    let source_thread_id = match parse_thread_id(&id) {
        Ok(t) => t,
        Err(resp) => return resp,
    };

    let snapshot = req
        .snapshot
        .map(Into::into)
        .unwrap_or(clarity_thread_store::ForkSnapshot::Interrupted);

    match state
        .thread_store
        .fork_thread(clarity_thread_store::ForkThreadParams {
            source_thread_id,
            snapshot,
            new_thread_id: None,
        })
        .await
    {
        Ok(new_id) => (
            StatusCode::CREATED,
            Json(serde_json::json!({ "thread_id": new_id.to_string() })),
        )
            .into_response(),
        Err(clarity_thread_store::ThreadStoreError::NotFound { .. }) => thread_not_found(&id),
        Err(e) => store_error(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_thread_via_manager() {
        let manager = clarity_core::thread::ThreadManager::new_in_memory();
        let thread_id = manager
            .create_thread(".", "test", SessionSource::Test)
            .await
            .unwrap();
        let stored = manager.read_thread(thread_id, false).await.unwrap();
        assert_eq!(stored.thread_id, thread_id);
    }

    #[test]
    fn test_fork_snapshot_body_mapping() {
        let body = ForkSnapshotBody::TruncateBeforeNthUserMessage { n: 3 };
        let snapshot: clarity_thread_store::ForkSnapshot = body.into();
        assert_eq!(
            snapshot,
            clarity_thread_store::ForkSnapshot::TruncateBeforeNthUserMessage(3)
        );
    }
}
