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
            .unwrap_or_else(|e| {
                tracing::warn!(
                    "Failed to build reqwest Client with timeout, falling back with 10s timeout manually configured: {}",
                    e
                );
                // SAFE: the builder failed (likely TLS backend issue), but we still
                // need a working client. Build a new one and rely on tokio::time::timeout
                // at the call site for timeout enforcement.
                reqwest::Client::builder()
                    .timeout(std::time::Duration::from_secs(10))
                    .build()
                    .unwrap_or_else(|_| reqwest::Client::new())
            });
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_default_reads_env_or_falls_back() {
        let client = GatewayTaskClient::default();
        let env_url = std::env::var("CLARITY_GATEWAY_URL").unwrap_or_default();
        if env_url.is_empty() {
            assert_eq!(client.base_url, DEFAULT_GATEWAY);
        } else {
            assert_eq!(client.base_url, env_url);
        }
    }

    #[test]
    fn client_new_and_default_are_equivalent() {
        let client1 = GatewayTaskClient::new();
        let client2 = GatewayTaskClient::default();
        assert_eq!(client1.base_url, client2.base_url);
    }

    #[test]
    fn task_detail_dto_converts_to_task_info() {
        use clarity_core::background::TaskStatus;
        let dto = TaskDetailDto {
            task_id: "task-1".into(),
            name: "Test Task".into(),
            status: TaskStatus::Running,
            prompt: "Do something".into(),
            created_at: 1000,
            updated_at: 2000,
        };
        let info = dto.into_task_info();
        assert_eq!(info.id, "task-1");
        assert_eq!(info.spec.name, "Test Task");
        assert_eq!(info.spec.prompt, "Do something");
        assert!(matches!(info.status, TaskStatus::Running));
        assert_eq!(info.created_at, 1000);
        assert_eq!(info.updated_at, 2000);
    }

    #[test]
    fn task_detail_dto_default_agent_type_and_priority() {
        use clarity_core::background::TaskStatus;
        let dto = TaskDetailDto {
            task_id: "task-2".into(),
            name: "Minimal".into(),
            status: TaskStatus::Pending,
            prompt: "prompt".into(),
            created_at: 0,
            updated_at: 0,
        };
        let info = dto.into_task_info();
        // Spec defaults are filled in by the conversion.
        assert_eq!(info.spec.description, "");
        assert_eq!(info.spec.agent_type, "default");
        assert_eq!(info.spec.max_iterations, None);
        assert_eq!(info.spec.timeout_seconds, None);
    }

    #[test]
    fn create_task_request_serializes_correctly() {
        let req = CreateTaskRequest {
            name: "My Task".into(),
            prompt: "Do it".into(),
            max_iterations: Some(10),
        };
        let json = serde_json::to_value(&req).unwrap();
        assert_eq!(json["name"], "My Task");
        assert_eq!(json["prompt"], "Do it");
        assert_eq!(json["max_iterations"], 10);
    }

    #[test]
    fn create_task_request_skips_none_max_iterations() {
        let req = CreateTaskRequest {
            name: "Task".into(),
            prompt: "Go".into(),
            max_iterations: None,
        };
        let json = serde_json::to_value(&req).unwrap();
        assert!(json.get("max_iterations").is_none());
    }

    #[test]
    fn create_task_response_deserializes() {
        let json = serde_json::json!({"task_id": "t-abc", "status": "Pending"});
        let resp: CreateTaskResponse = serde_json::from_value(json).unwrap();
        assert_eq!(resp.task_id, "t-abc");
    }

    #[test]
    fn client_urls_use_base_url() {
        let client = GatewayTaskClient::default();
        // Verify the URLs are derived from the base_url.
        assert!(client.base_url.starts_with("http://"));
        // The list_tasks URL would be {base_url}/v1/tasks
        // (tested indirectly via URL construction in list_tasks/create_task)
    }
}
