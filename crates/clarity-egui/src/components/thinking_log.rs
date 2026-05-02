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
                .size(theme.text_lg)
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

    // ── Tool call list in glass card ──
    egui::Frame::group(ui.style())
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
        .stroke(egui::Stroke::new(1.0, theme.border))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            if app.chat_store.tool_calls.is_empty() {
                ui.label(
                    egui::RichText::new("No tool calls yet.")
                        .size(theme.text_sm)
                        .color(theme.text_dim),
                );
            } else {
                for tc in app.chat_store.tool_calls.iter().rev().take(20) {
                    let (icon, color) = match tc.status {
                        crate::ui::types::ToolCallStatus::Running => ("⏳", theme.status_busy),
                        crate::ui::types::ToolCallStatus::Done => ("✓", theme.status_online),
                    };
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(icon).size(theme.text_sm).color(color));
                        ui.label(
                            egui::RichText::new(&tc.name)
                                .size(theme.text_sm)
                                .color(theme.text)
                                .strong(),
                        );
                    });
                    if let Some(ref args) = tc.result {
                        let args_trimmed = if args.len() > 60 {
                            format!("{}…", &args[..60])
                        } else {
                            args.clone()
                        };
                        ui.label(
                            egui::RichText::new(args_trimmed)
                                .size(theme.text_xs)
                                .color(theme.text_dim),
                        );
                    }
                    ui.add_space(theme.space_4);
                }
            }
        });
}
