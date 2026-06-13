use axum::{
    extract::{Json, State},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use clarity_core::config::{ConfigHealthIssue, ConfigRollbackPoint};
use clarity_llm::LlmFactory;
use serde::{Deserialize, Serialize};

use std::sync::Arc;
use tracing::{error, info};

use crate::handlers::AgentHandle;
use crate::server::AppState;

// ==================== Admin API ====================

#[derive(Serialize)]
pub(crate) struct StatsResponse {
    pub active_sessions: usize,
    pub total_requests: u64,
    pub uptime_seconds: u64,
}

pub(crate) async fn admin_stats(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let active_sessions = state.session_store.session_count().await.unwrap_or(0);
    let total_requests = state.session_store.total_requests().await.unwrap_or(0);
    let uptime_seconds = (Utc::now() - state.started_at).num_seconds() as u64;
    let stats = StatsResponse {
        active_sessions,
        total_requests,
        uptime_seconds,
    };
    (StatusCode::OK, Json(stats))
}

#[derive(Serialize)]
pub(crate) struct ToolsResponse {
    pub tools: Vec<ToolInfo>,
}

#[derive(Serialize)]
pub(crate) struct ToolInfo {
    pub name: String,
    pub description: String,
    pub enabled: bool,
}

pub(crate) async fn admin_tools(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let tools = match state.registry().get_tool_schemas() {
        Ok(schemas) => {
            if let Some(functions) = schemas.as_array() {
                functions
                    .iter()
                    .filter_map(|f| {
                        let name = f.get("function")?.get("name")?.as_str()?.to_string();
                        let description =
                            f.get("function")?.get("description")?.as_str()?.to_string();
                        Some(ToolInfo {
                            name,
                            description,
                            enabled: true,
                        })
                    })
                    .collect()
            } else {
                vec![]
            }
        }
        Err(e) => {
            error!("Failed to get tool schemas: {}", e);
            vec![]
        }
    };

    (StatusCode::OK, Json(ToolsResponse { tools }))
}

// ==================== Admin: List Available Models ====================

#[derive(Serialize)]
pub(crate) struct ModelsResponse {
    pub models: Vec<ModelInfo>,
}

#[derive(Serialize)]
pub(crate) struct ModelInfo {
    pub alias: String,
    pub provider: String,
    pub model_id: String,
    pub protocol: String,
}

pub(crate) async fn admin_models() -> impl IntoResponse {
    let registry = match crate::handlers::config::load_model_registry().await {
        Ok(r) => r,
        Err(e) => {
            tracing::warn!("Failed to load model registry: {}", e);
            return (StatusCode::OK, Json(ModelsResponse { models: vec![] }));
        }
    };

    let models: Vec<ModelInfo> = registry
        .list_models()
        .into_iter()
        .map(|m| {
            let protocol = registry
                .get_provider(&m.provider)
                .map(|p| format!("{:?}", p.protocol))
                .unwrap_or_else(|| "unknown".into());
            ModelInfo {
                alias: m.alias.clone(),
                provider: m.provider.clone(),
                model_id: m.model_id.clone(),
                protocol,
            }
        })
        .collect();

    (StatusCode::OK, Json(ModelsResponse { models }))
}

// ==================== Admin: Approval Mode ====================

#[derive(Deserialize)]
pub(crate) struct SetApprovalModeRequest {
    pub mode: String,
}

#[derive(Serialize)]
pub(crate) struct ApprovalModeResponse {
    pub mode: String,
}

pub(crate) async fn admin_set_approval_mode(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SetApprovalModeRequest>,
) -> Response {
    use clarity_core::approval::ApprovalMode;

    let mode = match req.mode.to_lowercase().as_str() {
        "interactive" => ApprovalMode::Interactive,
        "yolo" => ApprovalMode::Yolo,
        "plan" => ApprovalMode::Plan,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Invalid mode. Use: interactive, yolo, plan"})),
            )
                .into_response();
        }
    };

    state.set_approval_mode(mode);
    let resp = ApprovalModeResponse {
        mode: format!("{:?}", mode).to_lowercase(),
    };
    (StatusCode::OK, Json(resp)).into_response()
}

