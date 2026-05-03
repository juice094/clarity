use crate::App;
use crate::ui::types::{UiEvent, WebTab};

/// Render a collapsible Web Tabs panel for the left sidebar.
pub fn render_web_tabs(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme;
    let expanded = app.ui_store.web_tabs_expanded;

    // ── Header bar ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Web Tabs")
                .size(theme.text_sm)
                .strong()
                .color(theme.text),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let arrow = if expanded { "▼" } else { "▶" };
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(arrow).size(theme.text_sm))
                        .fill(egui::Color32::TRANSPARENT)
                        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                )
                .clicked()
            {
                app.ui_store.web_tabs_expanded = !expanded;
            }
        });
    });

    if !expanded {
        return;
    }

    ui.add_space(theme.space_8);

    if app.ui_store.web_tabs.is_empty() {
        ui.label(
            egui::RichText::new("No web tabs yet.")
                .size(theme.text_xs)
                .color(theme.text_dim),
        );
    } else {
        egui::Frame::new()
            .fill(theme.surface)
            .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8))
            .inner_margin(egui::Margin::same(12))
            .show(ui, |ui| {
                for idx in 0..app.ui_store.web_tabs.len() {
                    let tab = app.ui_store.web_tabs[idx].clone();
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("🌐").size(theme.text_sm));

                        let title = if tab.title.is_empty() {
                            &tab.url
                        } else {
                            &tab.title
                        };
                        let display = crate::ui::render::truncate(title, 24);

                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(display)
                                        .size(theme.text_sm)
                                        .color(theme.text),
                                )
                                .fill(egui::Color32::TRANSPARENT)
                                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                            )
                            .clicked()
                        {
                            let url = tab.url.clone();
                            let tx = app.ui_tx.clone();
                            app.runtime.spawn(async move {
                                match crate::services::web_fetch::fetch_web_page(&url).await {
                                    Ok((title, content)) => {
                                        let _ = tx.send(UiEvent::WebPageFetched {
                                            title,
                                            url,
                                            content,
                                        });
                                    }
                                    Err(e) => {
                                        let _ = tx.send(UiEvent::Error(format!(
                                            "Failed to fetch {}: {}",
                                            url, e
                                        )));
                                    }
                                }
                            });
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("×")
                                            .size(theme.text_sm)
                                            .color(theme.text_dim),
                                    )
                                    .fill(egui::Color32::TRANSPARENT)
                                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                                )
                                .clicked()
                            {
                                app.ui_store.web_tabs.remove(idx);
                            }
                        });
                    });
                }
            });
    }

    ui.add_space(theme.space_8);

    // ── Add new tab input row ──
    ui.horizontal(|ui| {
        let response = ui.add(
            egui::TextEdit::singleline(&mut app.ui_store.editing_title)
                .hint_text("Paste URL…")
                .desired_width(ui.available_width() - 48.0)
                .margin(egui::Margin::symmetric(6, 4)),
        );

        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new("Add")
                        .size(theme.text_xs)
                        .color(theme.accent),
                )
                .fill(egui::Color32::TRANSPARENT)
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
            )
            .clicked()
            || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
        {
            let url = app.ui_store.editing_title.trim().to_string();
            if !url.is_empty() {
                let title = crate::ui::render::truncate(&url, 32);
                app.ui_store.web_tabs.push(WebTab { title, url });
                app.ui_store.editing_title.clear();
            }
        }
    });
}
