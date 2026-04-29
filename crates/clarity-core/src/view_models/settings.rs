use clarity_wire::{ButtonStyle, TextRole, UserAction, ViewCommand};
use std::collections::HashSet;
use std::path::PathBuf;

/// Snapshot of settings state that can be persisted or applied to an Agent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsSnapshot {
    pub provider: String,
    pub model: String,
    pub approval_mode: String,
    pub api_key: Option<String>,
    pub local_model_path: Option<String>,
    pub theme: String,
}

/// Protocol-driven ViewModel for the settings panel.
///
/// Holds form state, generates `ViewCommand` trees, and routes `UserAction` events.
/// Can broadcast its current command tree to a `clarity_wire::Wire` for remote frontends.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsViewModel {
    provider: String,
    model: String,
    approval_mode: String,
    api_key: Option<String>,
    local_model_path: Option<String>,
    theme: String,
    dirty: bool,
}

impl Default for SettingsViewModel {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            model: "gpt-4o".into(),
            approval_mode: "interactive".into(),
            api_key: None,
            local_model_path: None,
            theme: "dark".into(),
            dirty: false,
        }
    }
}

impl SettingsViewModel {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_snapshot(snapshot: &SettingsSnapshot) -> Self {
        Self {
            provider: snapshot.provider.clone(),
            model: snapshot.model.clone(),
            approval_mode: snapshot.approval_mode.clone(),
            api_key: snapshot.api_key.clone(),
            local_model_path: snapshot.local_model_path.clone(),
            theme: snapshot.theme.clone(),
            dirty: false,
        }
    }

    pub fn snapshot(&self) -> SettingsSnapshot {
        SettingsSnapshot {
            provider: self.provider.clone(),
            model: self.model.clone(),
            approval_mode: self.approval_mode.clone(),
            api_key: self.api_key.clone(),
            local_model_path: self.local_model_path.clone(),
            theme: self.theme.clone(),
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Generate the declarative command tree for the current state.
    pub fn commands(&self) -> Vec<ViewCommand> {
        let providers = get_available_models();

        let provider_options: Vec<(String, String)> = providers
            .iter()
            .map(|(k, l, _)| (k.clone(), l.clone()))
            .collect();

        let current_models = providers
            .iter()
            .find(|(k, _, _)| k == &self.provider)
            .map(|(_, _, m)| m.clone())
            .unwrap_or_default();

        let model_options: Vec<(String, String)> = current_models
            .into_iter()
            .map(|m| (m.clone(), m))
            .collect();

        let approval_options = vec![
            ("interactive".into(), "Interactive — Approve each tool call".into()),
            ("yolo".into(), "Yolo — Auto-approve all".into()),
            ("plan".into(), "Plan — Review plan before execution".into()),
        ];

        vec![
            ViewCommand::HStack {
                children: vec![
                    ViewCommand::Text { content: "Provider".into(), role: TextRole::Label, size: 13.0 },
                    ViewCommand::ComboBox {
                        id: "provider".into(),
                        selected_value: self.provider.clone(),
                        options: provider_options,
                        width: 200.0,
                    },
                ],
            },
            ViewCommand::Space { height: 8.0 },
            ViewCommand::HStack {
                children: vec![
                    ViewCommand::Text { content: "Model".into(), role: TextRole::Label, size: 13.0 },
                    ViewCommand::ComboBox {
                        id: "model".into(),
                        selected_value: self.model.clone(),
                        options: model_options,
                        width: 200.0,
                    },
                ],
            },
            ViewCommand::Space { height: 8.0 },
            ViewCommand::HStack {
                children: vec![
                    ViewCommand::Text { content: "API Key".into(), role: TextRole::Label, size: 13.0 },
                    ViewCommand::TextInput {
                        id: "api_key".into(),
                        value: self.api_key.clone().unwrap_or_default(),
                        placeholder: String::new(),
                        password: true,
                        width: 200.0,
                    },
                ],
            },
            ViewCommand::Space { height: 8.0 },
            ViewCommand::HStack {
                children: vec![
                    ViewCommand::Text { content: "Local Model Path".into(), role: TextRole::Label, size: 13.0 },
                    ViewCommand::TextInput {
                        id: "local_model_path".into(),
                        value: self.local_model_path.clone().unwrap_or_default(),
                        placeholder: String::new(),
                        password: false,
                        width: 200.0,
                    },
                ],
            },
            ViewCommand::Space { height: 8.0 },
            ViewCommand::HStack {
                children: vec![
                    ViewCommand::Text { content: "Approval Mode".into(), role: TextRole::Label, size: 13.0 },
                    ViewCommand::ComboBox {
                        id: "approval_mode".into(),
                        selected_value: self.approval_mode.clone(),
                        options: approval_options,
                        width: 200.0,
                    },
                ],
            },
            ViewCommand::Space { height: 16.0 },
            ViewCommand::HStack {
                children: vec![
                    ViewCommand::Button {
                        id: "cancel".into(),
                        label: "Cancel".into(),
                        style: ButtonStyle::Secondary,
                        min_width: 80.0,
                        min_height: 32.0,
                    },
                    ViewCommand::Button {
                        id: "save".into(),
                        label: "Save".into(),
                        style: ButtonStyle::Primary,
                        min_width: 80.0,
                        min_height: 32.0,
                    },
                ],
            },
        ]
    }

    /// Route a user action back into the ViewModel state.
    pub fn handle_action(&mut self, action: UserAction) {
        match action {
            UserAction::ComboChange { id, selected } if id == "provider" => {
                self.provider = selected.clone();
                let providers = get_available_models();
                if let Some((_, _, models)) = providers.iter().find(|(k, _, _)| k == &selected) {
                    if let Some(first) = models.first() {
                        self.model = first.clone();
                    }
                }
                self.dirty = true;
            }
            UserAction::ComboChange { id, selected } if id == "model" => {
                self.model = selected;
                self.dirty = true;
            }
            UserAction::ComboChange { id, selected } if id == "approval_mode" => {
                self.approval_mode = selected;
                self.dirty = true;
            }
            UserAction::TextInputChange { id, value } if id == "api_key" => {
                self.api_key = if value.is_empty() { None } else { Some(value) };
                self.dirty = true;
            }
            UserAction::TextInputChange { id, value } if id == "local_model_path" => {
                self.local_model_path = if value.is_empty() { None } else { Some(value) };
                self.dirty = true;
            }
            _ => {}
        }
    }

    /// Broadcast the current command tree to all wire consumers.
    pub fn sync_to_wire(&self, wire: &clarity_wire::Wire) {
        let commands = self.commands();
        wire.soul_side().send_view(commands);
    }
}

// ============================================================================
// Model enumeration (moved from clarity-egui)
// ============================================================================

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

// ============================================================================
// Unit tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_view_model_default() {
        let vm = SettingsViewModel::new();
        assert_eq!(vm.provider, "openai");
        assert_eq!(vm.model, "gpt-4o");
        assert_eq!(vm.approval_mode, "interactive");
        assert!(!vm.is_dirty());
    }