pub(crate) async fn admin_get_approval_mode(
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let mode = state.approval_mode();
    let resp = ApprovalModeResponse {
        mode: format!("{:?}", mode).to_lowercase(),
    };
    (StatusCode::OK, Json(resp))
}

// ==================== Admin: Switch Provider ====================

#[derive(Deserialize)]
pub(crate) struct SwitchProviderRequest {
    pub provider: String,
    #[serde(default)]
    pub args: Option<Vec<String>>,
}

#[derive(Serialize)]
pub(crate) struct SwitchProviderResponse {
    pub provider: String,
    pub message: String,
}

static MESH_PROVIDER: std::sync::OnceLock<Arc<clarity_llm::mesh::MeshLlmProvider>> =
    std::sync::OnceLock::new();

pub(crate) async fn admin_switch_provider(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SwitchProviderRequest>,
) -> impl IntoResponse {
    info!("Admin: switching provider to '{}'", req.provider);

    // MCP LLM prefix: "mcp:<command>"
    let provider_raw = req.provider.clone();
    if let Some(cmd) = provider_raw.strip_prefix("mcp:") {
        let cmd = cmd.to_string();
        let args = req.args.unwrap_or_default();
        info!("MCP LLM switch: command={}, args={:?}", cmd, args);
        match clarity_llm::mcp_llm_provider::McpLlmProvider::connect_stdio(&cmd, &args).await {
            Ok(provider) => {
                state.set_llm(Arc::new(provider));
                state.set_provider_label(format!("mcp:{}", cmd));
                let resp = SwitchProviderResponse {
                    provider: provider_raw,
                    message: format!("Switched to MCP LLM server: {}", cmd),
                };
                return (StatusCode::OK, Json(resp));
            }
            Err(e) => {
                error!("Failed to connect MCP LLM: {}", e);
                let resp = SwitchProviderResponse {
                    provider: provider_raw,
                    message: format!("Failed to connect MCP LLM: {}", e),
                };
                return (StatusCode::BAD_REQUEST, Json(resp));
            }
        }
    }

    let names: Vec<String> = if req.provider.trim() == "mesh" {
        std::env::var("CLARITY_MESH_PROVIDERS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else if req.provider.contains(',') {
        req.provider
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    } else {
        vec![req.provider.clone()]
    };

    if names.len() == 1 {
        // Single provider — try alias registry (with ReliableProvider wrap) first, then legacy factory.
        let name = names[0].clone();
        match crate::handlers::config::build_reliable_provider_for_alias(&name).await {
            Ok(new_llm) => {
                let _ = crate::handlers::config::save_active_alias(&name).await;
                state.set_llm(new_llm);
                state.set_provider_label(&name);
                let resp = SwitchProviderResponse {
                    provider: name.clone(),
                    message: "Provider switched successfully".to_string(),
                };
                return (StatusCode::OK, Json(resp));
            }
            Err(alias_err) => {
                tracing::debug!("Alias registry miss for '{}': {}", name, alias_err);
            }
        }

        match LlmFactory::create(&name).await {
            Ok(new_llm) => {
                state.set_llm(Arc::from(new_llm));
                state.set_provider_label(&name);
                let resp = SwitchProviderResponse {
                    provider: name.clone(),
                    message: "Provider switched successfully".to_string(),
                };
                (StatusCode::OK, Json(resp))
            }
            Err(e) => {
                error!("Failed to switch provider: {}", e);
                let resp = SwitchProviderResponse {
                    provider: name.clone(),
                    message: format!("Failed to create provider: {}", e),
                };
                (StatusCode::BAD_REQUEST, Json(resp))
            }
        }
    } else {
        // Multi-provider mesh
        match clarity_llm::mesh::MeshLlmProvider::from_names(names.clone()).await {
            Ok(mesh) => {
                let mesh = Arc::new(mesh);
                let _ = MESH_PROVIDER.set(mesh.clone());
                state.set_llm(mesh);
                state.set_provider_label(format!("mesh:{}", names.join(",")));
                let resp = SwitchProviderResponse {
                    provider: req.provider,
                    message: format!("Switched to mesh with providers: {:?}", names),
                };
                (StatusCode::OK, Json(resp))
            }
            Err(e) => {
                error!("Failed to create mesh: {}", e);
                let resp = SwitchProviderResponse {
                    provider: req.provider,
                    message: format!("Failed to create mesh: {}", e),
                };
                (StatusCode::BAD_REQUEST, Json(resp))
            }
        }
    }
}

#[derive(Serialize)]
pub(crate) struct MeshStatusResponse {
    pub active: bool,
    pub providers: Vec<String>,
    pub circuits: std::collections::HashMap<String, String>,
}

pub(crate) async fn admin_mesh_status() -> impl IntoResponse {
    if let Some(mesh) = MESH_PROVIDER.get() {
        let circuits = mesh
            .circuit_states()
            .into_iter()
            .map(|(k, v)| (k, format!("{:?}", v).to_lowercase()))
            .collect();
        let resp = MeshStatusResponse {
            active: true,
            providers: mesh.provider_names(),
            circuits,
        };
        (StatusCode::OK, Json(resp))
    } else {
        let resp = MeshStatusResponse {
            active: false,
            providers: vec![],
            circuits: std::collections::HashMap::new(),
        };
        (StatusCode::OK, Json(resp))
    }
}

// ==================== Admin: Config Health ====================

/// Serializable configuration health report.
#[derive(Serialize)]
pub(crate) struct ConfigHealthResponse {
    pub healthy: bool,
    pub layers: Vec<ConfigLayerInfo>,
    pub issues: Vec<ConfigHealthIssue>,
    pub rollback_points: Vec<ConfigRollbackPoint>,
}

/// Serializable source layer summary.
#[derive(Serialize)]
pub(crate) struct ConfigLayerInfo {
    pub source: String,
    pub active: bool,
}

pub(crate) async fn admin_config_health() -> Response {
    match clarity_core::config::Config::load_with_health() {
        Ok((_, health)) => {
            let layers = health
                .layers()
                .iter()
                .map(|(s, active)| ConfigLayerInfo {
                    source: s.to_string(),
                    active: *active,
                })
                .collect();
            let resp = ConfigHealthResponse {
                healthy: health.is_healthy(),
                layers,
                issues: health.issues().to_vec(),
                rollback_points: health.rollback_points().to_vec(),
            };
            (StatusCode::OK, Json(resp)).into_response()
        }
        Err(e) => {
            error!("Failed to load configuration for health check: {}", e);
            let resp = serde_json::json!({
                "healthy": false,
                "error": e.to_string(),
            });
            (StatusCode::INTERNAL_SERVER_ERROR, Json(resp)).into_response()
        }
    }
}

pub(crate) async fn admin_validate_config() -> Response {
    match clarity_core::config::Config::load_with_health() {
        Ok((_, health)) => {
            let status = if health.is_healthy() {
                StatusCode::OK
            } else {
                StatusCode::UNPROCESSABLE_ENTITY
            };
            let resp = serde_json::json!({
                "healthy": health.is_healthy(),
                "issue_count": health.issues().len(),
                "issues": health.issues(),
            });
            (status, Json(resp)).into_response()
        }
        Err(e) => {
            error!("Failed to validate configuration: {}", e);
            let resp = serde_json::json!({
                "healthy": false,
                "error": e.to_string(),
            });
            (StatusCode::INTERNAL_SERVER_ERROR, Json(resp)).into_response()
        }
    }
}
