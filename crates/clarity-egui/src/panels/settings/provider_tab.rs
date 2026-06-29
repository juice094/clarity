use crate::App;
use crate::design_system::{self, Space, TextStyle};
use crate::provider::ProviderDefinition;

/// Renders the provider UI.
pub fn render_provider(app: &mut App, ui: &mut egui::Ui) {
    let left_w = (ui.available_width() * 0.35).clamp(180.0, 260.0);

    ui.horizontal(|ui| {
        // ── Left column: provider list ──
        ui.allocate_ui_with_layout(
            egui::vec2(left_w, ui.available_height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| render_left_column(app, ui),
        );

        ui.add_space(app.ui_store.theme.space_12);

        // ── Right column: detail / add form / empty state ──
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), ui.available_height()),
            egui::Layout::top_down(egui::Align::Min),
            |ui| render_right_column(app, ui),
        );
    });
}

// ---------------------------------------------------------------------------
// Left column
// ---------------------------------------------------------------------------
fn render_left_column(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    design_system::text(ui, app.t("Provider"), TextStyle::Subheading);
    design_system::gap(ui, Space::S0);
    design_system::text(ui, "Connect to an AI service", TextStyle::Small);
    design_system::gap(ui, Space::S2);

    // Settings should always list every configured provider, including chat-only
    // ones like deepseek-device. The active-session context only affects whether
    // a chat-only provider can be *used* for the current turn, not whether the
    // user is allowed to view or configure it.
    let all: Vec<ProviderDefinition> = app
        .settings_store
        .provider_registry
        .list()
        .into_iter()
        .cloned()
        .collect();
    let current = app.settings_store.settings_edit.provider.clone();

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
                let resp = crate::widgets::provider_row(
                    ui,
                    &theme,
                    p.display(),
                    has_key,
                    p.models.len(),
                    is_active,
                );

                if resp.clicked() && !is_active {
                    app.settings_store.settings_edit.provider = id.clone();
                    app.settings_store.show_add_provider = false;
                    if let Some(prov) = app.settings_store.provider_registry.get(&id) {
                        if !prov.models.is_empty() {
                            app.settings_store.settings_edit.model = prov.models[0].clone();
                        }
                    }
                    app.auto_save_settings();
                }
            }
        });

    crate::design_system::gap(ui, crate::design_system::Space::S2);

    if ui.add(theme.primary_button("+ Add Custom")).clicked() {
        app.settings_store.show_add_provider = !app.settings_store.show_add_provider;
    }
}

// ---------------------------------------------------------------------------
// Right column
// ---------------------------------------------------------------------------
fn render_right_column(app: &mut App, ui: &mut egui::Ui) {
    if app.settings_store.show_add_provider {
        super::provider_add_form::render_add_form(app, ui);
        return;
    }

    let current = app.settings_store.settings_edit.provider.clone();
    let prov_opt = app.settings_store.provider_registry.get(&current).cloned();

    if let Some(prov) = prov_opt {
        super::provider_detail::render_provider_detail(app, ui, prov);
    } else {
        render_empty_state(app, ui);
    }
}

fn render_empty_state(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    ui.vertical_centered(|ui| {
        ui.add_space(theme.space_40 * 2.0);
        ui.label(
            egui::RichText::new("Select a provider")
                .font(theme.font(theme.text_md))
                .color(theme.text_dim),
        );
        crate::design_system::gap(ui, crate::design_system::Space::S0);
        ui.label(
            egui::RichText::new("Choose from the list or add a custom provider")
                .font(theme.font(theme.text_sm))
                .color(theme.text_muted),
        );
    });
}

// ── Panel trait implementation ──

pub struct ProviderPanel;

impl crate::design_system::Panel for ProviderPanel {
    fn title(&self, _app: &crate::App) -> &str {
        "Provider"
    }

    fn render(&mut self, app: &mut crate::App, ui: &mut egui::Ui) {
        render_provider(app, ui);
    }
}
