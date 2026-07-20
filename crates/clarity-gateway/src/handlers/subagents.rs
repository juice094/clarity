//! Subagent orchestration REST API.
//!
//! Exposes the `clarity-subagents` crate capabilities through the public
//! Gateway API under `/api/v1/subagents/*`. These endpoints are intentionally
//! separate from the legacy `/v1/parallel` background-task routes: they speak
//! the subagent-native `RunSpec` / `ParallelConfig` semantics and are designed
//! for external orchestrators such as the KimiClaw ACP bridge.

use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info};

use crate::server::AppState;
use clarity_contract::subagent::{ParallelConfig, RunSpec};

/// Request body for running a single subagent.
#[derive(Debug, Deserialize)]
pub(crate) struct RunSubagentRequest {
    /// Human-readable task description.
    pub description: String,
    /// Agent type registered in the labor market (e.g. `coder`, `review`).
    pub agent_type: String,
    /// Prompt / instructions for the subagent.
    pub prompt: String,
    /// Optional model alias override.
    #[serde(default)]
    pub model: Option<String>,
    /// Optional maximum iteration override.
    #[serde(default)]
    pub max_iterations: Option<usize>,
    /// Whether to force read-only mode.
    #[serde(default)]
    pub read_only: bool,
    /// Optional goal tags for routing.
    #[serde(default)]
    pub goal_tags: Vec<String>,
}

/// Request body for running multiple subagents in parallel.
#[derive(Debug, Deserialize)]
pub(crate) struct RunParallelSubagentsRequest {
    /// Subagent specifications.
    pub tasks: Vec<RunSubagentRequest>,
    /// Maximum concurrency.
    #[serde(default)]
    pub max_concurrency: Option<usize>,
    /// Cancel remaining tasks when one fails.
    #[serde(default)]
    pub cancel_on_error: bool,
}

/// Serializable subagent execution result.
#[derive(Debug, Serialize)]
pub(crate) struct SubagentRunResponse {
    /// Agent identifier.
    pub agent_id: String,
    /// Agent type.
    pub agent_type: String,
    /// Execution status.
    pub status: String,
    /// Result summary.
    pub summary: String,
    /// Full output.
    pub full_output: String,
    /// Steps taken.
    pub steps_taken: usize,
    /// Elapsed milliseconds.
    pub elapsed_ms: u64,
}

/// Serializable parallel execution result.
#[derive(Debug, Serialize)]
pub(crate) struct ParallelSubagentsResponse {
    /// Overall success rate.
    pub success_rate: f64,
    /// Total elapsed milliseconds.
    pub total_elapsed_ms: u64,
    /// Successful results.
    pub results: Vec<SubagentRunResponse>,
    /// Failures as (description, error) pairs.
    pub failures: Vec<ParallelSubagentFailure>,
    /// Optional aggregated summary.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub aggregated_summary: Option<String>,
}

/// Failure entry for a parallel run.
#[derive(Debug, Serialize)]
pub(crate) struct ParallelSubagentFailure {
    /// Task description.
    pub description: String,
    /// Error message.
    pub error: String,
}

/// Registered agent type summary.
#[derive(Debug, Serialize)]
pub(crate) struct SubagentTypeSummary {
    /// Agent type name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Default maximum iterations.
    pub max_iterations: usize,
    /// Whether the agent is read-only by default.
    pub read_only: bool,
    /// Capability tags.
    pub capabilities: Vec<String>,
}

/// List registered subagent types.
#[derive(Debug, Serialize)]
pub(crate) struct SubagentTypeListResponse {
    /// Registered agent types.
    pub types: Vec<SubagentTypeSummary>,
}

