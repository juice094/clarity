//! Task panel — background task list for the right IDE rail.
//!
//! Surfaces `TaskStore` data refreshed by `App::refresh_tasks` (polled while
//! this tab is visible). Viewing a result reuses the `TaskView` modal;
//! creation reuses the `TaskCreate` modal.

use crate::App;
use crate::design_system::{self, BadgeVariant, Space, TextStyle};
use crate::ui::types::UiEvent;
use clarity_core::background::{TaskInfo, TaskStatus};
use clarity_ui::widgets::button::Button;

/// Render the task panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.context.ui_store.theme.clone();

    // ── Toolbar ──
    ui.horizontal(|ui| {
        design_system::text(ui, app.t("Background Tasks"), TextStyle::CaptionStrong);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(Button::new(app.t("New Task")).primary().small())
                .clicked()
            {
                app.open_modal(clarity_core::ui::ModalType::TaskCreate);
            }
            design_system::gap(ui, Space::S0);
            if ui
                .add(Button::new(app.t("Refresh")).ghost().small())
                .clicked()
            {
                app.refresh_tasks();
            }
        });
    });
    design_system::gap(ui, Space::S1);

    let tasks = app.task_store().tasks.clone();
    if tasks.is_empty() {
        render_empty_state(app, ui, &theme);
        return;
    }

    let mut action = TaskAction::None;
    egui::ScrollArea::vertical()
        .id_salt("task_panel")
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for task in &tasks {
                let card_action = render_task_card(&*app, ui, task, &theme);
                if !matches!(card_action, TaskAction::None) {
                    action = card_action;
                }
                design_system::gap(ui, Space::S1);
            }
        });

    match action {
        TaskAction::ViewResult(task_id) => view_task_result(app, task_id),
        TaskAction::Cancel(task_id) => cancel_task(app, task_id),
        TaskAction::None => {}
    }
}

/// Actions emitted by a task card.
enum TaskAction {
    None,
    ViewResult(String),
    Cancel(String),
}

fn render_empty_state(app: &App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    ui.vertical_centered(|ui| {
        ui.add_space(theme.space_24);
        clarity_ui::design_system::icon_with_color(
            ui,
            crate::theme::ICON_LIST,
            theme.text_2xl,
            theme.text_dim,
        );
        design_system::gap(ui, Space::S1);
        design_system::text(ui, app.t("No tasks yet"), TextStyle::Subheading);
        design_system::gap(ui, Space::S1);
        design_system::text(
            ui,
            app.t("Create a task to run it in the background"),
            TextStyle::Small,
        );
    });
}

fn render_task_card(
    app: &App,
    ui: &mut egui::Ui,
    task: &TaskInfo,
    theme: &crate::theme::Theme,
) -> TaskAction {
    let mut action = TaskAction::None;
    let (icon, icon_color) = status_icon_and_color(task.status, theme);
    let is_terminal = task.status.is_terminal();

    design_system::card(ui, |ui| {
        ui.set_min_width(ui.available_width());

        // Row 1: status icon + name + status badge
        ui.horizontal(|ui| {
            clarity_ui::design_system::icon_with_color(ui, icon, theme.text_sm, icon_color);
            design_system::gap(ui, Space::S0);
            design_system::text(ui, &task.spec.name, TextStyle::Body);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                design_system::badge(
                    ui,
                    app.t(task_status_key(task.status)),
                    status_badge_for(task.status),
                );
            });
        });

        // Row 2: elapsed time
        design_system::gap(ui, Space::S0);
        design_system::text(
            ui,
            format_elapsed_secs(task_elapsed_secs(task)),
            TextStyle::Small,
        );

        // Row 3: actions
        design_system::gap(ui, Space::S1);
        ui.horizontal(|ui| {
            if is_terminal
                && ui
                    .add(Button::new(app.t("View Result")).ghost().small())
                    .clicked()
            {
                action = TaskAction::ViewResult(task.id.clone());
            }
            if !is_terminal
                && ui
                    .add(Button::new(app.t("Cancel")).danger_ghost().small())
                    .clicked()
            {
                action = TaskAction::Cancel(task.id.clone());
            }
        });
    });

    action
}

/// Load the task result from the local store and open the `TaskView` modal.
///
/// ponytail: results are read from the local `TaskStore`; tasks executed on a
/// remote Gateway keep their result there, so the modal stays in its loading
/// state for those. Upgrade path: bridge a `GET /v1/tasks/:id/result` client
/// call into `viewing_task_result`.
fn view_task_result(app: &mut App, task_id: String) {
    app.task_store_mut().viewing_task_id = Some(task_id.clone());
    app.task_store_mut().viewing_task_result = None;

    let store = app.context.state.task_store.clone();
    let tx = app.context.ui_tx.clone();
    let id = task_id.clone();
    app.context.runtime.spawn(async move {
        match store.get_result_opt(&id).await {
            Ok(Some(result)) => {
                if let Err(e) = tx.send(UiEvent::TaskResultLoaded {
                    task_id: id,
                    result,
                }) {
                    tracing::warn!("Failed to send TaskResultLoaded: {}", e);
                }
            }
            Ok(None) => tracing::warn!("No result stored for task {}", id),
            Err(e) => tracing::warn!("Failed to load result for task {}: {}", id, e),
        }
    });

    app.open_modal(clarity_core::ui::ModalType::TaskView);
}

