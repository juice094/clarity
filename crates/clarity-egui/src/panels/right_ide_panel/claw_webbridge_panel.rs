//! Claw WebBridge panel in the right IDE rail.
//!
//! Embedded web / code viewer. The user can enter a URL and the panel
//! fetches and displays its content via the Gateway proxy. Supports
//! both web pages (rendered as text) and source code files.

use crate::App;

// LAYOUT-EXEMPT: compact URL input height local to this panel.
const URL_INPUT_H: f32 = 24.0;

/// Render the Claw WebBridge panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.context.ui_store.theme.clone();

    // URL input bar.
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(crate::theme::ICON_GLOBE)
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        let url_resp = ui.add_sized(
            egui::vec2(ui.available_width() - theme.space_4, URL_INPUT_H),
            clarity_ui::widgets::TextInput::singleline(&mut app.chat_store_mut().input)
                .hint_text("https://example.com or /path/to/file")
                .font(theme.font_mono(theme.text_base)),
        );
        if url_resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
            let url = app.chat_store().input.trim().to_string();
            if !url.is_empty() {
                app.push_toast(
                    format!("Loading: {}", url),
                    crate::ui::types::ToastLevel::Info,
                );
            }
        }
    });

    crate::design_system::gap(ui, crate::design_system::Space::S1);

    // Content area.
    // LAYOUT-EXEMPT: viewport reserve/maximum tied to panel proportions, not grid.
    let content_height = (ui.available_height() - 40.0).max(200.0);
    egui::ScrollArea::vertical()
        .max_height(content_height)
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            // Show the active project file content if available.
            let has_content = app.context.ui_store.active_project.is_some();
            if has_content {
                let path = app.context.ui_store.active_project.as_deref().unwrap_or("");
                ui.label(
                    egui::RichText::new(format!("{} {}", crate::theme::ICON_FILE_CODE, path))
                        .size(theme.text_xs)
                        .color(theme.text_muted),
                );
                crate::design_system::gap(ui, crate::design_system::Space::S1);

                // Placeholder: show a styled preview area.
                let preview_text = format!(
                    "// Content of {}\n//\n// WebBridge fetches and displays web pages,\n// source code, and documentation from the\n// selected Claw device via the Gateway.\n//\n// Features planned:\n// - Syntax highlighting (Tree-sitter)\n// - HTML/Markdown rendering\n// - Web page proxy view\n// - Line numbers",
                    path
                );
                crate::design_system::code_frame(ui, |ui| {
                    ui.label(
                        egui::RichText::new(&preview_text)
                            .size(theme.text_sm)
                            .color(theme.text)
                            .code(),
                    );
                });
            } else {
                crate::design_system::gap(ui, crate::design_system::Space::S3);
                ui.vertical_centered(|ui| {
                    ui.label(
                        egui::RichText::new(crate::theme::ICON_GLOBE)
                            .size(theme.text_xl)
                            .color(theme.text_dim),
                    );
                    crate::design_system::gap(ui, crate::design_system::Space::S1);
                    ui.label(
                        egui::RichText::new(app.t("Enter a URL or file path to preview"))
                            .size(theme.text_sm)
                            .color(theme.text_muted),
                    );
                    crate::design_system::gap(ui, crate::design_system::Space::S0);
                    ui.label(
                        egui::RichText::new(app.t(
                            "WebBridge lets you view web pages and code side-by-side with your chat",
                        ))
                        .size(theme.text_xs)
                        .color(theme.text_dim),
                    );
                });
            }
        });
}

// ── Panel trait implementation ──

/// Claw WebBridge panel renderer.
pub struct ClawWebBridgePanel;

impl crate::design_system::Panel for ClawWebBridgePanel {
    fn title(&self, app: &crate::App) -> &str {
        app.t("WebBridge")
    }
    fn render(&mut self, app: &mut crate::App, ui: &mut egui::Ui) {
        render(app, ui);
    }
}
