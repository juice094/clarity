use crate::ui::types::WebTab;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A named profile that overrides provider/model/approval_mode for a specific use-case.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AgentProfile {
    pub model: String,
    pub provider: String,
    pub approval_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_model_path: Option<String>,
}

/// Top-level structure of `profiles.toml`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct ProfilesFile {
    #[serde(default)]
    profiles: HashMap<String, AgentProfile>,
}

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
    #[serde(default)]
    pub active_profile: Option<String>,
    #[serde(default)]
    pub font_scale: Option<f32>,
    #[serde(default)]
    pub content_width: Option<f32>,
    #[serde(default)]
    pub input_style: Option<String>,
    #[serde(default)]
    pub web_tabs: Vec<WebTab>,
    #[serde(skip)]
    pub profiles: HashMap<String, AgentProfile>,
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

    pub fn profiles_path() -> PathBuf {
        if let Ok(appdata) = std::env::var("APPDATA") {
            let mut path = PathBuf::from(appdata);
            path.push("clarity");
            path.push("profiles.toml");
            return path;
        }
        if let Ok(home) = std::env::var("HOME") {
            let mut path = PathBuf::from(home);
            path.push(".config");
            path.push("clarity");
            path.push("profiles.toml");
            return path;
        }
        PathBuf::from("profiles.toml")
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        let mut settings: Self = if let Ok(content) = std::fs::read_to_string(&path) {
            match serde_json::from_str(&content) {
                Ok(settings) => settings,
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse settings at {}: {}. Falling back to defaults.",
                        path.display(),
                        e
                    );
                    // Backup corrupted file so user can manually recover
                    let bak = path.with_extension("json.bak");
                    if let Err(e) = std::fs::rename(&path, &bak) {
                        tracing::warn!(
                            "Failed to backup corrupted settings to {}: {}",
                            bak.display(),
                            e
                        );
                    }
                    Self::default_with_env()
                }
            }
        } else {
            Self::default_with_env()
        };

        // Load profiles from separate TOML file (single source of truth)
        let profiles_path = Self::profiles_path();
        if let Ok(content) = std::fs::read_to_string(&profiles_path) {
            match toml::from_str::<ProfilesFile>(&content) {
                Ok(file) => {
                    settings.profiles = file.profiles;
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse profiles at {}: {}. Using empty profiles.",
                        profiles_path.display(),
                        e
                    );
                }
            }
        }

        settings
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

    /// Resolve an API key value, expanding `${env:VAR_NAME}` syntax.
    ///
    /// If the stored value matches `${env:VAR_NAME}`, reads the named environment variable.
    /// Otherwise returns the value as-is (backward compatible with plain keys).
    #[allow(dead_code)]
    pub fn resolve_api_key(value: &Option<String>) -> Option<String> {
        let raw = value.as_ref()?;
        // Parse `${env:VAR_NAME}` without pulling in regex crate
        let inner = match raw.strip_prefix("${env:").and_then(|s| s.strip_suffix("}")) {
            Some(inner) => inner,
            None => return Some(raw.clone()),
        };
        if inner.is_empty() || inner.contains(['{', '}']) {
            return Some(raw.clone());
        }
        std::env::var(inner).ok().or_else(|| Some(raw.clone()))
    }

    /// Save settings incrementally — only changed fields overwrite the disk file.
    /// Unchanged fields and unknown fields (e.g. from newer versions) are preserved.
    #[allow(dead_code)]
    pub fn save(&self) -> Result<(), String> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }

        // Read existing config as base so we don't lose unknown fields
        let existing: serde_json::Value = std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}));

        let new = serde_json::to_value(self).map_err(|e| e.to_string())?;
        let merged = merge_json(existing, new);

        let content = serde_json::to_string_pretty(&merged).map_err(|e| e.to_string())?;
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
            active_profile: None,
            font_scale: None,
            content_width: None,
            input_style: Some("gui".into()),
            web_tabs: Vec::new(),
            profiles: HashMap::new(),
        }
    }
}

// Model enumeration moved to clarity_core::view_models::settings.