    #[test]
    fn test_view_model_from_snapshot_roundtrip() {
        let vm = SettingsViewModel::new();
        let snapshot = vm.snapshot();
        let vm2 = SettingsViewModel::from_snapshot(&snapshot);
        assert_eq!(vm, vm2);
    }

    #[test]
    fn test_view_model_provider_change_cascades_model() {
        let mut vm = SettingsViewModel::new();
        vm.handle_action(UserAction::ComboChange {
            id: "provider".into(),
            selected: "kimi".into(),
        });
        assert_eq!(vm.provider, "kimi");
        assert_eq!(vm.model, "kimi-k2-07132k"); // first model for kimi
        assert!(vm.is_dirty());
    }

    #[test]
    fn test_view_model_api_key_update() {
        let mut vm = SettingsViewModel::new();
        vm.handle_action(UserAction::TextInputChange {
            id: "api_key".into(),
            value: "sk-secret".into(),
        });
        assert_eq!(vm.api_key, Some("sk-secret".into()));
        assert!(vm.is_dirty());
    }

    #[test]
    fn test_view_model_api_key_empty_becomes_none() {
        let mut vm = SettingsViewModel::new();
        vm.api_key = Some("old".into());
        vm.handle_action(UserAction::TextInputChange {
            id: "api_key".into(),
            value: "".into(),
        });
        assert_eq!(vm.api_key, None);
    }

    #[test]
    fn test_view_model_commands_not_empty() {
        let vm = SettingsViewModel::new();
        let cmds = vm.commands();
        assert!(!cmds.is_empty());
        // Should contain Provider, Model, API Key, Local Path, Approval Mode, Buttons
        assert!(cmds.iter().any(|c| matches!(c, ViewCommand::HStack { .. })));
    }

    #[test]
    fn test_get_available_models_has_providers() {
        let models = get_available_models();
        assert!(!models.is_empty());
        let keys: Vec<String> = models.iter().map(|(k, _, _)| k.clone()).collect();
        assert!(keys.contains(&"openai".to_string()));
        assert!(keys.contains(&"local".to_string()));
    }

    #[test]
    fn test_get_available_models_local_label() {
        let models = get_available_models();
        let local = models.iter().find(|(k, _, _)| k == "local");
        assert!(local.is_some());
        let (_, label, _) = local.unwrap();
        assert_eq!(label, "Local (GGUF)");
    }
}
