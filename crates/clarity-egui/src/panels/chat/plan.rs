use crate::ui::types::{AgentStatus, UiEvent};
use crate::App;

pub fn render_plan(app: &mut App, ui: &mut egui::Ui) {
    // Plan review card above input bar
    if let Some(ref plan) = app.chat_store.pending_plan {
        let mut execute = false;
        let mut cancel = false;
        egui::Frame::group(ui.style())
            .fill(app.ui_store.theme.surface)
            .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_md as u8))
            .stroke(egui::Stroke::new(1.0_f32, app.ui_store.theme.accent))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(
                    egui::RichText::new(format!("📋 Plan Review: {}", plan.title))
                        .size(app.ui_store.theme.text_base)
                        .strong()
                        .color(app.ui_store.theme.text),
                );
                ui.add_space(app.ui_store.theme.space_8);
                for step in &plan.steps {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!("{}.", step.id))
                                .size(app.ui_store.theme.text_sm)
                                .strong()
                                .color(app.ui_store.theme.text),
                        );
                        ui.label(
                            egui::RichText::new(&step.description)
                                .size(app.ui_store.theme.text_sm)
                                .color(app.ui_store.theme.text),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("→")
                                .size(app.ui_store.theme.text_xs)
                                .color(app.ui_store.theme.text_dim),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "{}({})",
                                step.tool_name, step.tool_params
                            ))
                            .size(app.ui_store.theme.text_xs)
                            .color(app.ui_store.theme.text_dim)
                            .monospace(),
                        );
                    });
                    ui.add_space(2.0);
                }
                ui.add_space(app.ui_store.theme.space_8);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add_sized(
                                egui::vec2(80.0, 32.0),
                                egui::Button::new(
                                    egui::RichText::new("Cancel")
                                        .size(app.ui_store.theme.text_sm)
                                        .color(app.ui_store.theme.text),
                                )
                                .fill(app.ui_store.theme.border),
                            )
                            .clicked()
                        {
                            cancel = true;
                        }
                        if ui
                            .add_sized(
                                egui::vec2(80.0, 32.0),
                                egui::Button::new(
                                    egui::RichText::new("Execute")
                                        .size(app.ui_store.theme.text_sm)
                                        .color(app.ui_store.theme.text),
                                )
                                .fill(app.ui_store.theme.accent),
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
            // Initialize live execution tracker.
            app.chat_store.plan_tracker = Some(crate::ui::types::PlanExecutionTracker {
                title: plan.title.clone(),
                steps: plan
                    .steps
                    .iter()
                    .map(|s| crate::ui::types::PlanStepTracker {
                        id: s.id.clone(),
                        description: s.description.clone(),
                        tool_name: s.tool_name.clone(),
                        status: crate::ui::types::PlanStepStatus::Pending,
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
                        if let Err(err) =
                            tx.send(UiEvent::Error(format!("Plan execution failed: {}", e)))
                        {
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
        ui.separator();
    }

    // Plan execution tracker panel
    if let Some(ref tracker) = app.chat_store.plan_tracker {
        let mut dismiss = false;
        egui::Frame::group(ui.style())
            .fill(app.ui_store.theme.surface)
            .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_md as u8))
            .stroke(egui::Stroke::new(1.0_f32, app.ui_store.theme.accent))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("📋 {}", tracker.title))
                            .size(app.ui_store.theme.text_base)
                            .strong()
                            .color(app.ui_store.theme.text),
                    );
                    if ui
                        .button(
                            egui::RichText::new(crate::theme::ICON_X)
                                .font(app.ui_store.theme.font_icon(app.ui_store.theme.text_sm))
                                .color(app.ui_store.theme.text_dim),
                        )
                        .clicked()
                    {
                        dismiss = true;
                    }
                });
                ui.add_space(app.ui_store.theme.space_8);
                for step in &tracker.steps {
                    let (icon, color) = match step.status {
                        crate::ui::types::PlanStepStatus::Pending => {
                            (crate::theme::ICON_HOURGLASS, app.ui_store.theme.text_dim)
                        }
                        crate::ui::types::PlanStepStatus::Running => {
                            (crate::theme::ICON_PLAY, app.ui_store.theme.accent)
                        }
                        crate::ui::types::PlanStepStatus::Success => {
                            (crate::theme::ICON_CHECK, app.ui_store.theme.ok)
                        }
                        crate::ui::types::PlanStepStatus::Failed => {
                            (crate::theme::ICON_X, app.ui_store.theme.danger)
                        }
                    };
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(icon)
                                .font(app.ui_store.theme.font_icon(app.ui_store.theme.text_sm)),
                        );
                        ui.label(
                            egui::RichText::new(format!("{}.", step.id))
                                .size(app.ui_store.theme.text_sm)
                                .strong()
                                .color(app.ui_store.theme.text),
                        );
                        ui.label(
                            egui::RichText::new(&step.description)
                                .size(app.ui_store.theme.text_sm)
                                .color(app.ui_store.theme.text),
                        );
                    });
                    ui.horizontal(|ui| {
                        ui.add_space(app.ui_store.theme.space_20);
                        ui.label(
                            egui::RichText::new(format!("→ {}", step.tool_name))
                                .size(app.ui_store.theme.text_xs)
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
        ui.separator();
    }
}
