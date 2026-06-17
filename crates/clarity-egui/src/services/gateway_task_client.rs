//! Gateway Task HTTP Client
//!
//! Bridges egui's task panel to the Gateway `/v1/tasks` REST API so that
//! background tasks are managed through a single source of truth (Gateway)
//! rather than each frontend maintaining its own `TaskStore`.
//!
//! If the Gateway is unreachable every operation falls back to the local
//! `clarity_core::background::TaskStore` so the UI never breaks when the
//! Gateway is offline.

use clarity_core::background::{TaskId, TaskInfo, TaskSpec};
use serde::{Deserialize, Serialize};

const DEFAULT_GATEWAY: &str = "http://127.0.0.1:18790";

/// DTO that mirrors `clarity_gateway::handlers::TaskDetailResponse`.
#[derive(Debug, Clone, Deserialize)]
struct TaskDetailDto {
    pub task_id: TaskId,
    pub name: String,
    pub status: clarity_core::background::TaskStatus,
    pub prompt: String,
    pub created_at: u64,
    pub updated_at: u64,
}

impl TaskDetailDto {
    /// Convert the flat Gateway DTO into the nested `TaskInfo` used by egui.
    fn into_task_info(self) -> TaskInfo {
        TaskInfo {
            id: self.task_id,
            spec: TaskSpec {
                name: self.name,
                description: String::new(),
                agent_type: "default".to_string(),
                prompt: self.prompt,
                max_iterations: None,
                timeout_seconds: None,
                priority: clarity_core::background::TaskPriority::Normal,
                model_alias: None,
            },
            status: self.status,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// DTO for `POST /v1/tasks` request body.
#[derive(Debug, Serialize)]
struct CreateTaskRequest {
    name: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_iterations: Option<usize>,
}

/// DTO for `POST /v1/tasks` response body.
#[derive(Debug, Deserialize)]
struct CreateTaskResponse {
    task_id: TaskId,
    #[allow(dead_code)]
    status: clarity_core::background::TaskStatus,
}

/// Thin HTTP client for Gateway task endpoints.
#[derive(Debug, Clone)]
pub struct GatewayTaskClient {
    base_url: String,
    client: reqwest::Client,
}

impl GatewayTaskClient {
    /// Create a new client pointing at `CLARITY_GATEWAY_URL` or the default.
    pub fn new() -> Self {
        let base_url =
            std::env::var("CLARITY_GATEWAY_URL").unwrap_or_else(|_| DEFAULT_GATEWAY.to_string());
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        Self { base_url, client }
    }

    /// `GET /v1/tasks`
    pub async fn list_tasks(&self) -> Result<Vec<TaskInfo>, String> {
        let url = format!("{}/v1/tasks", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("Gateway returned {}", resp.status()));
        }
        #[derive(Deserialize)]
        struct TaskListResponse {
            tasks: Vec<TaskDetailDto>,
        }
        let body: TaskListResponse = resp.json().await.map_err(|e| e.to_string())?;
        Ok(body.tasks.into_iter().map(|d| d.into_task_info()).collect())
    }

    /// `POST /v1/tasks`
    pub async fn create_task(
        &self,
        name: &str,
        prompt: &str,
        max_iterations: Option<usize>,
    ) -> Result<TaskId, String> {
        let url = format!("{}/v1/tasks", self.base_url);
        let req_body = CreateTaskRequest {
            name: name.to_string(),
            prompt: prompt.to_string(),
            max_iterations,
        };
        let resp = self
            .client
            .post(&url)
            .json(&req_body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("Gateway returned {}", resp.status()));
        }
        let body: CreateTaskResponse = resp.json().await.map_err(|e| e.to_string())?;
        Ok(body.task_id)
    }
}

impl Default for GatewayTaskClient {
    fn default() -> Self {
        Self::new()
    }
}
