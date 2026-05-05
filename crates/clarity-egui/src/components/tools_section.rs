use crate::App;

/// Render a collapsible Tools/Tasks section for the left sidebar.
/// Visual style mirrors the old toolbar.rs panel: large header, status dots, accent button.
pub fn render_tools_section(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme;
    let expanded = app.ui_store.tools_expanded;

    // ── Header bar (always visible) ──
    let active_tasks = app
        .task_store
        .tasks
        .iter()
        .filter(|t| !t.status.is_terminal())
        .count();
    let status_color = if active_tasks > 0 {
        theme.status_busy
    } else {
        theme.status_online
    };

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Tools")
                .size(theme.text_sm)
                .strong()
                .color(theme.text),
        );
        ui.label(
            egui::RichText::new("●")
                .size(theme.text_xs)
                .color(status_color),
        );
        ui.label(
            egui::RichText::new(format!("{}", active_tasks))
                .size(theme.text_sm)
                .color(theme.text_muted),
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
                app.ui_store.tools_expanded = !expanded;
            }
        });
    });

    if !expanded {
        return;
    }

    ui.add_space(theme.space_8);

    // ── Status overview ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("●")
                .size(theme.text_xs)
                .color(status_color),
        );
        ui.label(
            egui::RichText::new(format!("Active: {}", active_tasks))
                .size(theme.text_xs)
                .color(theme.text_muted),
        );
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("●")
                .size(theme.text_xs)
                .color(theme.status_online),
        );
        ui.label(
            egui::RichText::new(app.session_store.active_category.to_string())
                .size(theme.text_xs)
                .color(theme.text_muted),
        );
    });
    ui.add_space(theme.space_8);

    // ── Task list ──
    let action = crate::ui::task_panel::render_task_panel(ui, &app.task_store.tasks, theme);
    if let crate::ui::task_panel::TaskPanelAction::Cancel(task_id) = action {
        let store = app.state.task_store.clone();
        app.runtime.spawn(async move {
            if let Err(e) = store
                .update_status(&task_id, clarity_core::background::TaskStatus::Cancelled)
                .await
            {
                tracing::warn!("Failed to cancel task {}: {}", task_id, e);
            }
        });
    }

    // ── Create task button ──
    ui.add_space(theme.space_8);
    if ui
        .add(
            egui::Button::new(
                egui::RichText::new("+ Task")
                    .size(theme.text_sm)
                    .color(theme.accent),
            )
            .fill(egui::Color32::TRANSPARENT)
            .stroke(egui::Stroke::new(1.0, theme.accent))
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
            .min_size(egui::vec2(0.0, 28.0)),
        )
        .clicked()
    {
        app.task_store.task_create_modal_open = true;
    }

    // ── Cron jobs button ──
    {
        ui.add_space(theme.space_4);
        let cron_count = app.cron_store.tasks.len();
        let cron_btn_text = if cron_count > 0 {
            format!("+ Cron ({cron_count})")
        } else {
            "+ Cron".to_string()
        };
        if ui
            .add(
                egui::Button::new(
                    egui::RichText::new(cron_btn_text)
                        .size(theme.text_sm)
                        .color(theme.accent),
                )
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::new(1.0, theme.accent))
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                .min_size(egui::vec2(0.0, 28.0)),
            )
            .clicked()
        {
            app.cron_store.cron_panel_open = !app.cron_store.cron_panel_open;
        }
    }

}
