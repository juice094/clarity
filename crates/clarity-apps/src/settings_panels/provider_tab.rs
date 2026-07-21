use crate::provider::ProviderDefinition;
use crate::settings::SettingsStore;
use clarity_core::view_models::settings::SettingsViewModel;
use clarity_shell::AppState;
use clarity_ui::design_system::{self, Space, TextStyle};

use super::provider_add_form;
use super::provider_detail;

/// Whether a provider's model catalog can be pulled from a remote API.
///
/// Delegates to `clarity_llm::catalog::capability`, the single source of
/// truth: built-in providers are judged by their canonical family (which
/// knows about OAuth device-flow channels like Kimi Code and local GGUF),
/// custom providers by the runtime config they would produce.
pub(crate) fn provider_supports_catalog(def: &ProviderDefinition) -> bool {
    if def.builtin {
        // Note: the built-in Kimi Code id is `kimi_code` while the canonical
        // family is `kimi-code`; unknown families are denied by default, which
        // is exactly the correct verdict for that OAuth channel.
        clarity_llm::catalog::capability::family_supports_catalog(&def.id)
    } else {
        clarity_llm::runtime::RuntimeProviderConfig {
            provider_id: def.id.clone(),
            base_url: def.base_url.clone(),
            api_format: def.api_format.runtime_api_format().to_string(),
            api_key: String::new(),
            model: String::new(),
        }
        .supports_model_catalog()
    }
}

/// Inject catalog-pull capability for every known provider into the
/// ViewModel, pinning incapable channels to `ModelRefreshState::Unsupported`.
///
/// Idempotent and cheap (the registry holds a handful of entries), so the
/// provider page re-applies it on every open/frame.
pub(crate) fn apply_catalog_capabilities<'a>(
    vm: &mut SettingsViewModel,
    defs: impl IntoIterator<Item = &'a ProviderDefinition>,
) {
    for def in defs {
        vm.set_catalog_supported(&def.id, provider_supports_catalog(def));
    }
}

/// Renders the provider UI.
pub fn render_provider(store: &mut SettingsStore, state: &mut dyn AppState, ui: &mut egui::Ui) {
    apply_catalog_capabilities(&mut store.settings_vm, store.provider_registry.list());

    let left_w = (ui.available_width() * 0.35).clamp(180.0, 260.0);
    let theme = state.theme().clone();

    ui.horizontal(|ui| {
        // ── Left column: provider list ──
        ui.allocate_ui_with_layout(
            egui::vec2(left_w, ui.available_height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| render_left_column(store, state, &theme, ui),
        );

        ui.add_space(theme.space_12);

        // ── Right column: detail / add form / empty state ──
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), ui.available_height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| render_right_column(store, state, &theme, ui),
        );
    });
}

// ---------------------------------------------------------------------------
// Left column
// ---------------------------------------------------------------------------
fn render_left_column(
    store: &mut SettingsStore,
    state: &mut dyn AppState,
    theme: &clarity_ui::theme::Theme,
    ui: &mut egui::Ui,
) {
    design_system::text(ui, state.t("Provider"), TextStyle::Subheading);
    design_system::gap(ui, Space::S0);
    design_system::text(ui, "Connect to an AI service", TextStyle::Small);
    design_system::gap(ui, Space::S2);

    // Settings should always list every configured provider, including chat-only
    // ones like deepseek-device. The active-session context only affects whether
    // a chat-only provider can be *used* for the current turn, not whether the
    // user is allowed to view or configure it.
    let all: Vec<ProviderDefinition> = store
        .provider_registry
        .list()
        .into_iter()
        .cloned()
        .collect();
    let current = store.settings_edit.provider.clone();

    egui::ScrollArea::vertical()
        .min_scrolled_height(200.0)
        .show(ui, |ui| {
            for p in &all {
                let is_active = p.id == current;
                let id = p.id.clone();
                let has_key = !p.api_key_ref.is_empty() && p.resolve_api_key().is_some();

                // S4-α (2026-05-11): extracted to widgets/provider_row.rs.
                // The previous inline implementation used `allocate_exact_size +
                // Sense::click()` plus two `painter.rect_filled` calls. Both
                // painter calls are gone; the widget uses `Frame::fill` +
                // `Frame::stroke` for backgrounds and active accent.
                let resp = clarity_ui::widgets::provider_row(
                    ui,
                    theme,
                    p.display(),
                    has_key,
                    p.models.len(),
                    is_active,
                );

                if resp.clicked() && !is_active {
                    store.settings_edit.provider = id.clone();
                    store.show_add_provider = false;
                    if let Some(prov) = store.provider_registry.get(&id) {
                        if !prov.models.is_empty() {
                            store.settings_edit.model = prov.models[0].clone();
                        }
                    }
                    state.auto_save_settings();
                }
            }
        });

    design_system::gap(ui, Space::S2);

    if ui.add(theme.primary_button("+ Add Custom")).clicked() {
        store.show_add_provider = !store.show_add_provider;
    }
}

