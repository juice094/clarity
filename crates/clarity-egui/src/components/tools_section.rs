use crate::App;

/// Render a collapsible Tools/Tasks section for the left sidebar.
/// Visual style mirrors the old toolbar.rs panel: large header, status dots, accent button.
pub fn render_tools_section(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme;
    let expanded = app.ui_store.tools_expanded;

    // ── Header bar (always visible) ──
    let active_tasks = app.task_store.tasks.iter().filter(|t| !t.status.is_terminal()).count();
    let status_color = if active_tasks > 0 {
        theme.status_busy
    } else {
        theme.status_online
    };

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Tools")
                .size(theme.text_lg)
                .strong()
                .color(theme.text),
        );
        ui.label(egui::RichText::new("●").size(theme.text_xs).color(status_color));
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
        ui.label(egui::RichText::new("●").size(theme.text_xs).color(status_color));
        ui.label(
            egui::RichText::new(format!("Active tasks: {}", active_tasks))
                .size(theme.text_sm)
                .color(theme.text_muted),
        );
    });
    ui.add_space(theme.space_4);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("●").size(theme.text_xs).color(theme.status_online));
        ui.label(
            egui::RichText::new(format!("Category: {}", app.session_store.active_category))
                .size(theme.text_sm)
                .color(theme.text_muted),
        );
    });
    ui.add_space(theme.space_8);
    ui.separator();
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
                egui::RichText::new("+ Create Task")
                    .size(theme.text_base)
                    .color(theme.text),
            )
            .fill(theme.accent)
            .min_size(egui::vec2(ui.available_width(), 36.0)),
        )
        .clicked()
    {
        app.task_store.task_create_modal_open = true;
    }

    // ── SubAgent parallel progress ──
    ui.add_space(theme.space_12);
    egui::Frame::group(ui.style())
        .fill(theme.bg)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            crate::panels::subagent_progress::render_subagent_progress(app, ui);
        });
}
