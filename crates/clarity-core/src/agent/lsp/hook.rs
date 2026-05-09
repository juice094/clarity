//! `AgentHook` implementation that syncs file edits to an LSP server
//! and injects diagnostics into the conversation.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc};
use parking_lot::RwLock;

use crate::agent::hooks::{AgentHook, HookResult};
use crate::agent::lsp::client::LspClient;
use crate::agent::lsp::config::LspClientConfig;
use clarity_llm::api::Message;
use crate::types::ToolCall;
use serde_json::Value;
use tracing::{debug, info, warn};

/// Hook that intercepts file-edit tool calls and maintains an LSP
/// diagnostic channel into the agent conversation.
pub struct LspHook {
    client: Arc<LspClient>,
    /// URIs that have already been `didOpen`-ed.
    opened_uris: RwLock<HashSet<String>>,
    /// URIs whose content changed since the last `on_llm_input`.
    pending_uris: RwLock<Vec<String>>,
    /// Per-URI document version counter for `didChange`.
    uri_versions: RwLock<HashMap<String, i32>>,
    /// File extensions this hook cares about (empty = all).
    file_extensions: Vec<String>,
    /// Workspace directory for resolving relative paths.
    working_dir: std::path::PathBuf,
}

impl LspHook {
    /// Attempt to create an `LspHook`. Returns `None` if the LSP server
    /// cannot be started or the config is disabled.
    pub async fn try_new(config: &LspClientConfig, working_dir: &std::path::Path) -> Option<Self> {
        if !config.enabled {
            return None;
        }

        let root_uri = config.root_uri.clone().or_else(|| {
            let path_str = working_dir.to_string_lossy().replace('\\', "/");
            if path_str.starts_with('/') {
                Some(format!("file://{}", path_str))
            } else {
                Some(format!("file:/// {}", path_str).replace(' ', ""))
            }
        })?;

        match LspClient::spawn(&config.command, &config.args, Some(root_uri)).await {
            Ok(client) => {
                info!("LSP hook active: {}", config.command);
                Some(Self {
                    client: Arc::new(client),
                    opened_uris: RwLock::new(HashSet::new()),
                    pending_uris: RwLock::new(Vec::new()),
                    uri_versions: RwLock::new(HashMap::new()),
                    file_extensions: config.file_extensions.clone(),
                    working_dir: working_dir.to_path_buf(),
                })
            }
            Err(e) => {
                warn!(
                    "LSP hook disabled — failed to start '{}': {}",
                    config.command, e
                );
                None
            }
        }
    }

    /// Resolve a possibly-relative path against `working_dir` and convert
    /// to a `file://` URI.
    fn path_to_uri(&self, path_str: &str) -> Option<String> {
        path_to_uri_impl(&self.working_dir, path_str)
    }

    /// Check whether a path matches one of the configured extensions.
    fn matches_extension(&self, path: &std::path::Path) -> bool {
        matches_extension_impl(path, &self.file_extensions)
    }

    /// Map a file extension to an LSP `languageId`.
    fn language_id(ext: &str) -> &str {
        match ext {
            "rs" => "rust",
            "ts" => "typescript",
            "tsx" => "typescriptreact",
            "js" => "javascript",
            "jsx" => "javascriptreact",
            "py" => "python",
            "go" => "go",
            "java" => "java",
            "c" => "c",
            "cpp" | "cc" | "cxx" => "cpp",
            "h" | "hpp" => "cpp",
            "json" => "json",
            "yaml" | "yml" => "yaml",
            "toml" => "toml",
            "md" => "markdown",
            _ => ext,
        }
    }
}

pub(crate) fn path_to_uri_impl(working_dir: &std::path::Path, path_str: &str) -> Option<String> {
    let path = std::path::Path::new(path_str);
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        working_dir.join(path)
    };
    let normalized = abs.to_string_lossy().replace('\\', "/");
    if normalized.starts_with('/') {
        Some(format!("file://{}", normalized))
    } else {
        // Windows drive letter
        Some(format!("file:/// {}", normalized).replace(' ', ""))
    }
}

pub(crate) fn uri_to_path_impl(uri: &str) -> Option<std::path::PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    // Windows URIs: file:///C:/foo  ->  /C:/foo  ->  C:/foo
    let rest = if rest.starts_with('/')
        && rest.len() >= 3
        && rest.as_bytes()[1].is_ascii_alphabetic()
        && rest.as_bytes()[2] == b':'
    {
        &rest[1..]
    } else {
        rest
    };
    Some(std::path::PathBuf::from(rest))
}

pub(crate) fn matches_extension_impl(path: &std::path::Path, extensions: &[String]) -> bool {
    if extensions.is_empty() {
        return true;
    }
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    extensions.iter().any(|e| e == ext)
}

#[async_trait::async_trait]
impl AgentHook for LspHook {
    async fn before_tool_call(&self, _tool_call: &mut ToolCall) -> HookResult {
        HookResult::Continue
    }

    async fn after_tool_call(&self, tool_call: &ToolCall, result: &mut Value) {
        if tool_call.function.name != "file_write" && tool_call.function.name != "file_edit" {
            return;
        }

        // Skip failed edits
        if result.get("error").is_some() {
            return;
        }

        let args: Value = match serde_json::from_str(&tool_call.function.arguments) {
            Ok(v) => v,
            Err(_) => return,
        };

        let path_str = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return,
        };

