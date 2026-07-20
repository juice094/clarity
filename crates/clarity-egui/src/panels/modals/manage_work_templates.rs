//! Modal for managing custom work templates.

use crate::App;
use clarity_ui::design_system::{Space, TextStyle, gap, text};
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::icon_button::icon_button_toolbar;
use clarity_ui::widgets::modal::Modal;
use clarity_ui::widgets::text_input::TextInput;

/// Render the work templates management modal.
pub fn render_manage_work_templates_modal(app: &mut App, ctx: &egui::Context) {
    if app.current_modal() != Some(&clarity_core::ui::ModalType::ManageWorkTemplates) {
        return;
    }

    let theme = app.context.ui_store.theme.clone();

    // Clone templates so the render closure doesn't conflict with mutable app borrows.
    let mut templates_edit = app.settings_store().settings_edit.work_templates.clone();
    let mut needs_save = false;

    Modal::new("manage_work_templates")
        .width(420.0)
        .max_height(600.0)
        .show(ctx, |ui| {
            text(ui, app.t("Work Templates"), TextStyle::Title);
            gap(ui, Space::S2);

            // ponytail: ScrollArea is not yet wrapped in clarity-ui. Once a
            // scrollable container component exists, replace this raw call.
            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    let mut remove_idx: Option<usize> = None;
                    for (i, template) in templates_edit.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            text(ui, app.t("Name"), TextStyle::CaptionStrong);
                            ui.add(
                                TextInput::singleline(&mut template.name)
                                    .hint_text("My Workflow")
                                    .width(120.0),
                            );
                            text(ui, app.t("Prompt"), TextStyle::CaptionStrong);
                            ui.add(
                                TextInput::multiline(&mut template.prompt)
                                    .hint_text("Write a function that...")
                                    .width(200.0)
                                    .min_height(48.0),
                            );
                            if icon_button_toolbar(ui, crate::theme::ICON_X, theme.text_sm, &theme)
                                .clicked()
                            {
                                remove_idx = Some(i);
                            }
                        });
                        // TextInput mutations are in-place; write-back below
                        // always syncs them to settings_edit in memory.
                    }
                    if let Some(idx) = remove_idx {
                        templates_edit.remove(idx);
                    }
                });

            gap(ui, Space::S2);

            ui.horizontal(|ui| {
                let add_label = format!("{} {}", crate::theme::ICON_PLUS, app.t("Add"));
                if ui.add(Button::new(&add_label).ghost()).clicked() {
                    templates_edit.push(crate::settings::WorkTemplate {
                        name: String::new(),
                        prompt: String::new(),
                    });
                    needs_save = true;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(Button::new(app.t("Close")).ghost().width(80.0))
                        .clicked()
                    {
                        needs_save = true;
                        app.close_modal();
                    }
                });
            });
        });

    // Always sync edits back to in-memory settings (TextInput mutations are
    // in-place and don't fire change events). Only persist to disk on
    // explicit actions (add, delete, close) to avoid per-frame disk writes.
    app.settings_store_mut().settings_edit.work_templates = templates_edit;
    if needs_save {
        app.auto_save_settings();
    }
}