/// Deep-merge two JSON values: `new` overwrites `base` recursively.
/// Arrays are replaced (not merged). Null values in `new` delete keys in `base`.
fn merge_json(base: serde_json::Value, new: serde_json::Value) -> serde_json::Value {
    use serde_json::Value;
    match (base, new) {
        (Value::Object(mut base_map), Value::Object(new_map)) => {
            for (key, new_val) in new_map {
                if new_val.is_null() {
                    base_map.remove(&key);
                } else {
                    let merged = match base_map.remove(&key) {
                        Some(base_val) => merge_json(base_val, new_val),
                        None => new_val,
                    };
                    base_map.insert(key, merged);
                }
            }
            Value::Object(base_map)
        }
        (_, new) => new,
    }
}

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

    #[test]
    fn test_resolve_api_key_plain() {
        assert_eq!(
            GuiSettings::resolve_api_key(&Some("sk-plain-key".into())),
            Some("sk-plain-key".into())
        );
    }

    #[test]
    fn test_resolve_api_key_env_syntax() {
        std::env::set_var("CLARITY_TEST_KEY_12345", "secret-from-env");
        assert_eq!(
            GuiSettings::resolve_api_key(&Some("${env:CLARITY_TEST_KEY_12345}".into())),
            Some("secret-from-env".into())
        );
    }

    #[test]
    fn test_resolve_api_key_env_missing_fallback() {
        assert_eq!(
            GuiSettings::resolve_api_key(&Some("${env:CLARITY_MISSING_VAR_XYZ}".into())),
            Some("${env:CLARITY_MISSING_VAR_XYZ}".into())
        );
    }

    #[test]
    fn test_resolve_api_key_none() {
        assert_eq!(GuiSettings::resolve_api_key(&None), None);
    }

    #[test]
    fn test_merge_json_basic() {
        let base = serde_json::json!({"provider": "openai", "model": "gpt-4o"});
        let new = serde_json::json!({"provider": "kimi"});
        let merged = merge_json(base, new);
        assert_eq!(merged["provider"], "kimi");
        assert_eq!(merged["model"], "gpt-4o"); // preserved
    }

    #[test]
    fn test_merge_json_delete_with_null() {
        let base = serde_json::json!({"provider": "openai", "extra": "keep"});
        let new = serde_json::json!({"provider": null});
        let merged = merge_json(base, new);
        assert!(!merged.as_object().unwrap().contains_key("provider"));
        assert_eq!(merged["extra"], "keep");
    }

    #[test]
    fn test_merge_json_nested() {
        let base = serde_json::json!({"a": {"x": 1, "y": 2}});
        let new = serde_json::json!({"a": {"x": 10}});
        let merged = merge_json(base, new);
        assert_eq!(merged["a"]["x"], 10);
        assert_eq!(merged["a"]["y"], 2);
    }

    #[test]
    fn test_merge_json_preserves_unknown_fields() {
        let base = serde_json::json!({"provider": "openai", "future_field": true});
        let new = serde_json::json!({"provider": "kimi"});
        let merged = merge_json(base, new);
        assert_eq!(merged["provider"], "kimi");
        assert_eq!(merged["future_field"], true);
    }

    // ============================================================================
    // Sprint 10 D1: AgentProfile tests
    // ============================================================================

    #[test]
    fn test_profiles_file_parsing() {
        let toml = r#"
[profiles.default]
model = "gpt-4o"
provider = "openai"
approval_mode = "interactive"

[profiles.greylocal]
model = "local-qwen"
provider = "local"
approval_mode = "yolo"
"#;
        let file: ProfilesFile = toml::from_str(toml).expect("parse profiles.toml");
        assert_eq!(file.profiles.len(), 2);
        assert!(file.profiles.contains_key("default"));
        assert!(file.profiles.contains_key("greylocal"));
        let greylocal = file.profiles.get("greylocal").unwrap();
        assert_eq!(greylocal.provider, "local");
        assert_eq!(greylocal.model, "local-qwen");
    }

    #[test]
    fn test_gui_settings_skips_profiles_in_json() {
        let mut settings = GuiSettings {
            active_profile: Some("research".into()),
            ..Default::default()
        };
        settings.profiles.insert(
            "research".into(),
            AgentProfile {
                model: "kimi-k2".into(),
                provider: "kimi".into(),
                approval_mode: "plan".into(),
                api_key: Some("sk-test".into()),
                local_model_path: None,
            },
        );
        let json = serde_json::to_string(&settings).expect("serialize");
        // profiles field is #[serde(skip)], so it must not appear in JSON
        assert!(
            !json.contains("profiles"),
            "profiles should not be serialized to gui-settings.json"
        );
        assert!(
            json.contains("active_profile"),
            "active_profile should be serialized"
        );
    }

    #[test]
    fn test_active_profile_roundtrip() {
        let settings = GuiSettings {
            active_profile: Some("research".into()),
            ..Default::default()
        };
        let json = serde_json::to_string(&settings).expect("serialize");
        let deserialized: GuiSettings = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.active_profile, Some("research".into()));
    }
}
