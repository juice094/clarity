use axum::{
    extract::Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::{Deserialize, Serialize};

use tracing::warn;

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
    let mcp_path = clarity_core::mcp::config::default_config_path().ok();
    match clarity_core::mcp::config::McpConfig::load_default() {
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
    match clarity_core::mcp::config::McpConfig::load_default() {
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
    let default_path = clarity_core::mcp::config::default_config_path();
    let path = match default_path {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
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
    let default_path = clarity_core::mcp::config::default_config_path();
    let path = match default_path {
        Ok(p) => p,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response()
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
