//! Modal for managing web link bookmarks (Chat or Work context).

use crate::App;

/// Render the web links management modal.
pub fn render_manage_web_links_modal(app: &mut App, ctx: &egui::Context, is_chat: bool) {
    let expected_modal = if is_chat {
        clarity_core::ui::ModalType::ManageWebLinksChat
    } else {
        clarity_core::ui::ModalType::ManageWebLinksWork
    };
    if app.view_state.modal != Some(expected_modal) {
        return;
    }

    let title = app.t("Web");
    let theme = app.ui_store.theme.clone();

    // Clone links so the render closure doesn't conflict with mutable app borrows.
    let mut links_edit = if is_chat {
        app.settings_store.settings_edit.web_links_chat.clone()
    } else {
        app.settings_store.settings_edit.web_links_work.clone()
    };
    let mut needs_save = false;

    egui::Window::new(title)
        .collapsible(false)
        .resizable(true)
        .movable(true)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(theme.surface)
                .corner_radius(egui::CornerRadius::same(theme.radius_md as u8)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(360.0);

            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    let mut remove_idx: Option<usize> = None;
                    for (i, link) in links_edit.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(app.t("Name"));
                            ui.add(
                                egui::TextEdit::singleline(&mut link.name)
                                    .hint_text("GitHub")
                                    .desired_width(120.0),
                            );
                            ui.label(app.t("URL"));
                            ui.add(
                                egui::TextEdit::singleline(&mut link.url)
                                    .hint_text("https://github.com")
                                    .desired_width(160.0),
                            );
                            if ui
                                .add_sized(
                                    egui::vec2(20.0, 20.0),
                                    egui::Button::new(
                                        egui::RichText::new(crate::theme::ICON_X)
                                            .size(theme.text_sm),
                                    ),
                                )
                                .clicked()
                            {
                                remove_idx = Some(i);
                            }
                        });
                        // TextEdit mutations are in-place; write-back below
                        // always syncs them to settings_edit in memory.
                    }
                    if let Some(idx) = remove_idx {
                        links_edit.remove(idx);
                    }
                });

            ui.add_space(theme.space_8);

            ui.horizontal(|ui| {
                if ui
                    .add(
                        egui::Button::new(
                            egui::RichText::new(format!(
                                "{} {}",
                                crate::theme::ICON_PLUS,
                                app.t("Add")
                            ))
                            .size(theme.text_sm),
                        )
                        .fill(theme.bg_hover),
                    )
                    .clicked()
                {
                    links_edit.push(crate::settings::WebLink {
                        name: String::new(),
                        url: String::new(),
                    });
                    needs_save = true;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(app.t("Close")).clicked() {
                        needs_save = true;
                        app.view_state.close_modal();
                    }
                });
            });
        });

    // Always sync edits back to in-memory settings (TextEdit mutations are
    // in-place and don't fire change events). Only persist to disk on
    // explicit actions (add, delete, close) to avoid per-frame disk writes.
    if is_chat {
        app.settings_store.settings_edit.web_links_chat = links_edit;
    } else {
        app.settings_store.settings_edit.web_links_work = links_edit;
    }
    if needs_save {
        app.auto_save_settings();
    }
}
