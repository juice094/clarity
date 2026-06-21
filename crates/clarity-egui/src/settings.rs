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

/// A user-defined web bookmark shown in the left sidebar web section.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct WebLink {
    pub name: String,
    pub url: String,
}

/// A user-defined work template that launches a new session with a pre-filled prompt.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct WorkTemplate {
    pub name: String,
    pub prompt: String,
}

/// Authentication mode for a user-configured OpenClaw Gateway connection.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpenClawAuthMode {
    /// Plain token auth. Suitable for local or permissive Gateways.
    #[default]
    TokenOnly,
    /// Remote admin/device token plus Ed25519 device attestation.
    TokenWithDevice,
    /// Device token returned by the Gateway after pairing.
    DevicePaired,
}

/// Which JSON-RPC send method an OpenClaw connection should use.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpenClawSendMethod {
    /// `sessions.send` with `key` — typical remote OpenClaw path.
    #[default]
    SessionsSend,
    /// `chat.send` with `sessionKey` — KimiClaw-local/ACP-style path.
    ChatSend,
}

/// A user-configured OpenClaw Gateway connection.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct OpenClawConnection {
    /// Display name shown in the bot switcher.
    pub name: String,
    /// WebSocket URL of the Gateway, e.g. `ws://host:18789`.
    pub gateway_url: String,
    /// Admin or device token used for authentication.
    pub token: String,
    /// How the token is combined with the local device identity.
    #[serde(default)]
    pub auth_mode: OpenClawAuthMode,
    /// Whether this connection should be discovered and offered in the UI.
    #[serde(default = "default_openclaw_enabled")]
    pub enabled: bool,
    /// Optional device-specific token for `DevicePaired` mode.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_token: Option<String>,
    /// Optional session key override for this remote OpenClaw connection.
    ///
    /// When set, Claw sessions bound to this connection use this key instead of
    /// the default `agent:main:<role>` key, allowing egui to join an existing
    /// remote main session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_key: Option<String>,
    /// Which JSON-RPC send method to use for chat messages.
    ///
    /// Most remote OpenClaw Gateways expect `sessions.send`; some
    /// KimiClaw/ACP-style Gateways expect `chat.send`.
    #[serde(default)]
    pub send_method: OpenClawSendMethod,
}

fn default_openclaw_enabled() -> bool {
    true
}

impl Default for OpenClawConnection {
    fn default() -> Self {
        Self {
            name: "Remote OpenClaw".into(),
            gateway_url: String::new(),
            token: String::new(),
            auth_mode: OpenClawAuthMode::default(),
            enabled: true,
            device_token: None,
            session_key: None,
            send_method: OpenClawSendMethod::default(),
        }
    }
}

/// Holds gui settings state.
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
    pub sidebar_width: Option<f32>,
    #[serde(default)]
    pub web_tabs: Vec<WebTab>,
    /// S8 P3B.1: persisted active persona id (e.g. "kin", "analyst", "programmer").
    /// Maps to `clarity_core::endpoint::EndpointDescriptor.id`.
    #[serde(default)]
    pub active_persona_id: Option<String>,
    /// S6 Phase C: whether the right rail drawer is visible.
    #[serde(default)]
    pub right_rail_visible: bool,
    /// S6 Phase C: last active right rail drawer context.
    #[serde(default)]
    pub right_rail_context: clarity_core::ui::RightRailContext,
    /// S6 Phase C: display order of stacked right rail cards.
    #[serde(default)]
    pub right_rail_card_order: Vec<clarity_core::ui::RightRailCard>,
    /// S6 Phase C: display order of plugin toolbar items.
    #[serde(default)]
    pub plugin_order: Vec<String>,
    /// S6 Phase C3: show the layout debug overlay (green/blue/red/yellow diagnostic rects).
    #[serde(default)]
    pub debug_layout_overlay: bool,
    /// S6 navigation tree: custom work templates.
    #[serde(default)]
    pub work_templates: Vec<WorkTemplate>,
    /// S6 navigation tree: web bookmarks.
    #[serde(default)]
    pub web_links: Vec<WebLink>,
    /// S6 navigation tree: set to true after default templates are seeded on first launch.
    #[serde(default)]
    pub work_templates_initialized: bool,
    /// User-configured OpenClaw Gateway connections.
    #[serde(default)]
    pub openclaw_connections: Vec<OpenClawConnection>,
    #[serde(skip)]
    pub profiles: HashMap<String, AgentProfile>,
}