/// Run a single subagent.
pub(crate) async fn run_subagent(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RunSubagentRequest>,
) -> Response {
    if req.prompt.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "prompt is required"})),
        )
            .into_response();
    }

    let mut spec = RunSpec::new(req.description, req.prompt).with_type(req.agent_type);
    if let Some(model) = req.model {
        spec = spec.with_model(model);
    }
    if let Some(max_iterations) = req.max_iterations {
        spec = spec.with_max_iterations(max_iterations);
    }
    if req.read_only {
        spec = spec.with_read_only(true);
    }
    if !req.goal_tags.is_empty() {
        spec = spec.with_goal_tags(req.goal_tags);
    }

    let mut manager = state.subagent_manager.lock().await;
    match manager.run(spec, None).await {
        Ok(result) => {
            info!(
                agent_id = %result.agent_id,
                agent_type = %result.agent_type,
                "Subagent run completed"
            );
            let response = SubagentRunResponse {
                agent_id: result.agent_id,
                agent_type: result.agent_type,
                status: result.status.to_string(),
                summary: result.summary,
                full_output: result.full_output,
                steps_taken: result.steps_taken,
                elapsed_ms: result.elapsed_ms,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            error!("Subagent run failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

/// Run multiple subagents in parallel.
pub(crate) async fn run_parallel_subagents(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RunParallelSubagentsRequest>,
) -> Response {
    if req.tasks.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No tasks provided"})),
        )
            .into_response();
    }

    let specs: Vec<RunSpec> = req
        .tasks
        .into_iter()
        .map(|task| {
            let mut spec = RunSpec::new(task.description, task.prompt).with_type(task.agent_type);
            if let Some(model) = task.model {
                spec = spec.with_model(model);
            }
            if let Some(max_iterations) = task.max_iterations {
                spec = spec.with_max_iterations(max_iterations);
            }
            if task.read_only {
                spec = spec.with_read_only(true);
            }
            if !task.goal_tags.is_empty() {
                spec = spec.with_goal_tags(task.goal_tags);
            }
            spec
        })
        .collect();

    let config =
        ParallelConfig::new().with_max_concurrency(req.max_concurrency.unwrap_or(4).max(1));
    let config = if req.cancel_on_error {
        config.cancel_on_error()
    } else {
        config
    };

    let manager = state.subagent_manager.lock().await;
    match manager.run_parallel(specs, config, None, None).await {
        Ok(result) => {
            let success_rate = result.success_rate();
            let total_elapsed_ms = result.total_elapsed_ms;
            let aggregated_summary = result.aggregated_summary.clone();

            let results: Vec<SubagentRunResponse> = result
                .results
                .into_iter()
                .map(|r| SubagentRunResponse {
                    agent_id: r.agent_id,
                    agent_type: r.agent_type,
                    status: r.status.to_string(),
                    summary: r.summary,
                    full_output: r.full_output,
                    steps_taken: r.steps_taken,
                    elapsed_ms: r.elapsed_ms,
                })
                .collect();

            let failures: Vec<ParallelSubagentFailure> = result
                .failures
                .into_iter()
                .map(|(description, error)| ParallelSubagentFailure { description, error })
                .collect();

            let response = ParallelSubagentsResponse {
                success_rate,
                total_elapsed_ms,
                results,
                failures,
                aggregated_summary,
            };
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            error!("Parallel subagent run failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
        }
    }
}

/// List registered subagent types.
pub(crate) async fn list_subagent_types(State(state): State<Arc<AppState>>) -> Response {
    let manager = state.subagent_manager.lock().await;
    let types = manager
        .labor_market()
        .list()
        .into_iter()
        .map(|def| SubagentTypeSummary {
            name: def.name.clone(),
            description: def.description.clone(),
            max_iterations: def.max_iterations,
            read_only: def.read_only,
            capabilities: def.capabilities.clone(),
        })
        .collect();

    (StatusCode::OK, Json(SubagentTypeListResponse { types })).into_response()
}

#[cfg(test)]
mod tests {
    use crate::server::{AppState, create_api_router};
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
            .with_max_iterations(2)
            .with_read_only(false);
        let agent = Arc::new(Agent::with_config(registry, config).with_llm(Arc::new(MockLlm)));

        let temp = std::env::temp_dir().join(format!(
            "clarity-subagents-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
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

    async fn read_json_body(res: axum::response::Response) -> serde_json::Value {
        let body = res.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn test_list_subagent_types() {
        let state = test_state().await;
        let app = create_api_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/subagents/types")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json_body(response).await;
        assert!(body.get("types").is_some());
        let types = body["types"].as_array().unwrap();
        assert!(!types.is_empty());
        assert!(types.iter().any(|t| t["name"] == "coder"));
    }

    #[tokio::test]
    async fn test_run_subagent_missing_prompt() {
        let state = test_state().await;
        let app = create_api_router(state);

        let req_body = serde_json::json!({
            "description": "test",
            "agent_type": "coder",
            "prompt": ""
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/subagents/run")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(req_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = read_json_body(response).await;
        assert!(body["error"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_run_parallel_subagents_empty() {
        let state = test_state().await;
        let app = create_api_router(state);

        let req_body = serde_json::json!({"tasks": []});

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/subagents/run/parallel")
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
    async fn test_run_subagent_success() {
        let state = test_state().await;
        let app = create_api_router(state);

        let req_body = serde_json::json!({
            "description": "unit-test-run",
            "agent_type": "coder",
            "prompt": "Say hello",
            "max_iterations": 1
        });

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/subagents/run")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(req_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json_body(response).await;
        assert!(body["agent_id"].as_str().is_some());
        assert_eq!(body["agent_type"], "coder");
    }
}
