//! Modal for managing custom work templates.

use crate::App;

/// Render the work templates management modal.
pub fn render_manage_work_templates_modal(app: &mut App, ctx: &egui::Context) {
    if app.view_state.modal != Some(clarity_core::ui::ModalType::ManageWorkTemplates) {
        return;
    }

    let theme = app.ui_store.theme.clone();

    // Clone templates so the render closure doesn't conflict with mutable app borrows.
    let mut templates_edit = app.settings_store.settings_edit.work_templates.clone();
    let mut needs_save = false;

    egui::Window::new(app.t("Work Templates"))
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
            ui.set_min_width(400.0);

            egui::ScrollArea::vertical()
                .max_height(400.0)
                .show(ui, |ui| {
                    let mut remove_idx: Option<usize> = None;
                    for (i, template) in templates_edit.iter_mut().enumerate() {
                        ui.horizontal(|ui| {
                            ui.label(app.t("Name"));
                            ui.add(
                                egui::TextEdit::singleline(&mut template.name)
                                    .hint_text("My Workflow")
                                    .desired_width(120.0),
                            );
                            ui.label(app.t("Prompt"));
                            ui.add(
                                egui::TextEdit::multiline(&mut template.prompt)
                                    .hint_text("Write a function that...")
                                    .desired_width(200.0)
                                    .desired_rows(2),
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
                        templates_edit.remove(idx);
                    }
                });

            crate::design_system::gap(ui, crate::design_system::Space::S1);

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
                    templates_edit.push(crate::settings::WorkTemplate {
                        name: String::new(),
                        prompt: String::new(),
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
    app.settings_store.settings_edit.work_templates = templates_edit;
    if needs_save {
        app.auto_save_settings();
    }
}

// ── Panel trait implementation ──

pub struct ManageWorkTemplatesModal;

impl crate::design_system::Panel for ManageWorkTemplatesModal {
    fn title(&self, _app: &crate::App) -> &str {
        "ManageWorkTemplates"
    }
    fn render(&mut self, app: &mut crate::App, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        render_manage_work_templates_modal(app, &ctx);
    }
}