/// Cancel a pending/running task and refresh the list.
///
/// ponytail: cancellation goes through the local `BackgroundTaskManager`;
/// `GatewayTaskClient` has no cancel endpoint, so Gateway-side tasks are only
/// marked cancelled in the shared store. Upgrade path: add a
/// `POST /v1/tasks/:id/cancel` route and client method.
fn cancel_task(app: &mut App, task_id: String) {
    let bg_manager = std::sync::Arc::clone(&app.context.state.bg_manager);
    let tx = app.context.ui_tx.clone();
    app.context.runtime.spawn(async move {
        if let Err(e) = bg_manager.cancel(&task_id).await {
            tracing::warn!("Failed to cancel task {}: {}", task_id, e);
        }
        match bg_manager.list().await {
            Ok(tasks) => {
                if let Err(e) = tx.send(UiEvent::TaskList(tasks)) {
                    tracing::warn!("Failed to send TaskList after cancel: {}", e);
                }
            }
            Err(e) => tracing::warn!("Failed to list tasks after cancel: {}", e),
        }
    });
}

/// i18n key for a task status label.
fn task_status_key(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Pending => "Pending",
        TaskStatus::Running => "Running",
        TaskStatus::Completed => "Completed",
        TaskStatus::Failed => "Failed",
        TaskStatus::Cancelled => "Cancelled",
    }
}

fn status_badge_for(status: TaskStatus) -> BadgeVariant {
    match status {
        TaskStatus::Completed => BadgeVariant::Ok,
        TaskStatus::Failed => BadgeVariant::Danger,
        TaskStatus::Running | TaskStatus::Pending => BadgeVariant::Accent,
        TaskStatus::Cancelled => BadgeVariant::Warn,
    }
}

fn status_icon_and_color(
    status: TaskStatus,
    theme: &crate::theme::Theme,
) -> (&'static str, egui::Color32) {
    match status {
        TaskStatus::Completed => (crate::theme::ICON_CHECK, theme.status_online),
        TaskStatus::Failed => (crate::theme::ICON_X, theme.danger),
        TaskStatus::Cancelled => (crate::theme::ICON_PROHIBIT, theme.text_dim),
        TaskStatus::Running | TaskStatus::Pending => {
            (crate::theme::ICON_HOURGLASS, theme.status_busy)
        }
    }
}

/// Seconds between task creation and its last update (or now, while the task
/// is still pending/running).
fn task_elapsed_secs(task: &TaskInfo) -> u64 {
    let end = if task.status.is_terminal() {
        task.updated_at
    } else {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
    };
    end.saturating_sub(task.created_at)
}

/// Format a duration as a compact string (`45s`, `3m12s`, `2h05m`).
fn format_elapsed_secs(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{}h{:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

// ── Panel trait implementation ──

/// Task panel renderer.
pub struct TaskPanel;

impl crate::design_system::Panel for TaskPanel {
    fn title(&self, app: &crate::App) -> &str {
        app.t("Task")
    }
    fn render(&mut self, app: &mut crate::App, ui: &mut egui::Ui) {
        render(app, ui);
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_core::background::TaskSpec;

    fn make_task(status: TaskStatus, created_at: u64, updated_at: u64) -> TaskInfo {
        TaskInfo {
            id: "task-1".to_string(),
            spec: TaskSpec::new("demo", "do something"),
            status,
            created_at,
            updated_at,
        }
    }

    #[test]
    fn status_keys_cover_all_variants() {
        assert_eq!(task_status_key(TaskStatus::Pending), "Pending");
        assert_eq!(task_status_key(TaskStatus::Running), "Running");
        assert_eq!(task_status_key(TaskStatus::Completed), "Completed");
        assert_eq!(task_status_key(TaskStatus::Failed), "Failed");
        assert_eq!(task_status_key(TaskStatus::Cancelled), "Cancelled");
    }

    #[test]
    fn format_elapsed_secs_compacts_units() {
        assert_eq!(format_elapsed_secs(0), "0s");
        assert_eq!(format_elapsed_secs(45), "45s");
        assert_eq!(format_elapsed_secs(60), "1m0s");
        assert_eq!(format_elapsed_secs(192), "3m12s");
        assert_eq!(format_elapsed_secs(3600), "1h00m");
        assert_eq!(format_elapsed_secs(7500), "2h05m");
    }

    #[test]
    fn task_elapsed_uses_updated_at_for_terminal_tasks() {
        let task = make_task(TaskStatus::Completed, 1000, 1045);
        assert_eq!(task_elapsed_secs(&task), 45);
    }

    #[test]
    fn task_elapsed_never_underflows() {
        // updated_at before created_at (clock skew) must not panic.
        let task = make_task(TaskStatus::Completed, 2000, 1000);
        assert_eq!(task_elapsed_secs(&task), 0);
    }

    #[test]
    fn task_elapsed_running_uses_now() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let task = make_task(TaskStatus::Running, now.saturating_sub(5), 0);
        let elapsed = task_elapsed_secs(&task);
        assert!((5..=6).contains(&elapsed));
    }
}