        let path = std::path::Path::new(path_str);
        if !self.matches_extension(path) {
            return;
        }

        let uri = match self.path_to_uri(path_str) {
            Some(u) => u,
            None => return,
        };

        debug!("LSP hook queued URI for sync: {}", uri);
        let mut pending = self.pending_uris.write();
        if !pending.contains(&uri) {
            pending.push(uri);
        }
    }

    async fn on_llm_input(&self, messages: &mut Vec<Message>) {
        // 1. Drain pending URIs
        let uris: Vec<String> = {
            let mut pending = self.pending_uris.write();
            std::mem::take(&mut *pending)
        };

        if uris.is_empty() {
            // Still try to inject any already-buffered diagnostics
        }

        // 2. Send didOpen / didChange for each pending URI
        for uri in &uris {
            let path = match uri_to_path_impl(uri) {
                Some(p) => p,
                None => continue,
            };

            let text = match tokio::fs::read_to_string(&path).await {
                Ok(t) => t,
                Err(e) => {
                    debug!("LSP hook failed to read file for {}: {}", uri, e);
                    continue;
                }
            };

            let is_new = {
                let mut opened = self.opened_uris.write();
                if opened.contains(uri) {
                    false
                } else {
                    opened.insert(uri.clone());
                    true
                }
            };

            if is_new {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                let lang_id = Self::language_id(ext);
                if let Err(e) = self.client.did_open(uri, lang_id, &text).await {
                    warn!("LSP didOpen failed for {}: {}", uri, e);
                }
            } else {
                let version = {
                    let mut versions = self.uri_versions.write();
                    let v = versions.get(uri).copied().unwrap_or(1) + 1;
                    versions.insert(uri.clone(), v);
                    v
                };
                if let Err(e) = self.client.did_change(uri, version, &text).await {
                    warn!("LSP didChange failed for {}: {}", uri, e);
                }
            }
        }

        // 3. Drain buffered diagnostics and inject into messages
        let diagnostics = self.client.drain_diagnostics().await;
        if !diagnostics.is_empty() {
            let mut lines =
                vec!["Language server diagnostics for recently edited files:".to_string()];
            for params in &diagnostics {
                let path = uri_to_path_impl(&params.uri)
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| params.uri.clone());
                if params.diagnostics.is_empty() {
                    lines.push(format!("\n{}: no diagnostics", path));
                    continue;
                }
                lines.push(format!("\n{}:", path));
                for d in &params.diagnostics {
                    let severity = match d.severity {
                        Some(1) => "Error",
                        Some(2) => "Warning",
                        Some(3) => "Info",
                        Some(4) => "Hint",
                        _ => "Diagnostic",
                    };
                    lines.push(format!("  [{}] {}", severity, d.message));
                }
            }
            let text = lines.join("\n");
            messages.push(Message::system(text));
            debug!(
                "LSP hook injected diagnostics for {} file(s)",
                diagnostics.len()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_to_uri_unix_absolute() {
        let wd = std::path::PathBuf::from("/home/user/project");
        assert_eq!(
            path_to_uri_impl(&wd, "/home/user/project/src/main.rs"),
            Some("file:///home/user/project/src/main.rs".to_string())
        );
    }

    #[test]
    fn test_path_to_uri_relative() {
        let wd = std::path::PathBuf::from("/home/user/project");
        assert_eq!(
            path_to_uri_impl(&wd, "src/main.rs"),
            Some("file:///home/user/project/src/main.rs".to_string())
        );
    }

    #[test]
    fn test_path_to_uri_windows() {
        let wd = std::path::PathBuf::from("C:\\project");
        assert_eq!(
            path_to_uri_impl(&wd, "src\\main.rs"),
            Some("file:///C:/project/src/main.rs".to_string())
        );
    }

    #[test]
    fn test_uri_to_path_unix() {
        assert_eq!(
            uri_to_path_impl("file:///home/user/main.rs"),
            Some(std::path::PathBuf::from("/home/user/main.rs"))
        );
    }

    #[test]
    fn test_uri_to_path_windows() {
        assert_eq!(
            uri_to_path_impl("file:///C:/project/main.rs"),
            Some(std::path::PathBuf::from("C:/project/main.rs"))
        );
    }

    #[test]
    fn test_matches_extension_empty_list() {
        assert!(matches_extension_impl(std::path::Path::new("foo.rs"), &[]));
    }

    #[test]
    fn test_matches_extension_match() {
        assert!(matches_extension_impl(
            std::path::Path::new("foo.rs"),
            &["rs".to_string(), "ts".to_string()]
        ));
        assert!(!matches_extension_impl(
            std::path::Path::new("foo.py"),
            &["rs".to_string(), "ts".to_string()]
        ));
    }

    #[test]
    fn test_language_id() {
        assert_eq!(LspHook::language_id("rs"), "rust");
        assert_eq!(LspHook::language_id("ts"), "typescript");
        assert_eq!(LspHook::language_id("tsx"), "typescriptreact");
        assert_eq!(LspHook::language_id("py"), "python");
        assert_eq!(LspHook::language_id("go"), "go");
        assert_eq!(LspHook::language_id("json"), "json");
        assert_eq!(LspHook::language_id("unknown"), "unknown");
    }
}