// ---------------------------------------------------------------------------
// Right column
// ---------------------------------------------------------------------------
fn render_right_column(
    store: &mut SettingsStore,
    state: &mut dyn AppState,
    theme: &clarity_ui::theme::Theme,
    ui: &mut egui::Ui,
) {
    if store.show_add_provider {
        provider_add_form::render_add_form(store, state, theme, ui);
        return;
    }

    let current = store.settings_edit.provider.clone();
    let prov_opt = store.provider_registry.get(&current).cloned();

    if let Some(prov) = prov_opt {
        provider_detail::render_provider_detail(store, state, theme, ui, prov);
    } else {
        render_empty_state(state, theme, ui);
    }
}

fn render_empty_state(
    _state: &mut dyn AppState,
    theme: &clarity_ui::theme::Theme,
    ui: &mut egui::Ui,
) {
    ui.vertical_centered(|ui| {
        ui.add_space(theme.space_40 * 2.0);
        ui.label(
            egui::RichText::new("Select a provider")
                .font(theme.font(theme.text_md))
                .color(theme.text_dim),
        );
        design_system::gap(ui, Space::S0);
        ui.label(
            egui::RichText::new("Choose from the list or add a custom provider")
                .font(theme.font(theme.text_sm))
                .color(theme.text_muted),
        );
    });
}

// ============================================================================
// Unit tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::{ApiFormat, AuthType};
    use clarity_core::view_models::settings::ModelRefreshState;

    fn def(
        id: &str,
        builtin: bool,
        api_format: ApiFormat,
        auth_type: AuthType,
    ) -> ProviderDefinition {
        ProviderDefinition {
            id: id.to_string(),
            builtin,
            api_format,
            auth_type,
            ..Default::default()
        }
    }

    #[test]
    fn builtin_family_matrix() {
        for supported in ["openai", "deepseek", "kimi"] {
            assert!(
                provider_supports_catalog(&def(
                    supported,
                    true,
                    ApiFormat::OpenaiCompletions,
                    AuthType::ApiKey
                )),
                "{supported} should support catalog pull"
            );
        }
        for unsupported in ["anthropic", "deepseek-device", "kimi_code", "local"] {
            let api_format = match unsupported {
                "anthropic" => ApiFormat::AnthropicMessages,
                "deepseek-device" => ApiFormat::DeepSeekDevice,
                _ => ApiFormat::OpenaiCompletions,
            };
            assert!(
                !provider_supports_catalog(&def(unsupported, true, api_format, AuthType::ApiKey)),
                "{unsupported} should NOT support catalog pull"
            );
        }
    }

    #[test]
    fn custom_provider_uses_runtime_config_verdict() {
        assert!(provider_supports_catalog(&def(
            "my-openai",
            false,
            ApiFormat::OpenaiCompletions,
            AuthType::ApiKey
        )));
        assert!(!provider_supports_catalog(&def(
            "my-anthropic",
            false,
            ApiFormat::AnthropicMessages,
            AuthType::ApiKey
        )));
        assert!(!provider_supports_catalog(&def(
            "my-device",
            false,
            ApiFormat::DeepSeekDevice,
            AuthType::ApiKey
        )));
    }

    #[test]
    fn apply_catalog_capabilities_pins_unsupported_providers() {
        let defs = [
            def(
                "openai",
                true,
                ApiFormat::OpenaiCompletions,
                AuthType::ApiKey,
            ),
            def("local", true, ApiFormat::OpenaiCompletions, AuthType::None),
        ];

        let mut vm = SettingsViewModel::new();
        apply_catalog_capabilities(&mut vm, defs.iter());

        assert_eq!(vm.refresh_state("openai"), &ModelRefreshState::Idle);
        assert_eq!(vm.refresh_state("local"), &ModelRefreshState::Unsupported);
        // begin_refresh must skip the pinned provider.
        assert_eq!(vm.begin_refresh(Some("local")), Vec::<String>::new());
        assert_eq!(vm.begin_refresh(Some("openai")), vec!["openai".to_string()]);
    }
}
