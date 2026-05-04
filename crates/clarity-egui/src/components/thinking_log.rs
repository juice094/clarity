use crate::App;

/// Render a collapsible Thinking Log panel for the left sidebar.
/// Displays live tool call activity from `chat_store.tool_calls`.
pub fn render_thinking_log(app: &mut App, ui: &mut egui::Ui) {
    let theme = &app.ui_store.theme;
    let expanded = app.ui_store.thinking_log_expanded;

    // ── Header bar ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Thinking Log")
                .size(theme.text_sm)
                .strong()
                .color(theme.text),
        );
        let active = app
            .chat_store
            .tool_calls
            .iter()
            .filter(|t| matches!(t.status, crate::ui::types::ToolCallStatus::Running))
            .count();
        if active > 0 {
            ui.label(
                egui::RichText::new(format!("{}", active))
                    .size(theme.text_sm)
                    .color(theme.status_busy),
            );
        }
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
                app.ui_store.thinking_log_expanded = !expanded;
            }
        });
    });

    if !expanded {
        return;
    }

    ui.add_space(theme.space_8);

    if app.chat_store.tool_calls.is_empty() {
        ui.label(
            egui::RichText::new("No tool calls yet.")
                .size(theme.text_xs)
                .color(theme.text_dim),
        );
        return;
    }

    let total = app.chat_store.tool_calls.len();
    let show_all = app.ui_store.thinking_log_show_all;
    let visible_count = if show_all { total.min(20) } else { 3.min(total) };
    let hidden_count = total.saturating_sub(visible_count);

    egui::Frame::new()
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            for tc in app.chat_store.tool_calls.iter().rev().take(visible_count) {
                let inferred = tc.inferred_status();

                // ── Row 1: status icon + tool name ──
                ui.horizontal(|ui| {
                    if matches!(inferred, crate::ui::types::ToolCallStatus::Running) {
                        ui.add(egui::Spinner::new().size(theme.text_sm));
                    } else {
                        let (icon, color) = match inferred {
                            crate::ui::types::ToolCallStatus::Success => (crate::theme::ICON_CHECK, theme.ok),
                            crate::ui::types::ToolCallStatus::Error => (crate::theme::ICON_X, theme.danger),
                            crate::ui::types::ToolCallStatus::Warning => (crate::theme::ICON_WARNING, theme.warn),
                            _ => ("", theme.text),
                        };
                        ui.label(
                            egui::RichText::new(icon).font(theme.font_icon(theme.text_sm)).color(color),
                        );
                    }
                    ui.label(
                        egui::RichText::new(&tc.name)
                            .size(theme.text_sm)
                            .color(theme.text)
                            .strong(),
                    );
                });

                // ── Row 2: result preview + emotion dot ──
                if let Some(ref result) = tc.result {
                    let preview = crate::ui::render::truncate(result, 60);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(preview)
                                .size(theme.text_xs)
                                .color(theme.text_dim),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let dot_color = match inferred {
                                crate::ui::types::ToolCallStatus::Success => theme.status_online,
                                crate::ui::types::ToolCallStatus::Error => theme.danger,
                                crate::ui::types::ToolCallStatus::Warning => theme.warn,
                                _ => theme.status_busy,
                            };
                            ui.label(
                                egui::RichText::new("●")
                                    .size(8.0)
                                    .color(dot_color),
                            );
                        });
                    });

                    // ── Expandable error details ──
                    if matches!(inferred, crate::ui::types::ToolCallStatus::Error) {
                        egui::CollapsingHeader::new(
                            egui::RichText::new("Error details")
                                .size(theme.text_xs)
                                .color(theme.danger),
                        )
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(result)
                                    .size(theme.text_xs)
                                    .color(theme.danger),
                            );
                        });
                    }
                }
                ui.add_space(2.0);
            }

            // ── Show-more toggle ──
            if hidden_count > 0 && !show_all {
                if ui
                    .button(
                        egui::RichText::new(format!("还有 {} 个 ▼", hidden_count))
                            .size(theme.text_xs)
                            .color(theme.text_dim),
                    )
                    .clicked()
                {
                    app.ui_store.thinking_log_show_all = true;
                }
            } else if show_all && total > 3 {
                if ui
                    .button(
                        egui::RichText::new("收起 ▲")
                            .size(theme.text_xs)
                            .color(theme.text_dim),
                    )
                    .clicked()
                {
                    app.ui_store.thinking_log_show_all = false;
                }
            }
        });
}
