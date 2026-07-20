//! Share panel — session export and sharing for the right IDE rail.
//!
//! Supports exporting the active session as Markdown, JSON, or HTML.
//! Gateway-based sharing link and team/public visibility controls are
//! reserved as extension points for future backend features.

use crate::App;
use crate::design_system::{self, Space};
use crate::stores::share::ExportFormat;
use clarity_ui::widgets::button::Button;

/// Render the share panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.context.ui_store.theme.clone();

    // --- export format selector ---
    ui.label(
        egui::RichText::new(app.t("Export format"))
            .size(theme.text_sm)
            .strong()
            .color(theme.text_strong),
    );
    design_system::gap(ui, Space::S1);

    for fmt in [
        ExportFormat::Markdown,
        ExportFormat::Json,
        ExportFormat::Html,
    ] {
        let active = app.context.ui_store.theme.clone();
        let is_chosen = format_choice(app) == fmt;
        let frame = clarity_ui::design_system::Elevation::Elevated
            .frame(&active)
            .fill(if is_chosen {
                active.accent_subtle
            } else {
                active.surface
            })
            .stroke(egui::Stroke::new(
                1.0,
                if is_chosen {
                    active.accent
                } else {
                    active.border
                },
            ))
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
            .inner_margin(egui::Margin::symmetric(
                theme.space_12 as i8,
                theme.space_8 as i8,
            ))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                clarity_ui::design_system::text_with_color(
                    ui,
                    app.t(fmt.label_key()),
                    clarity_ui::design_system::TextStyle::Body,
                    if is_chosen {
                        active.accent
                    } else {
                        active.text
                    },
                );
            });
        if frame.response.clicked() {
            set_format_choice(app, fmt);
        }
        design_system::gap(ui, Space::S0);
    }

    design_system::gap(ui, Space::S3);

    // --- preview ---
    ui.label(
        egui::RichText::new(app.t("Preview"))
            .size(theme.text_sm)
            .strong()
            .color(theme.text_strong),
    );
    design_system::gap(ui, Space::S1);

    let preview = generate_preview(app);
    egui::ScrollArea::vertical()
        .id_salt("share_preview")
        // LAYOUT-EXEMPT: preview viewport height; tied to panel content, not spacing grid.
        .max_height(200.0)
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            crate::design_system::code_frame(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(
                    egui::RichText::new(&preview)
                        .size(theme.text_xs)
                        .color(theme.text_dim)
                        .monospace(),
                );
            });
        });

    design_system::gap(ui, Space::S3);

    // --- actions ---
    ui.horizontal(|ui| {
        if ui
            .add_sized(
                [ui.available_width(), theme.size_input],
                Button::new(app.t("Copy to Clipboard")).primary(),
            )
            .clicked()
        {
            let content = generate_export(app);
            ui.ctx().copy_text(content);
            let toast_msg = app.t("Copied to clipboard").to_string();
            crate::handlers::system::push_toast(
                &mut app.context.ui_store,
                &toast_msg,
                crate::ui::types::ToastLevel::Info,
            );
        }
    });
    design_system::gap(ui, Space::S1);
    ui.horizontal(|ui| {
        if ui
            .add_sized(
                [ui.available_width(), theme.size_input],
                Button::new(app.t("Save to File")).primary(),
            )
            .clicked()
        {
            let content = generate_export(app);
            let ext = format_choice(app).extension();
            let default_name = session_export_name(app, ext);
            let task = rfd::FileDialog::new()
                .set_file_name(&default_name)
                .save_file();
            // SAFE: spawn on runtime so rfd modal doesn't block the egui frame.
            let content_clone = content.clone();
            app.context.runtime.spawn(async move {
                if let Some(path) = task {
                    let _ = std::fs::write(&path, &content_clone);
                }
            });
        }
    });

    design_system::gap(ui, Space::S3);

    // --- share section (extension point) ---
    ui.label(
        egui::RichText::new(app.t("Share"))
            .size(theme.text_sm)
            .strong()
            .color(theme.text_strong),
    );
    design_system::gap(ui, Space::S1);

    ui.add_enabled(false, Button::new(app.t("Copy Share Link")).ghost())
        .on_disabled_hover_text(app.t("Requires Gateway server with sharing enabled"));

    design_system::gap(ui, Space::S0);

    // Visibility control stubs.
    for target in &["Team", "Public"] {
        ui.add_enabled_ui(false, |ui| {
            let _ = ui.selectable_label(
                false,
                egui::RichText::new(app.t(target))
                    .size(theme.text_sm)
                    .color(theme.text_dim),
            );
        });
    }
    ui.label(
        egui::RichText::new(app.t("Visibility controls coming soon"))
            .size(theme.text_xs)
            .color(theme.text_dim)
            .italics(),
    );
}

