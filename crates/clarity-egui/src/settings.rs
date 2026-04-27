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
    #[serde(default)]
    pub network_probe_url: Option<String>,
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub api_key: Option<String>,
}

impl GuiSettings {
    pub fn config_path() -> PathBuf {
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

    #[allow(dead_code)]
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
            language: Some("zh".into()),
            api_key: None,
        }
    }
}

#[allow(dead_code)]
pub fn scan_local_models() -> Vec<(String, String)> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    fn add_ggufs_from_dir(
        dir: &PathBuf,
        results: &mut Vec<(String, String)>,
        seen: &mut HashSet<String>,
    ) {
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

    if let Some(home) = dirs::home_dir() {
        let models_dir = home.join("models");
        if models_dir.is_dir() {
            add_ggufs_from_dir(&models_dir, &mut results, &mut seen);
        }
    }

    results.sort_by(|a, b| a.1.cmp(&b.1));
    results
}

#[allow(dead_code)]
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
        ("local".into(), "Local (GGUF)".into(), local_model_names),
    ]
}
