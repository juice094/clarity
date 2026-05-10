use axum::{
    extract::Json,
    http::StatusCode,
    response::{
        IntoResponse, Response,
    },
};
use serde::{Deserialize, Serialize};

use tracing::error;

use clarity_core::memory::{MemoryStore, PersistentMemoryStore};

// ==================== 跨会话全文检索 API ====================

#[derive(Deserialize)]
pub(crate) struct SearchRequest {
    pub query: String,
    #[serde(default = "default_search_limit")]
    pub limit: usize,
}

fn default_search_limit() -> usize {
    20
}

#[derive(Serialize)]
pub(crate) struct SearchResult {
    pub fact_id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub score: f32,
}

#[derive(Serialize)]
pub(crate) struct SearchResponse {
    pub results: Vec<SearchResult>,
    pub total: usize,
}

pub(crate) async fn search_memory(Json(req): Json<SearchRequest>) -> Response {
    // Try to open the persistent memory store from the default location
    let clarity_dir = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".clarity");
    let memory_db = clarity_dir.join("memory.db");

    if !memory_db.exists() {
        return (
            StatusCode::OK,
            Json(SearchResponse {
                results: Vec::new(),
                total: 0,
            }),
        )
            .into_response();
    }

    match PersistentMemoryStore::new(memory_db.as_path()).await {
        Ok(memory) => {
            let memories = memory.search(&req.query, req.limit).await;
            match memories {
                Ok(memories) => {
                    let results: Vec<SearchResult> = memories
                        .into_iter()
                        .map(|m| SearchResult {
                            fact_id: m.id,
                            content: m.content,
                            tags: m.tags,
                            score: m.importance,
                        })
                        .collect();
                    let total = results.len();
                    (StatusCode::OK, Json(SearchResponse { results, total })).into_response()
                }
                Err(e) => {
                    error!("Memory search failed: {}", e);
                    (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({"error": e.to_string()})),
                    )
                        .into_response()
                }
            }
        }
        Err(e) => {
            error!("Failed to open memory store: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}
