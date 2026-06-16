//! Right rail — Subagents / background tasks card.

use crate::App;
use crate::design_system::{self, ButtonStyle, Space, Surface, Text};
use crate::services::gateway_task_client::GatewayTaskClient;
use clarity_core::background::TaskStatus;

/// Render sub-agent progress and background tasks into the right rail.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    design_system::text(ui, "Subagents & Tasks", Text::BodyStrong);
    design_system::gap(ui, Space::S2);

    // New task button
    let mut new_task_clicked = false;
    ui.horizontal(|ui| {
        design_system::push_right(ui);
        new_task_clicked = design_system::btn(ui, "+ New Task", ButtonStyle::Secondary).clicked();
    });
    if new_task_clicked {
        app.view_state
            .open_modal(clarity_core::ui::ModalType::TaskCreate);
    }
    design_system::gap(ui, Space::S1);

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
        design_system::gap(ui, Space::S3);
        design_system::text(ui, "Parallel Batches", Text::BodyStrong);
        design_system::gap(ui, Space::S0);
        design_system::surface(ui, Surface::Well, |ui| {
            ui.set_min_width(ui.available_width());
            for batch in &app.subagent_store.parallel_batches {
                design_system::row(ui, |ui| {
                    design_system::text(ui, &batch.batch_id, Text::Small);
                    design_system::push_right(ui);
                    design_system::text(
                        ui,
                        format!("{}/{}", batch.completed, batch.total),
                        Text::BodyMuted,
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
    app.view_state
        .open_modal(clarity_core::ui::ModalType::TaskView);
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
