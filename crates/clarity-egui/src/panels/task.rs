use crate::App;
use clarity_core::background::TaskStatus;

#[allow(dead_code)]
pub fn render_task_panel(app: &mut App, ctx: &egui::Context) {
    egui::SidePanel::right("task_panel")
        .default_width(320.0)
        .min_width(240.0)
        .max_width(400.0)
        .resizable(true)
        .frame(
            egui::Frame::side_top_panel(&ctx.style())
                .fill(app.ui_store.theme.bg)
                .stroke(egui::Stroke::new(1.0_f32, app.ui_store.theme.border))
                .inner_margin(egui::Margin::symmetric(12, 16)),
        )
        .show(ctx, |ui| {
            ui.add_space(app.ui_store.theme.space_12);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("Tasks")
                        .size(app.ui_store.theme.text_lg)
                        .strong()
                        .color(app.ui_store.theme.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(
                            egui::RichText::new("+ New")
                                .size(app.ui_store.theme.text_sm)
                                .color(app.ui_store.theme.text),
                        )
                        .clicked()
                    {
                        app.task_store.task_create_modal_open = true;
                    }
                });
            });

            ui.add_space(app.ui_store.theme.space_12);
            let action = crate::ui::task_panel::render_task_panel(
                ui,
                &app.task_store.tasks,
                &app.ui_store.theme,
            );
            match action {
                crate::ui::task_panel::TaskPanelAction::Cancel(task_id) => {
                    let store = app.state.task_store.clone();
                    let tx = app.ui_tx.clone();
                    app.runtime.spawn(async move {
                        if let Err(e) = store.update_status(&task_id, TaskStatus::Cancelled).await {
                            tracing::warn!("Failed to cancel task {}: {}", task_id, e);
                        } else {
                            match store.list_all().await {
                                Ok(tasks) => {
                                    let _ = tx.send(crate::ui::types::UiEvent::TaskList(tasks));
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to list tasks after cancel: {}", e);
                                }
                            }
                        }
                    });
                }
                crate::ui::task_panel::TaskPanelAction::ViewOutput(task_id) => {
                    app.task_store.viewing_task_id = Some(task_id.clone());
                    app.task_store.task_view_modal_open = true;
                    let store = app.state.task_store.clone();
                    let tx = app.ui_tx.clone();
                    app.runtime.spawn(async move {
                        match store.get_result_opt(&task_id).await {
                            Ok(Some(result)) => {
                                let _ = tx.send(crate::ui::types::UiEvent::TaskResultLoaded {
                                    task_id,
                                    result,
                                });
                            }
                            Ok(None) => {
                                let _ = tx.send(crate::ui::types::UiEvent::TaskResultLoaded {
                                    task_id,
                                    result: clarity_core::background::TaskResult {
                                        status: clarity_core::background::TaskStatus::Pending,
                                        output: "No result available yet.".to_string(),
                                        elapsed_ms: 0,
                                        steps: 0,
                                    },
                                });
                            }
                            Err(e) => {
                                tracing::warn!("Failed to get task result: {}", e);
                                let _ = tx.send(crate::ui::types::UiEvent::Error(format!(
                                    "Failed to load task result: {}",
                                    e
                                )));
                            }
                        }
                    });
                }
                _ => {}
            }
        });
}
