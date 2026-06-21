use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use std::sync::Arc;
use tracing::{error, info};

use crate::handlers::AgentHandle;
use crate::server::AppState;
use clarity_core::background::TaskId;
use clarity_core::background::{TaskResult, TaskSpec, TaskStatus};

// ==================== Background Tasks API ====================

#[derive(Debug, Deserialize)]
pub(crate) struct CreateTaskRequest {
    pub name: String,
    pub prompt: String,
    #[serde(default)]
    pub max_iterations: Option<usize>,
}

#[derive(Serialize)]
pub(crate) struct TaskCreateResponse {
    pub task_id: TaskId,
    pub status: TaskStatus,
}

/// Detailed background task response returned by the HTTP and WebSocket APIs.
#[derive(Debug, Serialize, Deserialize)]
pub struct TaskDetailResponse {
    /// Task identifier.
    pub task_id: TaskId,
    /// Human-readable task name.
    pub name: String,
    /// Current task status.
    pub status: TaskStatus,
    /// Prompt that the task is executing against.
    pub prompt: String,
    /// Creation timestamp as seconds since the Unix epoch.
    pub created_at: u64,
    /// Last update timestamp as seconds since the Unix epoch.
    pub updated_at: u64,
    /// Final task result, if the task has reached a terminal state.
    pub result: Option<TaskResult>,
}

