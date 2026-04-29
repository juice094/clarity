use serde::{Deserialize, Serialize};
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
            match serde_json::from_str(&content) {
                Ok(settings) => return settings,
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse settings at {}: {}. Falling back to defaults.",
                        path.display(),
                        e
                    );
                    // Backup corrupted file so user can manually recover
                    let bak = path.with_extension("json.bak");
                    if let Err(e) = std::fs::rename(&path, &bak) {
                        tracing::warn!("Failed to backup corrupted settings to {}: {}", bak.display(), e);
                    }
                }
            }
        }
        Self::default_with_env()
    }

    pub fn default_with_env() -> Self {
        let mut s = Self::default();
        if let Ok(key) = std::env::var("KIMI_API_KEY") {
            s.provider = "kimi".to_string();
            s.api_key = Some(key);
        } else if let Ok(key) = std::env::var("OPENAI_API_KEY") {
            s.provider = "openai".to_string();
            s.api_key = Some(key);
        }
        // Match model env var to the selected provider to avoid mismatches
        match s.provider.as_str() {
            "kimi" => {
                if let Ok(model) = std::env::var("KIMI_MODEL") {
                    s.model = model;
                }
            }
            "openai" => {
                if let Ok(model) = std::env::var("OPENAI_MODEL") {
                    s.model = model;
                }
            }
            _ => {}
        }
        s
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

// Model enumeration moved to clarity_core::view_models::settings.

// ============================================================================
// Unit tests for settings persistence and model enumeration
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings_structure() {
        let settings = GuiSettings::default_with_env();
        // All string fields must be non-empty or have sensible defaults.
        assert!(!settings.model.is_empty());
        assert!(!settings.provider.is_empty());
        assert!(!settings.approval_mode.is_empty());
        assert!(!settings.theme.is_empty());
    }

    #[test]
    fn test_get_available_models_has_providers() {
        let models = clarity_core::view_models::settings::get_available_models();
        assert!(!models.is_empty());
        let keys: Vec<String> = models.iter().map(|(k, _, _)| k.clone()).collect();
        assert!(keys.contains(&"openai".to_string()));
        assert!(keys.contains(&"local".to_string()));
    }

    #[test]
    fn test_get_available_models_local_label() {
        let models = clarity_core::view_models::settings::get_available_models();
        let local = models.iter().find(|(k, _, _)| k == "local");
        assert!(local.is_some());
        let (_, label, _) = local.unwrap();
        assert_eq!(label, "Local (GGUF)");
    }

    #[test]
    fn test_settings_clone_roundtrip() {
        let original = GuiSettings::default_with_env();
        let cloned = original.clone();
        assert_eq!(original.model, cloned.model);
        assert_eq!(original.provider, cloned.provider);
        assert_eq!(original.approval_mode, cloned.approval_mode);
    }

    #[test]
    fn test_settings_serde_roundtrip() {
        let original = GuiSettings::default_with_env();
        let json = serde_json::to_string(&original).expect("serialize");
        let deserialized: GuiSettings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(original.model, deserialized.model);
        assert_eq!(original.provider, deserialized.provider);
        assert_eq!(original.approval_mode, deserialized.approval_mode);
    }
}
