//! Modal for managing web link bookmarks.

use crate::App;
use clarity_ui::design_system::{Space, TextStyle, gap, text};
use clarity_ui::theme::ICON_X;
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::icon_button::icon_button;
use clarity_ui::widgets::modal::Modal;
use clarity_ui::widgets::text_input::TextInput;

/// Render the web links management modal.
pub fn render_manage_web_links_modal(app: &mut App, ctx: &egui::Context) {
    if app.current_modal() != Some(&clarity_core::ui::ModalType::ManageWebLinks) {
        return;
    }

    let theme = app.context.ui_store.theme.clone();

    // Clone links so the render closure doesn't conflict with mutable app borrows.
    let mut links_edit = app.settings_store().settings_edit.web_links.clone();
    let mut needs_save = false;

    Modal::new("manage_web_links")
        .width(420.0)
        .max_height(600.0)
        .show(ctx, |ui| {
            text(ui, app.t("Web"), TextStyle::Title);
            gap(ui, Space::S2);

            let mut remove_idx: Option<usize> = None;
            for (i, link) in links_edit.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    text(ui, app.t("Name"), TextStyle::CaptionStrong);
                    ui.add(
                        TextInput::singleline(&mut link.name)
                            .hint_text("GitHub")
                            .width(120.0),
                    );
                    text(ui, app.t("URL"), TextStyle::CaptionStrong);
                    ui.add(
                        TextInput::singleline(&mut link.url)
                            .hint_text("https://github.com")
                            .width(160.0),
                    );
                    if icon_button(
                        ui,
                        ICON_X,
                        theme.text_sm,
                        theme.bg_hover,
                        egui::CornerRadius::same(theme.radius_sm as u8),
                        &theme,
                    )
                    .clicked()
                    {
                        remove_idx = Some(i);
                    }
                });
                // TextInput mutations are in-place; write-back below
                // always syncs them to settings_edit in memory.
            }
            if let Some(idx) = remove_idx {
                links_edit.remove(idx);
            }

            gap(ui, Space::S1);

            ui.horizontal(|ui| {
                if ui.add(Button::new(app.t("Add")).ghost()).clicked() {
                    links_edit.push(crate::settings::WebLink {
                        name: String::new(),
                        url: String::new(),
                    });
                    needs_save = true;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add(Button::new(app.t("Close")).width(80.0)).clicked() {
                        needs_save = true;
                        app.close_modal();
                    }
                });
            });
        });

    // Always sync edits back to in-memory settings (TextInput mutations are
    // in-place and don't fire change events). Only persist to disk on
    // explicit actions (add, delete, close) to avoid per-frame disk writes.
    app.settings_store_mut().settings_edit.web_links = links_edit;
    if needs_save {
        app.auto_save_settings();
    }
}
