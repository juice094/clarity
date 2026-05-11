use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use std::sync::Arc;
use tracing::{error, info};

use crate::server::AppState;

#[derive(Serialize)]
pub(crate) struct CronTaskOverview {
    pub task_id: String,
    pub name: String,
    pub cron_expr: String,
    pub enabled: bool,
    pub next_run: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct CronTasksResponse {
    pub tasks: Vec<CronTaskOverview>,
}

#[derive(Deserialize)]
pub(crate) struct CreateCronRequest {
    pub name: String,
    pub prompt: String,
    pub cron_expr: String,
    pub agent_type: Option<String>,
    pub max_iterations: Option<usize>,
}

#[derive(Serialize)]
pub(crate) struct CreateCronResponse {
    pub task_id: String,
}

pub(crate) async fn list_cron_tasks(State(state): State<Arc<AppState>>) -> Json<CronTasksResponse> {
    let tasks = state
        .task_manager
        .list_cron_tasks()
        .await
        .unwrap_or_default();
    let overviews: Vec<CronTaskOverview> = tasks
        .into_iter()
        .map(|t| CronTaskOverview {
            task_id: t.task_id.clone(),
            name: t.task_spec.name.clone(),
            cron_expr: t.schedule.expr.clone(),
            enabled: t.enabled,
            next_run: None, // computed on next scheduler tick
        })
        .collect();
    Json(CronTasksResponse { tasks: overviews })
}

pub(crate) async fn create_cron_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateCronRequest>,
) -> Response {
    let spec = clarity_core::background::TaskSpec::new(req.name.clone(), req.prompt)
        .with_agent_type(req.agent_type.unwrap_or_else(|| "default".into()))
        .with_max_iterations(req.max_iterations.unwrap_or(10));

    match state.task_manager.schedule_cron(spec, &req.cron_expr).await {
        Ok(task_id) => {
            info!("Created cron task: {} ({})", req.name, task_id);
            (StatusCode::CREATED, Json(CreateCronResponse { task_id })).into_response()
        }
        Err(e) => {
            error!("Failed to create cron task: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

pub(crate) async fn delete_cron_task(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(task_id): axum::extract::Path<String>,
) -> Response {
    match state.task_manager.cancel_cron(&task_id).await {
        Ok(()) => {
            info!("Deleted cron task: {}", task_id);
            (StatusCode::OK, Json(serde_json::json!({"deleted": true}))).into_response()
        }
        Err(e) => {
            error!("Failed to delete cron task {}: {}", task_id, e);
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}
