//! Plan panel — migrated from chat/plan.rs to workspace bottom fold.
//!
//! Sprint 39: Plan UI now lives in the right-side Workspace panel as a
//! collapsible section, freeing chat message list from plan cards.

use crate::ui::types::{AgentStatus, PlanStepStatus, UiEvent};
use crate::App;

/// Render the Plan foldable section inside the workspace panel.
pub fn render_workspace_plan(app: &mut App, ui: &mut egui::Ui) {
    let has_plan = app.chat_store.pending_plan.is_some() || app.chat_store.plan_tracker.is_some();
    if !has_plan {
        return;
    }

    let theme = app.ui_store.theme.clone();

    ui.add_space(theme.space_12);
    ui.separator();
    ui.add_space(theme.space_8);

    // Collapsing header for the plan section
    let header_text = if let Some(ref plan) = app.chat_store.pending_plan {
        format!("📋 Plan: {} (review)", plan.title)
    } else if let Some(ref tracker) = app.chat_store.plan_tracker {
        let done = tracker.steps.iter().filter(|s| matches!(s.status, PlanStepStatus::Success | PlanStepStatus::Failed | PlanStepStatus::Skipped)).count();
        format!("📋 {} ({}/{})", tracker.title, done, tracker.steps.len())
    } else {
        "📋 Plan".to_string()
    };

    let header = egui::CollapsingHeader::new(
        egui::RichText::new(&header_text)
            .size(theme.text_sm)
            .strong()
            .color(theme.text),
    )
    .id_salt("workspace_plan_fold")
    .open(Some(app.ui_store.workspace_plan_expanded));

    let resp = header.show(ui, |ui| {
        ui.add_space(theme.space_8);
        if app.chat_store.pending_plan.is_some() {
            render_plan_review(app, ui);
        }
        if app.chat_store.plan_tracker.is_some() {
            render_plan_tracker(app, ui);
        }
    });

    // Toggle expansion on header click; track manual collapse
    if resp.header_response.clicked() {
        app.ui_store.workspace_plan_expanded = !app.ui_store.workspace_plan_expanded;
        app.ui_store.workspace_plan_manually_collapsed = !app.ui_store.workspace_plan_expanded;
    }
}

fn render_plan_review(app: &mut App, ui: &mut egui::Ui) {
    let Some(ref plan) = app.chat_store.pending_plan else { return };
    let theme = &app.ui_store.theme;
    let mut execute = false;
    let mut cancel = false;

    egui::Frame::new()
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .stroke(egui::Stroke::new(1.0_f32, theme.accent))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new("Review before execution")
                    .size(theme.text_xs)
                    .color(theme.text_dim),
            );
            ui.add_space(theme.space_8);

            for step in &plan.steps {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{}.", step.id))
                            .size(theme.text_sm)
                            .strong()
                            .color(theme.text),
                    );
                    ui.label(
                        egui::RichText::new(&step.description)
                            .size(theme.text_sm)
                            .color(theme.text),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("→")
                            .size(theme.text_xs)
                            .color(theme.text_dim),
                    );
                    ui.label(
                        egui::RichText::new(format!("{}({})", step.tool_name, step.tool_params))
                            .size(theme.text_xs)
                            .color(theme.text_dim)
                            .monospace(),
                    );
                });
                ui.add_space(2.0);
            }

            ui.add_space(theme.space_8);
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add_sized(
                            egui::vec2(72.0, 28.0),
                            egui::Button::new(
                                egui::RichText::new("Cancel")
                                    .size(theme.text_xs)
                                    .color(theme.text),
                            )
                            .fill(theme.border),
                        )
                        .clicked()
                    {
                        cancel = true;
                    }
                    if ui
                        .add_sized(
                            egui::vec2(72.0, 28.0),
                            egui::Button::new(
                                egui::RichText::new("Execute")
                                    .size(theme.text_xs)
                                    .color(theme.text),
                            )
                            .fill(theme.accent),
                        )
                        .clicked()
                    {
                        execute = true;
                    }
                });
            });
        });

    if execute {
        let plan = app.chat_store.pending_plan.take().unwrap();
        app.chat_store.plan_tracker = Some(crate::ui::types::PlanExecutionTracker {
            title: plan.title.clone(),
            steps: plan
                .steps
                .iter()
                .map(|s| crate::ui::types::PlanStepTracker {
                    id: s.id.clone(),
                    description: s.description.clone(),
                    tool_name: s.tool_name.clone(),
                    status: PlanStepStatus::Pending,
                })
                .collect(),
        });
        let state = app.state.clone();
        let tx = app.ui_tx.clone();
        app.chat_store.is_loading = true;
        app.chat_store.agent_status = AgentStatus::Busy;
        app.runtime.spawn(async move {
            match state.agent.execute_plan(&plan).await {
                Ok(results) => {
                    let mut text = String::new();
                    for r in &results {
                        text.push_str(&format!(
                            "**Step {}**: {}\n```\n{}\n```\n\n",
                            r.step_id,
                            if r.success {
                                crate::theme::ICON_CHECK
                            } else {
                                crate::theme::ICON_X
                            },
                            r.output
                        ));
                    }
                    if let Err(e) = tx.send(UiEvent::Chunk(text)) {
                        tracing::warn!("Failed to send plan results: {}", e);
                    }
                }
                Err(e) => {
                    if let Err(err) = tx.send(UiEvent::Error(format!("Plan execution failed: {}", e))) {
                        tracing::warn!("Failed to send Error: {}", err);
                    }
                }
            }
            if let Err(e) = tx.send(UiEvent::Done) {
                tracing::warn!("Failed to send Done: {}", e);
            }
        });
    } else if cancel {
        app.chat_store.pending_plan = None;
    }
}

