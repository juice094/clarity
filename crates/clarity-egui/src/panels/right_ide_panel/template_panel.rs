//! Templates panel — work-template injection for the right IDE rail.
//!
//! Provides a small library of built-in prompt templates that can be
//! injected into the chat input with one click.  Remote template
//! marketplace browsing is reserved as an extension point for future
//! backend features.

use crate::App;
use crate::design_system::{self, TextStyle};
use crate::stores::FocusTarget;
use crate::stores::template::BuiltInTemplate;
use clarity_ui::widgets::button::Button;

/// Render the templates panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.context.ui_store.theme.clone();
    // Pre-translate button label to avoid mutable borrow conflict.
    let inject_label = app.t("Inject").to_string();
    let templates: Vec<BuiltInTemplate> = app.context.template_store.built_in.clone();

    // --- built-in templates ---
    ui.label(
        egui::RichText::new(app.t("Built-in Templates"))
            .size(theme.text_sm)
            .strong()
            .color(theme.text_strong),
    );
    crate::design_system::gap(ui, crate::design_system::Space::S1);

    egui::ScrollArea::vertical()
        .id_salt("template_list")
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for tmpl in &templates {
                render_template_card(app, ui, tmpl, &inject_label, &theme);
                crate::design_system::gap(ui, crate::design_system::Space::S1);
            }
        });

    crate::design_system::gap(ui, crate::design_system::Space::S3);

    // --- remote templates (extension point) ---
    ui.label(
        egui::RichText::new(app.t("Browse Templates"))
            .size(theme.text_sm)
            .strong()
            .color(theme.text_strong),
    );
    crate::design_system::gap(ui, crate::design_system::Space::S1);
    ui.add_enabled(
        false,
        Button::new(app.t("Open Template Marketplace")).ghost(),
    )
    .on_disabled_hover_text(app.t("Template marketplace coming soon"));
    ui.label(
        egui::RichText::new(app.t("Remote template browsing will be available in a future update"))
            .size(theme.text_xs)
            .color(theme.text_dim)
            .italics(),
    );
}

fn render_template_card(
    app: &mut App,
    ui: &mut egui::Ui,
    tmpl: &BuiltInTemplate,
    inject_label: &str,
    theme: &crate::theme::Theme,
) {
    design_system::card(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(tmpl.icon)
                    .font(theme.font_icon(theme.text_md))
                    .color(theme.accent),
            );
            crate::design_system::gap(ui, crate::design_system::Space::S1);
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(tmpl.name)
                        .size(theme.text_sm)
                        .strong()
                        .color(theme.text),
                );
                design_system::text(ui, tmpl.description, TextStyle::Small);
            });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add_sized(
                        [theme.space_16 * 4.0, theme.size_input],
                        Button::new(inject_label).primary().small(),
                    )
                    .clicked()
                {
                    app.chat_store_mut().input = tmpl.prompt.to_string();
                    app.context.ui_store.focus_target = Some(FocusTarget::ChatInput);
                    let toast_msg = app.t("Template injected").to_string();
                    crate::handlers::system::push_toast(
                        &mut app.context.ui_store,
                        &toast_msg,
                        crate::ui::types::ToastLevel::Info,
                    );
                }
            });
        });
    });
}

// ── Panel trait implementation ──

/// Templates panel renderer.
pub struct TemplatesPanel;

impl crate::design_system::Panel for TemplatesPanel {
    fn title(&self, app: &crate::App) -> &str {
        app.t("Templates")
    }
    fn render(&mut self, app: &mut crate::App, ui: &mut egui::Ui) {
        render(app, ui);
    }
}
