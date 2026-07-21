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
use std::collections::HashMap;

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

/// Per-provider model catalog refresh state.
///
/// The state machine is driven externally: the frontend sends
/// [`UserAction::RefreshModels`], the host performs the (async) catalog fetch
/// via `clarity_llm`, then reports the outcome through
/// [`SettingsViewModel::apply_refresh_result`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ModelRefreshState {
    /// No refresh has been requested for this provider.
    #[default]
    Idle,
    /// A catalog fetch is in flight.
    Loading,
    /// The fetch succeeded; carries the number of models received.
    Ready {
        /// Number of models the refreshed catalog contains.
        count: usize,
    },
    /// The fetch failed; carries a human-readable error message.
    Error {
        /// Human-readable failure reason.
        message: String,
    },
    /// The provider exposes no listable model catalog API (e.g. OAuth
    /// device-flow channels, device-relay providers, local GGUF files).
    Unsupported,
}

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
    refresh_states: HashMap<String, ModelRefreshState>,
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
            refresh_states: HashMap::new(),
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
            refresh_states: HashMap::new(),
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

    /// Current catalog refresh state for a provider (`Idle` when never touched).
    pub fn refresh_state(&self, provider: &str) -> &ModelRefreshState {
        self.refresh_states
            .get(provider)
            .unwrap_or(&ModelRefreshState::Idle)
    }

    /// Inject the catalog-pull capability for a provider.
    ///
    /// The host derives this from `clarity_llm::catalog::capability` (the
    /// single source of truth) and marks every listed provider before the
    /// settings UI is shown. `false` pins the provider to
    /// [`ModelRefreshState::Unsupported`]; `true` lifts that pin.
    pub fn set_catalog_supported(&mut self, provider: &str, supported: bool) {
        if supported {
            if self.refresh_states.get(provider) == Some(&ModelRefreshState::Unsupported) {
                self.refresh_states.remove(provider);
            }
        } else {
            self.refresh_states
                .insert(provider.to_string(), ModelRefreshState::Unsupported);
        }
    }

    /// Begin a catalog refresh and return the providers that transitioned to
    /// [`ModelRefreshState::Loading`].
    ///
    /// `None` targets every provider in `available_models`. Providers pinned
    /// to `Unsupported` or already `Loading` are skipped, so the returned
    /// list is exactly the work the host must perform. The host should fetch
    /// each returned provider and report back via
    /// [`Self::apply_refresh_result`].
    pub fn begin_refresh(&mut self, provider: Option<&str>) -> Vec<String> {
        let targets: Vec<String> = match provider {
            Some(id) => vec![id.to_string()],
            None => self
                .available_models
                .iter()
                .map(|(id, _, _)| id.clone())
                .collect(),
        };

        let mut started = Vec::new();
        for id in targets {
            match self.refresh_states.get(&id) {
                Some(ModelRefreshState::Unsupported | ModelRefreshState::Loading) => {}
                _ => {
                    self.refresh_states
                        .insert(id.clone(), ModelRefreshState::Loading);
                    started.push(id);
                }
            }
        }
        started
    }

    /// Report the outcome of a catalog fetch for a provider.
    ///
    /// On success the provider's model list in `available_models` is replaced
    /// (an entry is appended when the provider was not listed yet) and the
    /// state becomes `Ready`. On failure the existing model list is kept and
    /// the state becomes `Error`. The form's dirty flag is unaffected: a
    /// catalog refresh is not a user edit.
    pub fn apply_refresh_result(&mut self, provider: &str, result: Result<Vec<String>, String>) {
        let state = match result {
            Ok(models) => {
                let count = models.len();
                match self
                    .available_models
                    .iter_mut()
                    .find(|(id, _, _)| id == provider)
                {
                    Some((_, _, slot)) => *slot = models,
                    // ponytail: unknown providers reuse their id as display label;
                    // the next set_available_models injection restores proper labels.
                    None => self.available_models.push((
                        provider.to_string(),
                        provider.to_string(),
                        models,
                    )),
                }
                ModelRefreshState::Ready { count }
            }
            Err(message) => ModelRefreshState::Error { message },
        };
        self.refresh_states.insert(provider.to_string(), state);
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
            UserAction::RefreshModels { provider } => {
                self.begin_refresh(provider.as_deref());
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

    // ── Catalog refresh state machine ───────────────────────────────────────

    #[test]
    fn test_refresh_state_defaults_to_idle() {
        let vm = SettingsViewModel::new();
        assert_eq!(vm.refresh_state("openai"), &ModelRefreshState::Idle);
    }

    #[test]
    fn test_refresh_models_action_starts_loading() {
        let mut vm = SettingsViewModel::new();
        vm.set_available_models(sample_catalog());
        vm.handle_action(UserAction::RefreshModels {
            provider: Some("openai".into()),
        });
        assert_eq!(vm.refresh_state("openai"), &ModelRefreshState::Loading);
        assert_eq!(vm.refresh_state("kimi"), &ModelRefreshState::Idle);
        assert!(!vm.is_dirty(), "catalog refresh must not dirty the form");
    }

    #[test]
    fn test_begin_refresh_none_targets_all_capable_providers() {
        let mut vm = SettingsViewModel::new();
        vm.set_available_models(sample_catalog());
        vm.set_catalog_supported("local", false);

        let started = vm.begin_refresh(None);
        assert_eq!(started, vec!["openai".to_string(), "kimi".to_string()]);
        assert_eq!(vm.refresh_state("local"), &ModelRefreshState::Unsupported);
    }

    #[test]
    fn test_begin_refresh_skips_unsupported_and_in_flight() {
        let mut vm = SettingsViewModel::new();
        vm.set_available_models(sample_catalog());
        vm.set_catalog_supported("openai", false);
        assert!(vm.begin_refresh(Some("openai")).is_empty());

        assert_eq!(vm.begin_refresh(Some("kimi")), vec!["kimi".to_string()]);
        // Already Loading: no duplicate work.
        assert!(vm.begin_refresh(Some("kimi")).is_empty());
    }

    #[test]
    fn test_set_catalog_supported_true_lifts_unsupported_pin() {
        let mut vm = SettingsViewModel::new();
        vm.set_catalog_supported("openai", false);
        assert_eq!(vm.refresh_state("openai"), &ModelRefreshState::Unsupported);
        vm.set_catalog_supported("openai", true);
        assert_eq!(vm.refresh_state("openai"), &ModelRefreshState::Idle);
    }

    #[test]
    fn test_apply_refresh_result_writes_models_back() {
        let mut vm = SettingsViewModel::new();
        vm.set_available_models(sample_catalog());
        vm.begin_refresh(Some("openai"));

        vm.apply_refresh_result(
            "openai",
            Ok(vec!["gpt-4o".into(), "gpt-4.1".into(), "o3".into()]),
        );

        assert_eq!(
            vm.refresh_state("openai"),
            &ModelRefreshState::Ready { count: 3 }
        );
        let (_, _, models) = vm
            .available_models()
            .iter()
            .find(|(id, _, _)| id == "openai")
            .expect("openai entry");
        assert_eq!(models, &["gpt-4o", "gpt-4.1", "o3"]);
    }

    #[test]
    fn test_apply_refresh_result_inserts_unknown_provider() {
        let mut vm = SettingsViewModel::new();
        vm.apply_refresh_result("custom", Ok(vec!["m1".into()]));
        assert_eq!(
            vm.refresh_state("custom"),
            &ModelRefreshState::Ready { count: 1 }
        );
        assert!(
            vm.available_models()
                .iter()
                .any(|(id, _, _)| id == "custom")
        );
    }

    #[test]
    fn test_apply_refresh_error_keeps_existing_models() {
        let mut vm = SettingsViewModel::new();
        vm.set_available_models(sample_catalog());
        vm.begin_refresh(Some("kimi"));

        vm.apply_refresh_result("kimi", Err("connection refused".into()));

        assert_eq!(
            vm.refresh_state("kimi"),
            &ModelRefreshState::Error {
                message: "connection refused".into()
            }
        );
        let (_, _, models) = vm
            .available_models()
            .iter()
            .find(|(id, _, _)| id == "kimi")
            .expect("kimi entry");
        assert_eq!(models, &["kimi-k2.6", "kimi-k2-07132k"]);
    }

    #[test]
    fn test_refresh_after_error_can_retry() {
        let mut vm = SettingsViewModel::new();
        vm.set_available_models(sample_catalog());
        vm.begin_refresh(Some("openai"));
        vm.apply_refresh_result("openai", Err("boom".into()));

        assert_eq!(vm.begin_refresh(Some("openai")), vec!["openai".to_string()]);
        assert_eq!(vm.refresh_state("openai"), &ModelRefreshState::Loading);
    }
}
