//! Right rail — Subagents / background tasks card.

use crate::App;
use crate::services::gateway_task_client::GatewayTaskClient;
use clarity_core::background::TaskStatus;

/// Render sub-agent progress and background tasks into the right rail.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    ui.label(
        egui::RichText::new("Subagents & Tasks")
            .size(theme.text_base)
            .strong()
            .color(theme.text),
    );
    ui.add_space(theme.space_12);

    // New task button
    ui.horizontal(|ui| {
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(
                    egui::RichText::new("+ New Task")
                        .size(theme.text_xs)
                        .color(theme.text),
                )
                .clicked()
            {
                app.task_store.task_create_modal_open = true;
            }
        });
    });
    ui.add_space(theme.space_8);

    // Background tasks
    let action = crate::ui::task_panel::render_task_panel(ui, &app.task_store.tasks, &theme);
    match action {
        crate::ui::task_panel::TaskPanelAction::Cancel(task_id) => {
            cancel_task(app, &task_id);
        }
        crate::ui::task_panel::TaskPanelAction::ViewOutput(task_id) => {
            view_task_output(app, &task_id);
        }
        _ => {}
    }

    // Parallel batches
    if !app.subagent_store.parallel_batches.is_empty() {
        ui.add_space(theme.space_16);
        ui.label(
            egui::RichText::new("Parallel Batches")
                .size(theme.text_sm)
                .strong()
                .color(theme.text),
        );
        ui.add_space(theme.space_4);
        egui::Frame::new()
            .fill(theme.surface)
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                for batch in &app.subagent_store.parallel_batches {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&batch.batch_id)
                                .size(theme.text_xs)
                                .color(theme.text),
                        );
                        ui.label(
                            egui::RichText::new(format!("{}/{}", batch.completed, batch.total))
                                .size(theme.text_xs)
                                .color(theme.text_dim),
                        );
                    });
                }
            });
    }
}

fn cancel_task(app: &mut App, task_id: &str) {
    let gateway_client = GatewayTaskClient::new();
    let local_store = app.state.task_store.clone();
    let tx = app.ui_tx.clone();
    let task_id = task_id.to_string();
    app.runtime.spawn(async move {
        if let Err(e) = gateway_client.cancel_task(&task_id).await {
            tracing::debug!("Gateway cancel failed ({}), falling back to local store", e);
            if let Err(e) = local_store
                .update_status(&task_id, TaskStatus::Cancelled)
                .await
            {
                tracing::warn!("Failed to cancel task {} locally: {}", task_id, e);
                return;
            }
        }
        match local_store.list_all().await {
            Ok(tasks) => {
                let _ = tx.send(crate::ui::types::UiEvent::TaskList(tasks));
            }
            Err(e) => {
                tracing::warn!("Failed to list tasks after cancel: {}", e);
            }
        }
    });
}

fn view_task_output(app: &mut App, task_id: &str) {
    app.task_store.viewing_task_id = Some(task_id.to_string());
    app.task_store.task_view_modal_open = true;
    let gateway_client = GatewayTaskClient::new();
    let local_store = app.state.task_store.clone();
    let tx = app.ui_tx.clone();
    let task_id = task_id.to_string();
    app.runtime.spawn(async move {
        match gateway_client.get_task(&task_id).await {
            Ok((_, Some(result))) => {
                let _ = tx.send(crate::ui::types::UiEvent::TaskResultLoaded { task_id, result });
                return;
            }
            Ok((_, None)) => {
                let _ = tx.send(crate::ui::types::UiEvent::TaskResultLoaded {
                    task_id,
                    result: clarity_core::background::TaskResult {
                        status: TaskStatus::Pending,
                        output: "No result available yet.".to_string(),
                        elapsed_ms: 0,
                        steps: 0,
                    },
                });
                return;
            }
            Err(e) => {
                tracing::debug!(
                    "Gateway get_task failed ({}), falling back to local store",
                    e
                );
            }
        }

        match local_store.get_result_opt(&task_id).await {
            Ok(Some(result)) => {
                let _ = tx.send(crate::ui::types::UiEvent::TaskResultLoaded { task_id, result });
            }
            Ok(None) => {
                let _ = tx.send(crate::ui::types::UiEvent::TaskResultLoaded {
                    task_id,
                    result: clarity_core::background::TaskResult {
                        status: TaskStatus::Pending,
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
