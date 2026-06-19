//! Settings ViewModel.
//!
//! **ADR-006 status (2026-05-11)**: `sync_to_wire()` is deprecated and scheduled
//! for removal in 0.4.0. `commands() / apply_user_action()` will be migrated
//! to a new `clarity-frontend-ir` crate during Phase D — see
//! `docs/adr/ADR-006-protocol-layer-convergence.md`.
//!
//! File-level `#![allow(deprecated)]` is applied so the ViewModel keeps
//! compiling cleanly while the migration is staged. External callers
//! (clarity-egui, clarity-tui) see deprecation notices via per-item
//! `#[deprecated]` attributes.

#![allow(deprecated)]

use clarity_wire::{ButtonStyle, TextRole, UserAction, ViewCommand};

/// Snapshot of settings state that can be persisted or applied to an Agent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SettingsSnapshot {
    /// Provider.
    pub provider: String,
    /// Model.
    pub model: String,
    /// Current approval mode.
    pub approval_mode: String,
    /// Api key.
    pub api_key: Option<String>,
    /// Local model path.
    pub local_model_path: Option<String>,
    /// Theme.
    pub theme: String,
    /// Active profile.
    pub active_profile: Option<String>,
}

/// Entry describing one available model provider and its model IDs.
///
/// The tuple layout is `(provider_id, display_label, model_ids)`.
pub type ProviderModelEntry = (String, String, Vec<String>);

/// Protocol-driven ViewModel for the settings panel.
///
/// Holds form state, generates `ViewCommand` trees, and routes `UserAction` events.
/// Can broadcast its current command tree to a `clarity_wire::Wire` for remote frontends.
///
/// The model/provider catalog is **injected** via [`Self::set_available_models`] rather than
/// queried from `clarity_llm::ModelRegistry` directly, keeping this crate decoupled from
/// concrete LLM provider construction.
#[derive(Clone, Debug, PartialEq)]
pub struct SettingsViewModel {
    provider: String,
    model: String,
    approval_mode: String,
    api_key: Option<String>,
    local_model_path: Option<String>,
    theme: String,
    active_profile: Option<String>,
    profiles: Vec<(String, String)>, // (id, display_label)
    dirty: bool,
    available_models: Vec<ProviderModelEntry>,
}

impl Default for SettingsViewModel {
    fn default() -> Self {
        Self {
            provider: "openai".into(),
            model: "gpt-4o".into(),
            approval_mode: {
                let modes = crate::capability::CapabilityRegistry::supported_approval_modes("egui");
                if modes.contains(&"interactive") {
                    "interactive".into()
                } else {
                    modes.first().unwrap_or(&"yolo").to_string()
                }
            },
            api_key: None,
            local_model_path: None,
            theme: "dark".into(),
            active_profile: None,
            profiles: Vec::new(),
            dirty: false,
            available_models: Vec::new(),
        }
    }
}

impl SettingsViewModel {
    /// Create a new `SettingsViewModel`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create from snapshot.
    pub fn from_snapshot(snapshot: &SettingsSnapshot, profiles: Vec<(String, String)>) -> Self {
        Self {
            provider: snapshot.provider.clone(),
            model: snapshot.model.clone(),
            approval_mode: snapshot.approval_mode.clone(),
            api_key: snapshot.api_key.clone(),
            local_model_path: snapshot.local_model_path.clone(),
            theme: snapshot.theme.clone(),
            active_profile: snapshot.active_profile.clone(),
            profiles,
            dirty: false,
            available_models: Vec::new(),
        }
    }

    /// `snapshot`.
    pub fn snapshot(&self) -> SettingsSnapshot {
        SettingsSnapshot {
            provider: self.provider.clone(),
            model: self.model.clone(),
            approval_mode: self.approval_mode.clone(),
            api_key: self.api_key.clone(),
            local_model_path: self.local_model_path.clone(),
            theme: self.theme.clone(),
            active_profile: self.active_profile.clone(),
        }
    }

    /// `is_dirty`.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    /// `clear_dirty`.
    pub fn clear_dirty(&mut self) {
        self.dirty = false;
    }

