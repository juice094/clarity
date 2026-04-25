use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GuiSettings {
    pub model: String,
    pub provider: String,
    pub approval_mode: String,
    pub theme: String,
    #[serde(default)]
    pub local_model_path: Option<String>,
    /// Custom TCP probe endpoint for network reachability checks.
    /// Format: "host:port" (e.g. "1.1.1.1:443").
    /// When None the default "1.1.1.1:443" is used.
    #[serde(default)]
    pub network_probe_url: Option<String>,
}

impl GuiSettings {
    fn config_path() -> PathBuf {
        // Cross-platform MVP: APPDATA (Windows) → HOME/.config (Unix) → fallback cwd
        if let Ok(appdata) = std::env::var("APPDATA") {
            let mut path = PathBuf::from(appdata);
            path.push("clarity");
            path.push("gui-settings.json");
            return path;
        }
        if let Ok(home) = std::env::var("HOME") {
            let mut path = PathBuf::from(home);
            path.push(".config");
            path.push("clarity");
            path.push("gui-settings.json");
            return path;
        }
        PathBuf::from("gui-settings.json")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(settings) = serde_json::from_str(&content) {
                return settings;
            }
        }
        Self::default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let content = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, content).map_err(|e| e.to_string())
    }
}

impl Default for GuiSettings {
    fn default() -> Self {
        Self {
            model: "gpt-4o".into(),
            provider: "openai".into(),
            approval_mode: "interactive".into(),
            theme: "dark".into(),
            local_model_path: None,
            network_probe_url: None,
        }
    }
}

/// Scan known directories for `.gguf` model files.
/// Returns `Vec<(full_path, file_name)>` sorted by file name.
fn scan_local_models() -> Vec<(String, String)> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    // Helper to collect .gguf files from a directory
    fn add_ggufs_from_dir(dir: &PathBuf, results: &mut Vec<(String, String)>, seen: &mut HashSet<String>) {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.is_file()
                    && path
                        .extension()
                        .and_then(|ext| ext.to_str())
                        .map(|ext| ext.eq_ignore_ascii_case("gguf"))
                        .unwrap_or(false)
                {
                    let path_str = path.to_string_lossy().into_owned();
                    if seen.insert(path_str.clone()) {
                        let name = path
                            .file_name()
                            .map(|s| s.to_string_lossy().into_owned())
                            .unwrap_or_default();
                        results.push((path_str, name));
                    }
                }
            }
        }
    }

    // 1. Explicit env var (may be a file or directory)
    if let Ok(path_str) = std::env::var("CLARITY_LOCAL_MODEL_PATH") {
        let p = PathBuf::from(&path_str);
        if p.is_dir() {
            add_ggufs_from_dir(&p, &mut results, &mut seen);
        } else if p.is_file()
            && p.extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("gguf"))
                .unwrap_or(false)
        {
            let name = p
                .file_name()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default();
            if seen.insert(path_str.clone()) {
                results.push((path_str, name));
            }
        }
    }

    // 2. Auto-discover in ~/models/
    if let Some(home) = dirs::home_dir() {
        let models_dir = home.join("models");
        if models_dir.is_dir() {
            add_ggufs_from_dir(&models_dir, &mut results, &mut seen);
        }
    }

    results.sort_by(|a, b| a.1.cmp(&b.1));
    results
}

#[tauri::command]
pub fn get_settings(state: tauri::State<crate::AppState>) -> GuiSettings {
    state.cached_settings.lock().unwrap().clone()
}

/// Validate that a network probe URL looks like `host:port`.
fn validate_probe_url(probe: &str) -> Result<(), String> {
    if probe.is_empty() {
        return Ok(());
    }
    // Split from the right so IPv6 literals (if ever supported) won't break us.
    let parts: Vec<&str> = probe.rsplitn(2, ':').collect();
    if parts.len() != 2 {
        return Err(
            "Probe URL must contain a port, e.g. 1.1.1.1:443".to_string(),
        );
    }
    let port: u16 = parts[0]
        .parse()
        .map_err(|_| "Invalid port in probe URL".to_string())?;
    if port == 0 {
        return Err("Port cannot be 0".to_string());
    }
    Ok(())
}

#[tauri::command]
pub fn save_settings(
    settings: GuiSettings,
    state: tauri::State<crate::AppState>,
) -> Result<(), String> {
    if let Some(ref probe) = settings.network_probe_url {
        validate_probe_url(probe)?;
    }
    settings.save()?;
    let mut guard = state.cached_settings.lock().unwrap();
    *guard = settings;
    Ok(())
}

#[tauri::command]
pub fn get_available_models() -> Vec<(String, String, Vec<String>)> {
    let local_models = scan_local_models();
    let local_model_names: Vec<String> = if local_models.is_empty() {
        vec!["No models found — place .gguf in ~/models/".into()]
    } else {
        local_models.into_iter().map(|(_, name)| name).collect()
    };

    vec![
        (
            "openai".into(),
            "OpenAI".into(),
            vec!["gpt-4o".into(), "gpt-4o-mini".into(), "o3-mini".into()],
        ),
        (
            "anthropic".into(),
            "Anthropic".into(),
            vec!["claude-3-sonnet".into(), "claude-3-opus".into()],
        ),
        (
            "kimi".into(),
            "Kimi".into(),
            vec!["kimi-k2-07132k".into(), "kimi-latest".into()],
        ),
        (
            "ollama".into(),
            "Ollama".into(),
            vec!["llama3.2".into(), "qwen2.5".into()],
        ),
        (
            "local".into(),
            "Local (GGUF)".into(),
            local_model_names,
        ),
    ]
}

#[tauri::command]
pub fn get_local_models() -> Vec<(String, String)> {
    scan_local_models()
}

#[tauri::command]
pub fn set_approval_mode(mode: String, state: tauri::State<crate::AppState>) -> Result<(), String> {
    let mode = match mode.as_str() {
        "interactive" => clarity_core::approval::ApprovalMode::Interactive,
        "yolo" => clarity_core::approval::ApprovalMode::Yolo,
        "plan" => clarity_core::approval::ApprovalMode::Plan,
        _ => return Err(format!("Invalid approval mode: {}", mode)),
    };
    state.agent.set_approval_mode(mode);
    Ok(())
}

#[tauri::command]
pub fn get_approval_modes() -> Vec<(String, String)> {
    vec![
        (
            "interactive".into(),
            "Interactive — Approve each tool call".into(),
        ),
        ("yolo".into(), "Yolo — Auto-approve all".into()),
        (
            "plan".into(),
            "Plan — Review plan before execution".into(),
        ),
    ]
}
