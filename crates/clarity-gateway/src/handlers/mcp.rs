use axum::{
    extract::Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use tracing::warn;

/// Resolve the MCP config path, preferring the `CLARITY_MCP_CONFIG_PATH`
/// environment override over the platform default.
fn resolve_mcp_config_path() -> anyhow::Result<std::path::PathBuf> {
    std::env::var("CLARITY_MCP_CONFIG_PATH")
        .map(std::path::PathBuf::from)
        .or_else(|_| clarity_core::mcp::config::default_config_path())
}

/// Overview of a single MCP server entry (from `mcp.json`).
#[derive(Serialize)]
pub(crate) struct McpServerOverview {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub disabled: bool,
    pub transport: Option<String>,
    pub url: Option<String>,
}

/// Response for listing MCP servers.
#[derive(Serialize)]
pub(crate) struct McpServersResponse {
    pub servers: Vec<McpServerOverview>,
    pub config_path: String,
}

pub(crate) async fn list_mcp_servers() -> Response {
    let mcp_path = resolve_mcp_config_path().ok();
    let config = match &mcp_path {
        Some(path) => clarity_core::mcp::config::McpConfig::load(path),
        None => Err(anyhow::anyhow!("Could not determine MCP config path")),
    };
    match config {
        Ok(config) => {
            let servers: Vec<McpServerOverview> = config
                .servers
                .into_iter()
                .map(|(name, entry)| McpServerOverview {
                    name,
                    command: entry.command,
                    args: entry.args,
                    disabled: entry.disabled,
                    transport: entry.transport,
                    url: entry.url,
                })
                .collect();
            let config_path = mcp_path
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
            (
                StatusCode::OK,
                Json(McpServersResponse {
                    servers,
                    config_path,
                }),
            )
                .into_response()
        }
        Err(e) => {
            warn!("Failed to load MCP config: {}", e);
            (
                StatusCode::OK,
                Json(McpServersResponse {
                    servers: Vec::new(),
                    config_path: String::new(),
                }),
            )
                .into_response()
        }
    }
}

/// Get a single MCP server by name.
pub(crate) async fn get_mcp_server(
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Response {
    let config = match resolve_mcp_config_path() {
        Ok(path) => clarity_core::mcp::config::McpConfig::load(&path),
        Err(e) => Err(e),
    };
    match config {
        Ok(config) => match config.servers.get(&name) {
            Some(entry) => (
                StatusCode::OK,
                Json(McpServerOverview {
                    name: name.clone(),
                    command: entry.command.clone(),
                    args: entry.args.clone(),
                    disabled: entry.disabled,
                    transport: entry.transport.clone(),
                    url: entry.url.clone(),
                }),
            )
                .into_response(),
            None => (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "MCP server not found"})),
            )
                .into_response(),
        },
        Err(e) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("Failed to load MCP config: {}", e)})),
        )
            .into_response(),
    }
}

#[derive(Deserialize)]
pub(crate) struct McpServerUpdate {
    pub command: Option<String>,
    pub args: Option<Vec<String>>,
    pub disabled: Option<bool>,
    pub transport: Option<String>,
    pub url: Option<String>,
    pub headers: Option<std::collections::HashMap<String, String>>,
    pub env: Option<std::collections::HashMap<String, String>>,
}

/// Create or update an MCP server.
pub(crate) async fn update_mcp_server(
    axum::extract::Path(name): axum::extract::Path<String>,
    Json(req): Json<McpServerUpdate>,
) -> Response {
    let path = match resolve_mcp_config_path() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    // Load existing config or create new
    let mut config = clarity_core::mcp::config::McpConfig::load(&path).unwrap_or_default();

    let entry = config.servers.entry(name.clone()).or_insert_with(|| {
        clarity_core::mcp::config::McpServerEntry {
            command: req.command.clone().unwrap_or_default(),
            ..Default::default()
        }
    });

    if let Some(cmd) = req.command {
        entry.command = cmd;
    }
    if let Some(a) = req.args {
        entry.args = a;
    }
    if let Some(d) = req.disabled {
        entry.disabled = d;
    }
    if let Some(t) = req.transport {
        entry.transport = Some(t);
    }
    if let Some(u) = req.url {
        entry.url = Some(u);
    }
    if let Some(h) = req.headers {
        entry.headers = h;
    }
    if let Some(e) = req.env {
        entry.env = e;
    }

    match config.save(&path) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"saved": true}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