    /// Inject the list of available providers and their models.
    ///
    /// Consumers should obtain this list from `clarity_llm::get_available_models()`
    /// (or an equivalent frontend-specific source) and pass it before calling
    /// [`Self::commands`] or actions that may change the provider.
    pub fn set_available_models(&mut self, models: Vec<ProviderModelEntry>) {
        self.available_models = models;
    }

    /// Read back the currently injected available models.
    pub fn available_models(&self) -> &[ProviderModelEntry] {
        &self.available_models
    }

    /// Generate the declarative command tree for the current state.
    pub fn commands(&self) -> Vec<ViewCommand> {
        let provider_options: Vec<(String, String)> = self
            .available_models
            .iter()
            .map(|(k, l, _)| (k.clone(), l.clone()))
            .collect();

        let current_models = self
            .available_models
            .iter()
            .find(|(k, _, _)| k == &self.provider)
            .map(|(_, _, m)| m.clone())
            .unwrap_or_default();

        let model_options: Vec<(String, String)> =
            current_models.into_iter().map(|m| (m.clone(), m)).collect();

        let approval_modes =
            crate::capability::CapabilityRegistry::supported_approval_modes("egui");
        let approval_options: Vec<(String, String)> = approval_modes
            .into_iter()
            .map(|mode| {
                let label = match mode {
                    "interactive" => "Interactive — Approve each tool call",
                    "yolo" => "Yolo — Auto-approve all",
                    "plan" => "Plan — Review plan before execution",
                    "smart" => "Smart — Auto-approve low risk & remembered tools",
                    other => other,
                };
                (mode.into(), label.into())
            })
            .collect();

        let mut cmds: Vec<ViewCommand> = Vec::new();

        // Profile selector (only shown when profiles are configured)
        if !self.profiles.is_empty() {
            let profile_options: Vec<(String, String)> = self.profiles.clone();
            cmds.push(ViewCommand::HStack {
                children: vec![
                    ViewCommand::Text {
                        content: "Profile".into(),
                        role: TextRole::Label,
                        size: 13.0,
                    },
                    ViewCommand::ComboBox {
                        id: "profile".into(),
                        selected_value: self.active_profile.clone().unwrap_or_default(),
                        options: profile_options,
                        width: 200.0,
                    },
                ],
            });
            cmds.push(ViewCommand::Space { height: 8.0 });
        }

        cmds.push(ViewCommand::HStack {
            children: vec![
                ViewCommand::Text {
                    content: "Provider".into(),
                    role: TextRole::Label,
                    size: 13.0,
                },
                ViewCommand::ComboBox {
                    id: "provider".into(),
                    selected_value: self.provider.clone(),
                    options: provider_options,
                    width: 200.0,
                },
            ],
        });
        cmds.push(ViewCommand::Space { height: 8.0 });
        cmds.push(ViewCommand::HStack {
            children: vec![
                ViewCommand::Text {
                    content: "Model".into(),
                    role: TextRole::Label,
                    size: 13.0,
                },
                ViewCommand::ComboBox {
                    id: "model".into(),
                    selected_value: self.model.clone(),
                    options: model_options,
                    width: 200.0,
                },
            ],
        });
        cmds.push(ViewCommand::Space { height: 8.0 });
        cmds.push(ViewCommand::HStack {
            children: vec![
                ViewCommand::Text {
                    content: "API Key".into(),
                    role: TextRole::Label,
                    size: 13.0,
                },
                ViewCommand::TextInput {
                    id: "api_key".into(),
                    value: self.api_key.clone().unwrap_or_default(),
                    placeholder: "${env:KIMI_API_KEY} or plain key".into(),
                    password: true,
                    width: 200.0,
                },
            ],
        });
        cmds.push(ViewCommand::Text {
            content: "Supports ${env:VAR_NAME} syntax to avoid storing keys on disk.".into(),
            role: TextRole::Body,
            size: 11.0,
        });
        cmds.push(ViewCommand::Space { height: 8.0 });
        cmds.push(ViewCommand::HStack {
            children: vec![
                ViewCommand::Text {
                    content: "Local Model Path".into(),
                    role: TextRole::Label,
                    size: 13.0,
                },
                ViewCommand::TextInput {
                    id: "local_model_path".into(),
                    value: self.local_model_path.clone().unwrap_or_default(),
                    placeholder: String::new(),
                    password: false,
                    width: 200.0,
                },
            ],
        });
        cmds.push(ViewCommand::Space { height: 8.0 });
        cmds.push(ViewCommand::HStack {
            children: vec![
                ViewCommand::Text {
                    content: "Approval Mode".into(),
                    role: TextRole::Label,
                    size: 13.0,
                },
                ViewCommand::ComboBox {
                    id: "approval_mode".into(),
                    selected_value: self.approval_mode.clone(),
                    options: approval_options,
                    width: 200.0,
                },
            ],
        });
        cmds.push(ViewCommand::Space { height: 8.0 });
        cmds.push(ViewCommand::Button {
            id: "clear_batch_grants".into(),
            label: "Clear Batch Grants".into(),
            style: ButtonStyle::Secondary,
            min_width: 160.0,
            min_height: 28.0,
        });
        cmds.push(ViewCommand::Space { height: 16.0 });
        cmds.push(ViewCommand::HStack {
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
        });

        cmds
    }

