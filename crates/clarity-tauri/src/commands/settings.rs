use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct GuiSettings {
    pub model: String,
    pub provider: String,
    pub approval_mode: String,
    pub theme: String,
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
        }
    }
}

#[tauri::command]
pub fn get_settings() -> GuiSettings {
    GuiSettings::load()
}

#[tauri::command]
pub fn save_settings(settings: GuiSettings) -> Result<(), String> {
    settings.save()
}

#[tauri::command]
pub fn get_available_models() -> Vec<(String, String, Vec<String>)> {
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
    ]
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