impl GuiSettings {
    /// Returns the path to the settings file.
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

    /// Returns the path to the profiles file.
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

    /// Loads persisted state from disk.
    pub fn load() -> Self {
        let path = Self::config_path();
        let mut settings: Self = if let Ok(content) = std::fs::read_to_string(&path) {
            // ponytail: migrate legacy split web-link lists into a single list.
            // We parse as Value first so unknown fields survive and old keys can
            // be merged before `GuiSettings` deserialization rejects them.
            match serde_json::from_str::<serde_json::Value>(&content) {
                Ok(mut value) => {
                    migrate_legacy_web_links(&mut value);
                    match serde_json::from_value(value) {
                        Ok(settings) => settings,
                        Err(e) => {
                            tracing::warn!(
                                "Failed to deserialize migrated settings at {}: {}. Falling back to defaults.",
                                path.display(),
                                e
                            );
                            Self::default_with_env()
                        }
                    }
                }
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

    /// Returns defaults merged with environment variables.
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
            sidebar_width: None,
            web_tabs: Vec::new(),
            active_persona_id: None,
            right_rail_visible: false,
            right_rail_context: clarity_core::ui::RightRailContext::Session,
            right_rail_card_order: vec![
                clarity_core::ui::RightRailCard::Progress,
                clarity_core::ui::RightRailCard::Context,
            ],
            plugin_order: vec![
                "doc".to_string(),
                "web".to_string(),
                "sheet".to_string(),
                "ppt".to_string(),
            ],
            debug_layout_overlay: false,
            work_templates: Vec::new(),
            web_links: Vec::new(),
            work_templates_initialized: false,
            openclaw_connections: Vec::new(),
            profiles: HashMap::new(),
        }
    }
}

// Model enumeration moved to clarity_core::view_models::settings.

/// Merge legacy `web_links_chat` and `web_links_work` arrays into the unified
/// `web_links` field, deduplicating by URL. This preserves existing user
/// bookmarks when upgrading from versions that stored two separate lists.
fn migrate_legacy_web_links(value: &mut serde_json::Value) {
    let Some(obj) = value.as_object_mut() else {
        return;
    };

    let has_new = obj
        .get("web_links")
        .and_then(|v| v.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);
    if has_new {
        return;
    }

    let mut seen = std::collections::HashSet::new();
    let mut merged = Vec::new();

    for key in ["web_links_chat", "web_links_work"] {
        if let Some(arr) = obj.get(key).and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(url) = item.get("url").and_then(|v| v.as_str()) {
                    if seen.insert(url.to_string()) {
                        merged.push(item.clone());
                    }
                }
            }
        }
    }

    if !merged.is_empty() {
        obj.insert("web_links".to_string(), serde_json::Value::Array(merged));
    }
}

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
        let models = clarity_llm::get_available_models();
        assert!(!models.is_empty());
        let keys: Vec<String> = models.iter().map(|(k, _, _)| k.clone()).collect();
        assert!(keys.contains(&"openai".to_string()));
        assert!(keys.contains(&"local".to_string()));
    }

    #[test]
    fn test_get_available_models_local_label() {
        let models = clarity_llm::get_available_models();
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

    #[allow(unsafe_code)]
    fn set_env(key: &str, value: &str) {
        // SAFETY: test-only helper; env vars are manipulated in a single-threaded test context.
        unsafe { std::env::set_var(key, value) };
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
        set_env("CLARITY_TEST_KEY_12345", "secret-from-env");
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

[profiles.local]
model = "local-qwen"
provider = "local"
approval_mode = "yolo"
"#;
        let file: ProfilesFile = toml::from_str(toml).expect("parse profiles.toml");
        assert_eq!(file.profiles.len(), 2);
        assert!(file.profiles.contains_key("default"));
        assert!(file.profiles.contains_key("local"));
        let local = file.profiles.get("local").unwrap();
        assert_eq!(local.provider, "local");
        assert_eq!(local.model, "local-qwen");
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

    // ============================================================================
    // OpenClaw connection configuration tests
    // ============================================================================

    #[test]
    fn test_openclaw_connection_default() {
        let conn = OpenClawConnection::default();
        assert!(conn.enabled);
        assert_eq!(conn.auth_mode, OpenClawAuthMode::TokenOnly);
        assert!(conn.device_token.is_none());
    }

    #[test]
    fn test_openclaw_connection_serde_roundtrip() {
        let conn = OpenClawConnection {
            name: "Remote Lab".into(),
            gateway_url: "ws://openclaw.example.com:18789".into(),
            token: "${env:OPENCLAW_REMOTE_TOKEN}".into(),
            auth_mode: OpenClawAuthMode::TokenWithDevice,
            enabled: true,
            device_token: Some("device-token".into()),
            session_key: Some("remote-main-session".into()),
            send_method: OpenClawSendMethod::ChatSend,
        };
        let json = serde_json::to_string(&conn).expect("serialize");
        let deserialized: OpenClawConnection = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deserialized.name, conn.name);
        assert_eq!(deserialized.gateway_url, conn.gateway_url);
        assert_eq!(deserialized.token, conn.token);
        assert_eq!(deserialized.auth_mode, conn.auth_mode);
        assert_eq!(deserialized.enabled, conn.enabled);
        assert_eq!(deserialized.device_token, conn.device_token);
        assert_eq!(deserialized.session_key, conn.session_key);
        assert_eq!(deserialized.send_method, conn.send_method);
    }

    #[test]
    fn test_gui_settings_openclaw_connections_default_empty() {
        let settings = GuiSettings::default_with_env();
        assert!(settings.openclaw_connections.is_empty());
    }

    #[test]
    fn test_migrate_legacy_web_links_merges_and_dedupes() {
        let mut value = serde_json::json!({
            "model": "gpt-4o",
            "web_links_chat": [
                {"name": "Chat A", "url": "https://a.example"},
                {"name": "Shared", "url": "https://shared.example"}
            ],
            "web_links_work": [
                {"name": "Work B", "url": "https://b.example"},
                {"name": "Shared", "url": "https://shared.example"}
            ]
        });
        migrate_legacy_web_links(&mut value);
        let merged = value["web_links"].as_array().unwrap();
        assert_eq!(merged.len(), 3);
        let urls: Vec<String> = merged
            .iter()
            .map(|v| v["url"].as_str().unwrap().to_string())
            .collect();
        assert!(urls.contains(&"https://a.example".to_string()));
        assert!(urls.contains(&"https://b.example".to_string()));
        assert!(urls.contains(&"https://shared.example".to_string()));
        // Duplicate shared URL must appear only once.
        assert_eq!(
            urls.iter()
                .filter(|u| *u == "https://shared.example")
                .count(),
            1
        );
    }

    #[test]
    fn test_migrate_legacy_web_links_preserves_existing_web_links() {
        let mut value = serde_json::json!({
            "web_links": [{"name": "Existing", "url": "https://existing.example"}],
            "web_links_chat": [{"name": "Legacy", "url": "https://legacy.example"}]
        });
        migrate_legacy_web_links(&mut value);
        let merged = value["web_links"].as_array().unwrap();
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0]["url"], "https://existing.example");
    }

    #[test]
    fn test_migrate_legacy_web_links_no_old_keys_is_noop() {
        let mut value = serde_json::json!({"model": "gpt-4o"});
        migrate_legacy_web_links(&mut value);
        assert!(value.get("web_links").is_none());
    }
}