    /// Route a user action back into the ViewModel state.
    pub fn handle_action(&mut self, action: UserAction) {
        match action {
            UserAction::ComboChange { id, selected } if id == "profile" => {
                self.active_profile = if selected.is_empty() {
                    None
                } else {
                    Some(selected)
                };
                self.dirty = true;
            }
            UserAction::ComboChange { id, selected } if id == "provider" => {
                self.provider = selected.clone();
                if let Some((_, _, models)) = self
                    .available_models
                    .iter()
                    .find(|(k, _, _)| k == &selected)
                {
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
}

// ============================================================================
// Unit tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::capability::CapabilityRegistry;

    fn sample_catalog() -> Vec<ProviderModelEntry> {
        vec![
            (
                "openai".into(),
                "OpenAI".into(),
                vec!["gpt-4o".into(), "gpt-4o-mini".into()],
            ),
            (
                "kimi".into(),
                "Kimi".into(),
                vec!["kimi-k2.6".into(), "kimi-k2-07132k".into()],
            ),
            ("local".into(), "Local (GGUF)".into(), vec![]),
        ]
    }

    #[test]
    fn test_view_model_default() {
        let vm = SettingsViewModel::new();
        assert_eq!(vm.provider, "openai");
        assert_eq!(vm.model, "gpt-4o");
        // egui supports all approval modes including interactive, yolo, plan, smart
        let expected_mode =
            if CapabilityRegistry::supported_approval_modes("egui").contains(&"interactive") {
                "interactive"
            } else {
                "yolo"
            };
        assert_eq!(vm.approval_mode, expected_mode);
        assert!(!vm.is_dirty());
    }

    #[test]
    fn test_view_model_from_snapshot_roundtrip() {
        let vm = SettingsViewModel::new();
        let snapshot = vm.snapshot();
        let vm2 = SettingsViewModel::from_snapshot(&snapshot, Vec::new());
        assert_eq!(vm, vm2);
    }

    #[test]
    fn test_view_model_provider_change_cascades_model() {
        let mut vm = SettingsViewModel::new();
        vm.set_available_models(sample_catalog());
        vm.handle_action(UserAction::ComboChange {
            id: "provider".into(),
            selected: "kimi".into(),
        });
        assert_eq!(vm.provider, "kimi");
        assert_eq!(vm.model, "kimi-k2.6"); // first model for kimi
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
        let mut vm = SettingsViewModel::new();
        vm.set_available_models(sample_catalog());
        let cmds = vm.commands();
        assert!(!cmds.is_empty());
        // Should contain Provider, Model, API Key, Local Path, Approval Mode, Buttons
        assert!(cmds.iter().any(|c| matches!(c, ViewCommand::HStack { .. })));
    }

    #[test]
    fn test_commands_reflect_available_models() {
        let mut vm = SettingsViewModel::new();
        vm.set_available_models(sample_catalog());
        let cmds = vm.commands();
        // Find provider combobox and check options
        let provider_hstack = cmds.iter().find(|c| {
            matches!(c, ViewCommand::HStack { children } if children.iter().any(|c| matches!(c, ViewCommand::ComboBox { id, .. } if id == "provider")))
        });
        assert!(provider_hstack.is_some());
    }
}
