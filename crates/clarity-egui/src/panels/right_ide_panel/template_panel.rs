//! Templates panel — work-template injection for the right IDE rail.
//!
//! Provides a small library of built-in prompt templates that can be
//! injected into the chat input with one click.  Remote template
//! marketplace browsing is reserved as an extension point for future
//! backend features.

use crate::App;
use crate::stores::FocusTarget;
use crate::stores::template::BuiltInTemplate;

/// Render the templates panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    // Pre-translate button label to avoid mutable borrow conflict.
    let inject_label = app.t("Inject").to_string();
    let templates: Vec<BuiltInTemplate> = app.template_store.built_in.clone();

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
    ui.add_enabled_ui(false, |ui| {
        ui.set_min_width(ui.available_width());
        if ui
            .button(app.t("Open Template Marketplace"))
            .on_disabled_hover_text(app.t("Template marketplace coming soon"))
            .clicked()
        {}
    });
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
    let _card = egui::Frame::new()
        .fill(theme.surface)
        .stroke(egui::Stroke::new(1.0, theme.border))
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .inner_margin(egui::Margin::symmetric(
            theme.space_12 as i8,
            theme.space_8 as i8,
        ))
        .show(ui, |ui| {
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
                    ui.label(
                        egui::RichText::new(tmpl.description)
                            .size(theme.text_xs)
                            .color(theme.text_dim),
                    );
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_sized(
                            [theme.space_16 * 4.0, theme.size_input],
                            egui::Button::new(
                                egui::RichText::new(inject_label)
                                    .size(theme.text_xs)
                                    .color(theme.text_strong),
                            )
                            .fill(theme.accent)
                            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                        )
                        .clicked()
                    {
                        app.chat_store.input = tmpl.prompt.to_string();
                        app.ui_store.focus_target = Some(FocusTarget::ChatInput);
                        let toast_msg = app.t("Template injected").to_string();
                        crate::handlers::system::push_toast(
                            &mut app.ui_store,
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