pub(crate) async fn create_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateTaskRequest>,
) -> Response {
    let spec = TaskSpec::new(req.name.clone(), req.prompt)
        .with_agent_type("default")
        .with_max_iterations(req.max_iterations.unwrap_or(10));

    match state.task_manager.spawn_agent(spec).await {
        Ok(task_id) => {
            let response = TaskCreateResponse {
                task_id: task_id.clone(),
                status: TaskStatus::Pending,
            };
            info!("Created background task {}", task_id);
            (StatusCode::ACCEPTED, Json(response)).into_response()
        }
        Err(e) => {
            error!("Failed to create background task: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

pub(crate) async fn get_task(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(task_id): axum::extract::Path<TaskId>,
) -> Response {
    info!("get_task called with id={}", task_id);
    let store = state.task_manager.store();
    match store.get(&task_id).await {
        Ok(info) => {
            let result = if info.status.is_terminal() {
                store.get_result(&task_id).await.ok()
            } else {
                None
            };
            let response = TaskDetailResponse {
                task_id: info.id,
                name: info.spec.name,
                status: info.status,
                prompt: info.spec.prompt,
                created_at: info.created_at,
                updated_at: info.updated_at,
                result,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            error!("Failed to get task {}: {}", task_id, e);
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

pub(crate) async fn cancel_task(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(task_id): axum::extract::Path<TaskId>,
) -> impl IntoResponse {
    info!("cancel_task called with id={}", task_id);
    match state.task_manager.cancel(&task_id).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"cancelled": true}))),
        Err(e) => {
            error!("Failed to cancel task {}: {}", task_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
        }
    }
}

#[derive(Serialize)]
pub(crate) struct TaskListResponse {
    pub tasks: Vec<TaskDetailResponse>,
}

pub(crate) async fn list_tasks(State(state): State<Arc<AppState>>) -> Response {
    info!("list_tasks called");
    match state.task_manager.list().await {
        Ok(tasks) => {
            let response = TaskListResponse {
                tasks: tasks
                    .into_iter()
                    .map(|info| TaskDetailResponse {
                        task_id: info.id,
                        name: info.spec.name,
                        status: info.status,
                        prompt: info.spec.prompt,
                        created_at: info.created_at,
                        updated_at: info.updated_at,
                        result: None,
                    })
                    .collect(),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            error!("Failed to list tasks: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

// ==================== Parallel Subagent Execution API ====================

#[derive(Debug, Deserialize)]
pub(crate) struct ParallelTaskSpec {
    pub agent_type: String,
    pub prompt: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RunParallelRequest {
    pub tasks: Vec<ParallelTaskSpec>,
    #[serde(default)]
    pub max_concurrency: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct ParallelTaskResult {
    pub agent_id: String,
    pub agent_type: String,
    pub status: String,
    pub summary: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct ParallelFailure {
    pub task_id: String,
    pub error: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct RunParallelResponse {
    pub success_rate: f64,
    pub total_elapsed_ms: u64,
    pub results: Vec<ParallelTaskResult>,
    pub failures: Vec<ParallelFailure>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub batch_id: Option<String>,
}

pub(crate) async fn run_parallel(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RunParallelRequest>,
) -> Response {
    if req.tasks.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No tasks provided"})),
        )
            .into_response();
    }

    let batch_id = uuid::Uuid::new_v4().to_string();

    // Clone task specs before moving them into batch_progress
    let task_refs: Vec<(String, String)> = req
        .tasks
        .iter()
        .map(|t| (t.agent_type.clone(), t.prompt.clone()))
        .collect();

    let specs: Vec<clarity_contract::subagent::RunSpec> = task_refs
        .iter()
        .map(|(agent_type, prompt)| {
            clarity_contract::subagent::RunSpec::new(
                format!("parallel-{}", agent_type),
                prompt.clone(),
            )
            .with_type(agent_type)
        })
        .collect();

    // Create and register batch progress
    let progress = Arc::new(parking_lot::Mutex::new(
        clarity_contract::subagent::BatchProgress::new(batch_id.clone(), &specs),
    ));
    {
        let mut batches = state.parallel_batches.write().await;
        batches.insert(batch_id.clone(), progress.clone());
    }

    let config = clarity_contract::subagent::ParallelConfig::new()
        .with_max_concurrency(req.max_concurrency.unwrap_or(4).max(1));

    let agent = state.clone_agent();

    match agent
        .run_parallel(specs, config, Some(progress.clone()))
        .await
    {
        Ok(result) => {
            let success_rate = result.success_rate();
            let total_elapsed_ms = result.total_elapsed_ms;

            let results: Vec<ParallelTaskResult> = result
                .results
                .into_iter()
                .map(|r| ParallelTaskResult {
                    agent_id: r.agent_id,
                    agent_type: r.agent_type,
                    status: format!("{:?}", r.status),
                    summary: r.summary,
                })
                .collect();

            let failures: Vec<ParallelFailure> = result
                .failures
                .into_iter()
                .map(|(id, err)| ParallelFailure {
                    task_id: id,
                    error: err,
                })
                .collect();

            let response = RunParallelResponse {
                success_rate,
                total_elapsed_ms,
                results,
                failures,
                batch_id: Some(batch_id.clone()),
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            // Mark progress as failed
            let mut p = progress.lock();
            p.status = clarity_contract::subagent::BatchStatus::Failed(e.to_string());
            error!("Parallel execution failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

/// Query the current progress of a parallel batch.
#[derive(Serialize)]
pub(crate) struct ParallelStatusResponse {
    pub batch_id: String,
    pub total: usize,
    pub completed: usize,
    pub failed: usize,
    pub status: String,
    pub elapsed_ms: u64,
    pub agent_statuses: Vec<AgentStatusSummary>,
}

#[derive(Serialize)]
pub(crate) struct AgentStatusSummary {
    pub agent_id: String,
    pub status: String,
    pub summary: Option<String>,
}

pub(crate) async fn get_parallel_status(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(batch_id): axum::extract::Path<String>,
) -> Response {
    let batches = state.parallel_batches.read().await;
    match batches.get(&batch_id) {
        Some(progress_arc) => {
            let p = progress_arc.lock();
            let status_str = match &p.status {
                clarity_contract::subagent::BatchStatus::Running => "Running",
                clarity_contract::subagent::BatchStatus::Completed => "Completed",
                clarity_contract::subagent::BatchStatus::Cancelled => "Cancelled",
                clarity_contract::subagent::BatchStatus::Failed(_) => "Failed",
            };

            let mut agent_statuses: Vec<AgentStatusSummary> = p
                .results
                .iter()
                .map(|r| AgentStatusSummary {
                    agent_id: r.agent_id.clone(),
                    status: "Completed".to_string(),
                    summary: Some(r.summary.clone()),
                })
                .collect();

            // Add running agents
            for id in &p.running {
                agent_statuses.push(AgentStatusSummary {
                    agent_id: id.clone(),
                    status: "Running".to_string(),
                    summary: None,
                });
            }

            // Add failures
            for (id, err) in &p.failures {
                agent_statuses.push(AgentStatusSummary {
                    agent_id: id.clone(),
                    status: "Failed".to_string(),
                    summary: Some(err.clone()),
                });
            }

            let response = ParallelStatusResponse {
                batch_id: p.batch_id.clone(),
                total: p.total,
                completed: p.completed,
                failed: p.failed,
                status: status_str.to_string(),
                elapsed_ms: p.elapsed_ms,
                agent_statuses,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Batch not found"})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use crate::server::{AppState, create_api_router};
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use clarity_core::agent::{Agent, AgentConfig, MockLlm};
    use clarity_core::background::BackgroundTaskManager;
    use clarity_core::background::agent_executor::DefaultAgentTaskExecutor;
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

        let temp = std::env::temp_dir().join(format!(
            "clarity-tasks-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let _ = std::fs::create_dir_all(&temp);

        let registry = ToolRegistry::with_builtin_tools();
        let llm = Arc::new(MockLlm);
        let executor = Arc::new(DefaultAgentTaskExecutor::new(
            llm,
            registry,
            temp.join("work"),
        ));
        let task_manager = Arc::new(
            BackgroundTaskManager::new(temp.join("store"), temp.join("work"), temp.join("context"))
                .with_agent_executor(executor),
        );

        Arc::new(
            AppState::new_with_home(agent, task_manager, temp.join(".clarity"))
                .await
                .unwrap(),
        )
    }

    async fn read_json_body(res: axum::response::Response) -> serde_json::Value {
        let body = res.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn test_list_tasks_empty() {
        let state = test_state().await;
        let app = create_api_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/tasks")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json_body(response).await;
        assert!(body.get("tasks").is_some(), "missing tasks key: {}", body);
        assert!(
            body["tasks"].is_array(),
            "tasks is not an array: {}",
            body["tasks"]
        );
        assert!(body["tasks"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_create_task() {
        let state = test_state().await;
        let app = create_api_router(state.clone());

        let req_body = serde_json::json!({
            "name": "unit-test-task",
            "prompt": "Say hello",
            "max_iterations": 1
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/tasks")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(req_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let body = read_json_body(response).await;
        let task_id = body["task_id"].as_str().unwrap();
        assert!(!task_id.is_empty());
        assert_eq!(body["status"], "Pending");

        // Clean up the background task.
        let _ = state.task_manager.cancel(&task_id.to_string()).await;
    }

    #[tokio::test]
    async fn test_create_task_invalid_json() {
        let state = test_state().await;
        let app = create_api_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/tasks")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{not valid json"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_get_task() {
        let state = test_state().await;
        let app = create_api_router(state.clone());

        let req_body = serde_json::json!({
            "name": "unit-test-get-task",
            "prompt": "Say hello",
            "max_iterations": 1
        });

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/tasks")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(req_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(create_response.status(), StatusCode::ACCEPTED);
        let create_body = read_json_body(create_response).await;
        let task_id = create_body["task_id"].as_str().unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/tasks/{}", task_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json_body(response).await;
        assert_eq!(body["task_id"], task_id);
        assert_eq!(body["name"], "unit-test-get-task");
        assert_eq!(body["prompt"], "Say hello");
        assert!(body.get("status").is_some());

        // Clean up the background task.
        let _ = state.task_manager.cancel(&task_id.to_string()).await;
    }

    #[tokio::test]
    async fn test_get_task_not_found() {
        let state = test_state().await;
        let app = create_api_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/tasks/nonexistent-task-12345")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = read_json_body(response).await;
        assert!(body["error"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_cancel_task() {
        let state = test_state().await;
        let app = create_api_router(state.clone());

        let req_body = serde_json::json!({
            "name": "unit-test-cancel-task",
            "prompt": "Say hello",
            "max_iterations": 1
        });

        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/tasks")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(req_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(create_response.status(), StatusCode::ACCEPTED);
        let create_body = read_json_body(create_response).await;
        let task_id = create_body["task_id"].as_str().unwrap();

        let response = app
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/tasks/{}", task_id))
                    .method("DELETE")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json_body(response).await;
        assert_eq!(body["cancelled"], true);
    }

    #[tokio::test]
    async fn test_cancel_task_not_found() {
        let state = test_state().await;
        let app = create_api_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/tasks/nonexistent-task-12345")
                    .method("DELETE")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        let body = read_json_body(response).await;
        assert!(body["error"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_run_parallel_empty_tasks() {
        let state = test_state().await;
        let app = create_api_router(state);

        let req_body = serde_json::json!({"tasks": []});

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/parallel")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(req_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = read_json_body(response).await;
        assert_eq!(body["error"], "No tasks provided");
    }

    #[tokio::test]
    async fn test_run_parallel_invalid_json() {
        let state = test_state().await;
        let app = create_api_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/parallel")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{not valid json"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn test_get_parallel_status_not_found() {
        let state = test_state().await;
        let app = create_api_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/parallel/nonexistent-batch/status")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = read_json_body(response).await;
        assert_eq!(body["error"], "Batch not found");
    }
}