fn format_choice(app: &App) -> ExportFormat {
    app.context.share_store.export_format
}

fn set_format_choice(app: &mut App, fmt: ExportFormat) {
    app.context.share_store.export_format = fmt;
}

// ── Export generators ──

fn session_export_name(app: &App, ext: &str) -> String {
    let title = app
        .context
        .session_store
        .active_session()
        .map(|s| s.title.clone())
        .unwrap_or_else(|| "session".into());
    let sanitized: String = title
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .take(64)
        .collect();
    format!("{}.{}", sanitized, ext)
}

fn generate_preview(app: &App) -> String {
    let content = generate_export_inner(app, format_choice(app));
    // Show first 20 lines.
    content.lines().take(20).collect::<Vec<_>>().join("\n")
}

fn generate_export(app: &App) -> String {
    generate_export_inner(app, format_choice(app))
}

fn generate_export_inner(app: &App, fmt: ExportFormat) -> String {
    let session = match app.context.session_store.active_session() {
        Some(s) => s,
        None => return app.t("No active session").to_string(),
    };

    match fmt {
        ExportFormat::Markdown => {
            let mut out = String::new();
            out.push_str(&format!("# {}\n\n", session.title));
            for msg in &session.messages {
                let role = match msg.role {
                    crate::ui::types::Role::User => "**You**",
                    crate::ui::types::Role::Agent => "**Clarity**",
                    crate::ui::types::Role::System => "**System**",
                };
                out.push_str(&format!("### {}\n\n{}\n\n", role, msg.content));
            }
            out
        }
        ExportFormat::Json => {
            let messages: Vec<serde_json::Value> = session
                .messages
                .iter()
                .map(|m| {
                    serde_json::json!({
                        "role": match m.role {
                            crate::ui::types::Role::User => "user",
                            crate::ui::types::Role::Agent => "assistant",
                            crate::ui::types::Role::System => "system",
                        },
                        "content": m.content,
                    })
                })
                .collect();
            serde_json::json!({
                "title": session.title,
                "messages": messages,
            })
            .to_string()
        }
        ExportFormat::Html => {
            let mut out = String::from(
                "<!DOCTYPE html>\n<html><head><meta charset=\"utf-8\">\n<style>body{font-family:system-ui;max-width:720px;margin:2rem auto;background:#121212;color:#d6d6d6}h1{color:#1a88ff}.user{color:#a0a0a0}.agent{color:#d6d6d6}pre{background:#0d0d0d;padding:1rem;border-radius:8px}</style>\n</head><body>\n",
            );
            out.push_str(&format!("<h1>{}</h1>\n", html_escape(&session.title)));
            for msg in &session.messages {
                let cls = match msg.role {
                    crate::ui::types::Role::User => "user",
                    _ => "agent",
                };
                let escaped = html_escape(&msg.content);
                // Simple markdown code fences -> <pre>
                let body = escaped.replace("```", "<pre>").replace("```", "</pre>");
                out.push_str(&format!("<div class=\"{}\">{}</div>\n", cls, body));
            }
            out.push_str("</body></html>");
            out
        }
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ── Panel trait implementation ──

/// Share panel renderer.
pub struct SharePanel;

impl crate::design_system::Panel for SharePanel {
    fn title(&self, app: &crate::App) -> &str {
        app.t("Share")
    }
    fn render(&mut self, app: &mut crate::App, ui: &mut egui::Ui) {
        render(app, ui);
    }
}
