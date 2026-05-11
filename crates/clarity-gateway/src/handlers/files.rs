use axum::{
    extract::{Json, Query, State},
    http::StatusCode,
    response::IntoResponse,
};
use clarity_core::activity::WindowActivity;
use serde::Deserialize;
use serde_json::json;
use std::path::{Path, PathBuf};

use std::sync::Arc;

use crate::server::AppState;

// ==================== File System API ====================

#[derive(Debug, Deserialize)]
pub(crate) struct FileTreeParams {
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FileReadParams {
    pub path: String,
    pub offset: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FileWriteBody {
    pub path: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FileGlobParams {
    pub pattern: String,
}

pub(super) fn sanitize_path(raw: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(raw);
    let abs = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let canonical = abs
        .canonicalize()
        .map_err(|e| format!("Invalid path: {}", e))?;

    // Security: restrict to working directory
    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .canonicalize()
        .unwrap_or_else(|_| PathBuf::from("."));

    if !canonical.starts_with(&cwd) {
        return Err("Path is outside working directory".to_string());
    }

    Ok(canonical)
}

pub(super) fn is_sensitive_path(path: &Path) -> bool {
    let path_str = path.to_string_lossy().to_lowercase();
    [
        ".env",
        "id_rsa",
        "id_ed25519",
        ".ssh",
        ".p12",
        ".pfx",
        ".htpasswd",
        "secrets",
        "credentials",
        "token",
        "api_key",
        "private_key",
        "password",
        "passwd",
    ]
    .iter()
    .any(|s| path_str.contains(s))
}

fn build_tree<'a>(
    path: &'a Path,
    root: &'a Path,
    depth: usize,
) -> std::pin::Pin<
    Box<dyn std::future::Future<Output = Result<serde_json::Value, String>> + Send + 'a>,
> {
    Box::pin(async move {
        if depth > 10 {
            return Ok(json!({"name": "...", "type": "directory", "path": "", "children": []}));
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        if path.is_file() {
            let meta = tokio::fs::metadata(path).await.map_err(|e| e.to_string())?;
            Ok(json!({
                "name": name,
                "type": "file",
                "path": rel,
                "size": meta.len(),
            }))
        } else if path.is_dir() {
            let mut children = Vec::new();
            let mut entries = tokio::fs::read_dir(path).await.map_err(|e| e.to_string())?;
            while let Some(entry) = entries.next_entry().await.map_err(|e| e.to_string())? {
                let child_path = entry.path();
                // Skip hidden files/directories
                if child_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with('.'))
                    .unwrap_or(false)
                {
                    continue;
                }
                if let Ok(child) = build_tree(&child_path, root, depth + 1).await {
                    children.push(child);
                }
            }
            children.sort_by(|a, b| {
                let a_type = a["type"].as_str().unwrap_or("");
                let b_type = b["type"].as_str().unwrap_or("");
                let a_name = a["name"].as_str().unwrap_or("");
                let b_name = b["name"].as_str().unwrap_or("");
                a_type.cmp(b_type).reverse().then(a_name.cmp(b_name))
            });
            Ok(json!({
                "name": name,
                "type": "directory",
                "path": rel,
                "children": children,
            }))
        } else {
            Err("Unknown file type".to_string())
        }
    })
}

pub(crate) async fn file_tree(
    Query(params): Query<FileTreeParams>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let root = match &params.path {
        Some(p) => match sanitize_path(p) {
            Ok(path) => path,
            Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": e}))),
        },
        None => std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
    };

    if !root.is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Not a directory"})),
        );
    }

    match build_tree(&root, &root, 0).await {
        Ok(tree) => {
            state.activity_logger.log_window(WindowActivity {
                timestamp: chrono::Utc::now().to_rfc3339(),
                activity_type: "file_tree".to_string(),
                topic: format!("浏览目录: {}", root.display()),
                tools_used: vec!["file_tree".to_string()],
                files_involved: vec![],
                conclusion: "".to_string(),
            });
            (StatusCode::OK, Json(json!({"tree": tree})))
        }
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))),
    }
}

pub(crate) async fn file_read(
    Query(params): Query<FileReadParams>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let path = match sanitize_path(&params.path) {
        Ok(p) => p,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": e}))),
    };

    if path.is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "Path is a directory"})),
        );
    }

    if is_sensitive_path(&path) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Access to sensitive file denied"})),
        );
    }

    let mut args = json!({"path": path.display().to_string()});
    if let Some(offset) = params.offset {
        args["offset"] = json!(offset);
    }
    if let Some(limit) = params.limit {
        args["limit"] = json!(limit);
    }

    let ctx = clarity_core::tools::ToolContext::new();
    let registry = clarity_core::registry::ToolRegistry::with_builtin_tools();
    match registry.execute("file_read", args, ctx).await {
        Ok(result) => {
            state.activity_logger.log_window(WindowActivity {
                timestamp: chrono::Utc::now().to_rfc3339(),
                activity_type: "file_read".to_string(),
                topic: format!("读取文件: {}", params.path),
                tools_used: vec!["file_read".to_string()],
                files_involved: vec![params.path.clone()],
                conclusion: "".to_string(),
            });
            (StatusCode::OK, Json(result))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        ),
    }
}

pub(crate) async fn file_write(
    State(state): State<Arc<AppState>>,
    Json(body): Json<FileWriteBody>,
) -> impl IntoResponse {
    let path = match sanitize_path(&body.path) {
        Ok(p) => p,
        Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": e}))),
    };

    if is_sensitive_path(&path) {
        return (
            StatusCode::FORBIDDEN,
            Json(json!({"error": "Writing to sensitive path denied"})),
        );
    }

    let args = json!({
        "path": path.display().to_string(),
        "content": body.content,
    });

    let ctx = clarity_core::tools::ToolContext::new();
    let registry = clarity_core::registry::ToolRegistry::with_builtin_tools();
    match registry.execute("file_write", args, ctx).await {
        Ok(result) => {
            state.activity_logger.log_window(WindowActivity {
                timestamp: chrono::Utc::now().to_rfc3339(),
                activity_type: "file_write".to_string(),
                topic: format!("写入文件: {}", body.path),
                tools_used: vec!["file_write".to_string()],
                files_involved: vec![body.path.clone()],
                conclusion: "".to_string(),
            });
            (StatusCode::OK, Json(result))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        ),
    }
}

pub(crate) async fn file_glob(
    Query(params): Query<FileGlobParams>,
    State(state): State<Arc<AppState>>,
) -> impl IntoResponse {
    let args = json!({"pattern": params.pattern});
    let ctx = clarity_core::tools::ToolContext::new();
    let registry = clarity_core::registry::ToolRegistry::with_builtin_tools();
    match registry.execute("glob", args, ctx).await {
        Ok(result) => {
            state.activity_logger.log_window(WindowActivity {
                timestamp: chrono::Utc::now().to_rfc3339(),
                activity_type: "file_glob".to_string(),
                topic: format!("搜索文件: {}", params.pattern),
                tools_used: vec!["glob".to_string()],
                files_involved: vec![],
                conclusion: "".to_string(),
            });
            (StatusCode::OK, Json(result))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": e.to_string()})),
        ),
    }
}
