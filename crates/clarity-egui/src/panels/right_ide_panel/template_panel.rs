//! Templates panel — work-template injection for the right IDE rail.
//!
//! Provides a small library of built-in prompt templates that can be
//! injected into the chat input with one click.  Remote template
//! marketplace browsing is reserved as an extension point for future
//! backend features.

use crate::App;

/// A built-in work template.
pub(crate) struct BuiltInTemplate {
    name: &'static str,
    description: &'static str,
    icon: &'static str,
    prompt: &'static str,
}

/// Hardcoded template library. When the remote template marketplace
/// ships, these move to a `TemplateStore` that merges built-in and
/// remote sources.
const BUILT_IN_TEMPLATES: &[BuiltInTemplate] = &[
    BuiltInTemplate {
        name: "Code Review",
        description: "Review code for bugs, security issues, and style violations",
        icon: crate::theme::ICON_CHECK,
        prompt: "Please review the following code for bugs, security issues, and style violations. Provide specific, actionable feedback:\n\n```\n\n```",
    },
    BuiltInTemplate {
        name: "Bug Fix",
        description: "Investigate and fix a bug described below",
        icon: crate::theme::ICON_WARNING,
        prompt: "I need to fix a bug:\n\n**Steps to reproduce:**\n\n**Expected behavior:**\n\n**Actual behavior:**\n\n**Environment:**\n\nPlease investigate the root cause and propose a fix.",
    },
    BuiltInTemplate {
        name: "New Feature",
        description: "Implement a new feature from specification",
        icon: crate::theme::ICON_PLUS,
        prompt: "Please implement the following feature:\n\n**Goal:**\n\n**Requirements:**\n\n**Acceptance criteria:**\n\n",
    },
    BuiltInTemplate {
        name: "Refactor",
        description: "Restructure existing code without changing behavior",
        icon: crate::theme::ICON_WRENCH,
        prompt: "Please refactor the following code to improve clarity, performance, and maintainability without changing its external behavior:\n\n**Current issues:**\n\n**Target patterns:**\n\n",
    },
    BuiltInTemplate {
        name: "Write Tests",
        description: "Generate unit and integration tests for existing code",
        icon: crate::theme::ICON_FILE_CODE,
        prompt: "Please write comprehensive tests for the following code. Include:\n- Unit tests for edge cases\n- Integration tests where appropriate\n- Any necessary test fixtures\n\n",
    },
];

/// Render the templates panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    // Pre-translate button label to avoid mutable borrow conflict.
    let inject_label = app.t("Inject").to_string();

    // --- built-in templates ---
    ui.label(
        egui::RichText::new(app.t("Built-in Templates"))
            .size(theme.text_sm)
            .strong()
            .color(theme.text_strong),
    );
    ui.add_space(theme.space_8);

    egui::ScrollArea::vertical()
        .id_salt("template_list")
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for tmpl in BUILT_IN_TEMPLATES {
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
                            ui.add_space(theme.space_8);
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
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if ui
                                        .add_sized(
                                            [64.0, 24.0],
                                            egui::Button::new(
                                                egui::RichText::new(&inject_label)
                                                    .size(theme.text_xs)
                                                    .color(theme.text_strong),
                                            )
                                            .fill(theme.accent)
                                            .corner_radius(egui::CornerRadius::same(
                                                theme.radius_sm as u8,
                                            )),
                                        )
                                        .clicked()
                                    {
                                        app.chat_store.input = tmpl.prompt.to_string();
                                        app.ui_store.focus_input_requested = true;
                                        let toast_msg = app.t("Template injected").to_string();
                                        crate::handlers::system::push_toast(
                                            &mut app.ui_store,
                                            &toast_msg,
                                            crate::ui::types::ToastLevel::Info,
                                        );
                                    }
                                },
                            );
                        });
                    });

                ui.add_space(theme.space_8);
            }
        });

    ui.add_space(theme.space_16);

    // --- remote templates (extension point) ---
    ui.label(
        egui::RichText::new(app.t("Browse Templates"))
            .size(theme.text_sm)
            .strong()
            .color(theme.text_strong),
    );
    ui.add_space(theme.space_8);
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

// === Extension point type stubs ===

/// Remote template from the marketplace. Reserved for future backend integration.
#[allow(dead_code)]
pub struct RemoteTemplate {
    pub id: String,
    pub name: String,
    pub description: String,
    pub author: String,
    pub downloads: u64,
    pub tags: Vec<String>,
}

/// Template CRUD placeholder. Reserved for future backend integration.
#[allow(dead_code)]
pub struct TemplateStore {
    pub built_in: Vec<BuiltInTemplate>,
    pub remote_templates: Option<Vec<RemoteTemplate>>,
    pub search_query: String,
}

impl Default for TemplateStore {
    fn default() -> Self {
        Self {
            built_in: Vec::new(),
            remote_templates: None,
            search_query: String::new(),
        }
    }
}
