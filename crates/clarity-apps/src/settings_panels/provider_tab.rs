use crate::provider::ProviderDefinition;
use crate::settings::SettingsStore;
use clarity_shell::AppState;
use clarity_ui::design_system::{self, Space, TextStyle};

use super::provider_add_form;
use super::provider_detail;

/// Renders the provider UI.
pub fn render_provider(store: &mut SettingsStore, state: &mut dyn AppState, ui: &mut egui::Ui) {
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