/// Delete an MCP server.
pub(crate) async fn delete_mcp_server(
    axum::extract::Path(name): axum::extract::Path<String>,
) -> Response {
    let path = match resolve_mcp_config_path() {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response();
        }
    };

    let mut config = clarity_core::mcp::config::McpConfig::load(&path).unwrap_or_default();
    config.servers.remove(&name);

    match config.save(&path) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"deleted": true}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use clarity_core::agent::{Agent, AgentConfig, MockLlm};
    use clarity_core::background::BackgroundTaskManager;
    use clarity_core::registry::ToolRegistry;
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::util::ServiceExt;

    async fn read_json_body(res: axum::response::Response) -> serde_json::Value {
        let body = res.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&body).unwrap()
    }

    /// The handlers read `CLARITY_MCP_CONFIG_PATH` (or the platform default),
    /// which is a global resource. This lock serializes tests that mutate the
    /// env var so they don't race with each other.
    static MCP_CONFIG_PATH_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

    struct ConfigPathGuard {
        _lock: parking_lot::MutexGuard<'static, ()>,
        original: Option<String>,
        temp: std::path::PathBuf,
    }

    impl Drop for ConfigPathGuard {
        fn drop(&mut self) {
            match self.original {
                Some(ref val) => unsafe { std::env::set_var("CLARITY_MCP_CONFIG_PATH", val) },
                None => unsafe { std::env::remove_var("CLARITY_MCP_CONFIG_PATH") },
            }
            let _ = std::fs::remove_dir_all(&self.temp);
        }
    }

    fn set_temp_mcp_config_path() -> ConfigPathGuard {
        let lock = MCP_CONFIG_PATH_LOCK.lock();
        let temp = std::env::temp_dir().join(format!(
            "clarity-gateway-mcp-test-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();
        let config_path = temp.join("mcp.json");
        let original = std::env::var("CLARITY_MCP_CONFIG_PATH").ok();
        unsafe { std::env::set_var("CLARITY_MCP_CONFIG_PATH", &config_path) };
        ConfigPathGuard {
            _lock: lock,
            original,
            temp,
        }
    }

    async fn test_state() -> Arc<crate::server::AppState> {
        let registry = ToolRegistry::with_builtin_tools();
        let config = AgentConfig::new()
            .with_max_iterations(2)
            .with_read_only(false);
        let agent = Arc::new(Agent::with_config(registry, config).with_llm(Arc::new(MockLlm)));

        let temp = std::env::temp_dir().join(format!(
            "clarity-mcp-state-test-{}-{}",
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
            crate::server::AppState::new_with_home(agent, task_manager, temp.join(".clarity"))
                .await
                .unwrap(),
        )
    }

    #[test]
    fn test_mcp_server_overview_serialization() {
        let overview = McpServerOverview {
            name: "filesystem".to_string(),
            command: "npx".to_string(),
            args: vec![
                "-y".to_string(),
                "@modelcontextprotocol/server-filesystem".to_string(),
            ],
            disabled: false,
            transport: Some("stdio".to_string()),
            url: None,
        };
        let json = serde_json::to_value(&overview).unwrap();
        assert_eq!(json["name"], "filesystem");
        assert_eq!(json["command"], "npx");
        assert_eq!(
            json["args"],
            serde_json::json![["-y", "@modelcontextprotocol/server-filesystem"]]
        );
        assert_eq!(json["disabled"], false);
        assert_eq!(json["transport"], "stdio");
        assert!(json["url"].is_null());
    }

    #[test]
    fn test_mcp_server_update_deserialization() {
        let json = r#"{
            "command": "npx",
            "args": ["-y", "server"],
            "disabled": true,
            "transport": "sse",
            "url": "http://localhost:3001/sse",
            "headers": {"Authorization": "Bearer token"},
            "env": {"KEY": "value"}
        }"#;
        let update: McpServerUpdate = serde_json::from_str(json).unwrap();
        assert_eq!(update.command, Some("npx".to_string()));
        assert_eq!(
            update.args,
            Some(vec!["-y".to_string(), "server".to_string()])
        );
        assert_eq!(update.disabled, Some(true));
        assert_eq!(update.transport, Some("sse".to_string()));
        assert_eq!(update.url, Some("http://localhost:3001/sse".to_string()));
        assert_eq!(
            update.headers.as_ref().unwrap()["Authorization"],
            "Bearer token"
        );
        assert_eq!(update.env.as_ref().unwrap()["KEY"], "value");
    }

    #[tokio::test]
    async fn test_list_mcp_servers_empty_when_config_missing() {
        let _guard = set_temp_mcp_config_path();
        let state = test_state().await;
        let app = crate::server::create_api_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/mcp/servers")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json_body(response).await;
        assert!(body["servers"].is_array());
        assert!(body["servers"].as_array().unwrap().is_empty());
        // When the config file is missing the handler returns an empty path.
        assert_eq!(body["config_path"].as_str().unwrap(), "");
    }

    #[tokio::test]
    async fn test_get_mcp_server_not_found_when_config_missing() {
        let _guard = set_temp_mcp_config_path();
        let state = test_state().await;
        let app = crate::server::create_api_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/mcp/servers/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        let body = read_json_body(response).await;
        assert!(
            body["error"]
                .as_str()
                .unwrap()
                .contains("Failed to load MCP config")
        );
    }

    #[tokio::test]
    async fn test_update_mcp_server_creates_entry() {
        let _guard = set_temp_mcp_config_path();
        let state = test_state().await;
        let app = crate::server::create_api_router(state);

        let payload = serde_json::json!({
            "command": "npx",
            "args": ["-y", "@modelcontextprotocol/server-filesystem", "."],
            "disabled": false,
            "transport": "stdio"
        });

        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/mcp/servers/filesystem")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json_body(response).await;
        assert_eq!(body["saved"], true);

        // Verify the file was written and can be loaded.
        let path = resolve_mcp_config_path().unwrap();
        let config = clarity_core::mcp::config::McpConfig::load(&path).unwrap();
        let entry = config.servers.get("filesystem").unwrap();
        assert_eq!(entry.command, "npx");
        assert_eq!(
            entry.args,
            vec!["-y", "@modelcontextprotocol/server-filesystem", "."]
        );
        assert!(!entry.disabled);
        assert_eq!(entry.transport.as_deref().unwrap(), "stdio");
    }

    #[tokio::test]
    async fn test_get_mcp_server_found() {
        let _guard = set_temp_mcp_config_path();
        let state = test_state().await;
        let app = crate::server::create_api_router(state);

        let payload = serde_json::json!({
            "command": "npx",
            "args": ["-y", "server-git"],
            "disabled": true,
            "transport": "stdio"
        });
        let create_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/mcp/servers/git")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_response.status(), StatusCode::OK);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/mcp/servers/git")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json_body(response).await;
        assert_eq!(body["name"], "git");
        assert_eq!(body["command"], "npx");
        assert_eq!(body["args"], serde_json::json![["-y", "server-git"]]);
        assert_eq!(body["disabled"], true);
        assert_eq!(body["transport"], "stdio");
    }

    #[tokio::test]
    async fn test_delete_mcp_server() {
        let _guard = set_temp_mcp_config_path();
        let state = test_state().await;
        let app = crate::server::create_api_router(state);

        // Create a server to delete.
        let payload = serde_json::json!({"command": "echo"});
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/mcp/servers/delete-me")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Delete it.
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/mcp/servers/delete-me")
                    .method("DELETE")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json_body(response).await;
        assert_eq!(body["deleted"], true);

        // Confirm it is gone.
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/mcp/servers/delete-me")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_update_mcp_server_preserves_existing_fields() {
        let _guard = set_temp_mcp_config_path();
        let state = test_state().await;
        let app = crate::server::create_api_router(state);

        // Create with a full entry.
        let payload = serde_json::json!({
            "command": "npx",
            "args": ["-y", "server"],
            "disabled": false,
            "transport": "stdio",
            "url": "http://old.example.com",
            "headers": {"X-Old": "old"},
            "env": {"OLD": "old"}
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/mcp/servers/patchable")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // Partial update: only command and disabled.
        let patch = serde_json::json!({"command": "pnpm", "disabled": true});
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/mcp/servers/patchable")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(patch.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/mcp/servers/patchable")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json_body(response).await;
        assert_eq!(body["command"], "pnpm");
        assert_eq!(body["args"], serde_json::json![["-y", "server"]]);
        assert_eq!(body["disabled"], true);
        assert_eq!(body["transport"], "stdio");
        assert_eq!(body["url"], "http://old.example.com");
    }

    #[tokio::test]
    async fn test_list_mcp_servers_returns_populated_config() {
        let _guard = set_temp_mcp_config_path();
        let state = test_state().await;
        let app = crate::server::create_api_router(state);

        let payload = serde_json::json!({
            "command": "npx",
            "args": ["-y", "server"]
        });
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/mcp/servers/listable")
                    .method("POST")
                    .header("content-type", "application/json")
                    .body(Body::from(payload.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/api/mcp/servers")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = read_json_body(response).await;
        let servers = body["servers"].as_array().unwrap();
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0]["name"], "listable");
        assert_eq!(servers[0]["command"], "npx");
    }
}