fn render_plan_tracker(app: &mut App, ui: &mut egui::Ui) {
    let Some(ref tracker) = app.chat_store.plan_tracker else { return };
    let theme = &app.ui_store.theme;
    let mut dismiss = false;
    let mut skip_step_id: Option<String> = None;
    let mut retry_step_id: Option<String> = None;

    egui::Frame::new()
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .stroke(egui::Stroke::new(1.0_f32, theme.border))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&tracker.title)
                        .size(theme.text_sm)
                        .strong()
                        .color(theme.text),
                );
                if ui
                    .button(
                        egui::RichText::new(crate::theme::ICON_X)
                            .font(theme.font_icon(theme.text_sm))
                            .color(theme.text_dim),
                    )
                    .clicked()
                {
                    dismiss = true;
                }
            });
            ui.add_space(theme.space_8);

            for step in &tracker.steps {
                let (icon, color) = match step.status {
                    PlanStepStatus::Pending => (crate::theme::ICON_HOURGLASS, theme.text_dim),
                    PlanStepStatus::Running => (crate::theme::ICON_PLAY, theme.accent),
                    PlanStepStatus::Success => (crate::theme::ICON_CHECK, theme.ok),
                    PlanStepStatus::Failed => (crate::theme::ICON_X, theme.danger),
                    PlanStepStatus::Skipped => ("⏭", theme.text_dim),
                };
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(icon)
                            .font(theme.font_icon(theme.text_sm)),
                    );
                    ui.label(
                        egui::RichText::new(format!("{}.", step.id))
                            .size(theme.text_sm)
                            .strong()
                            .color(theme.text),
                    );
                    ui.label(
                        egui::RichText::new(&step.description)
                            .size(theme.text_sm)
                            .color(theme.text),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if step.status == PlanStepStatus::Failed {
                            if ui
                                .add_sized(
                                    egui::vec2(48.0, 22.0),
                                    egui::Button::new(
                                        egui::RichText::new("Retry")
                                            .size(theme.text_xs)
                                            .color(theme.text),
                                    )
                                    .fill(theme.surface),
                                )
                                .clicked()
                            {
                                retry_step_id = Some(step.id.clone());
                            }
                        }
                        if step.status == PlanStepStatus::Pending {
                            if ui
                                .add_sized(
                                    egui::vec2(48.0, 22.0),
                                    egui::Button::new(
                                        egui::RichText::new("Skip")
                                            .size(theme.text_xs)
                                            .color(theme.text_dim),
                                    )
                                    .fill(theme.surface),
                                )
                                .clicked()
                            {
                                skip_step_id = Some(step.id.clone());
                            }
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.add_space(theme.space_20);
                    ui.label(
                        egui::RichText::new(format!("→ {}", step.tool_name))
                            .size(theme.text_xs)
                            .color(color)
                            .monospace(),
                    );
                });
                ui.add_space(2.0);
            }
        });

    if dismiss {
        app.chat_store.plan_tracker = None;
    }
    if let Some(step_id) = skip_step_id {
        let _ = app.ui_tx.send(UiEvent::PlanSkip { step_id });
    }
    if let Some(step_id) = retry_step_id {
        let _ = app.ui_tx.send(UiEvent::PlanRetry { step_id });
    }
}
