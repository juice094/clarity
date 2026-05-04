//! Turn-level rendering — CLI and Glass variants for AgentTurn.

use crate::components::agent_turn::{AgentTurn, ToolCallRow};
use crate::theme::Theme;
use crate::ui::types::ToolCallStatus;

// ============================================================================
// Public dispatch
// ============================================================================

/// Render an AgentTurn in **CLI style**: zero borders, single avatar, indented tools.
pub fn render_agent_turn(ui: &mut egui::Ui, turn: &mut AgentTurn, theme: &Theme) -> f32 {
    let start_y = ui.cursor().min.y;

    // ── Header: avatar + "Agent" + meta ──
    ui.horizontal(|ui| {
        crate::components::chat::avatar::avatar(ui, "A", theme);
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Agent")
                .size(theme.text_xs)
                .color(theme.text_dim),
        );
        if turn.header.tool_count > 0 {
            ui.label(
                egui::RichText::new(format!("· {} tools", turn.header.tool_count))
                    .size(theme.text_xs)
                    .color(theme.text_dim),
            );
        }
    });
    ui.add_space(theme.space_8);

    // ── Thinking (collapsed by default) ──
    if let Some(ref thinking) = turn.thinking {
        let label = format!("思考过程 ▼ · {} tokens", thinking.token_hint);
        egui::CollapsingHeader::new(
            egui::RichText::new(label)
                .size(theme.text_sm)
                .color(theme.text_muted),
        )
        .id_salt("agent_turn_thinking_cli")
        .default_open(false)
        .show(ui, |ui| {
            for step in &thinking.steps {
                ui.label(
                    egui::RichText::new(step)
                        .size(theme.text_sm)
                        .color(theme.chat_text),
                );
            }
        });
        ui.add_space(theme.space_4);
    }

    // ── Tool calls (folded into single summary line) ──
    if !turn.tool_calls.is_empty() {
        let summary = format!("{} tools", turn.tool_calls.len());
        egui::CollapsingHeader::new(
            egui::RichText::new(summary)
                .size(theme.text_sm)
                .color(theme.text_dim),
        )
        .id_salt("agent_turn_tools_cli")
        .default_open(false)
        .show(ui, |ui| {
            for tc in &turn.tool_calls {
                render_tool_call_row_cli(ui, tc, theme);
            }
        });
        ui.add_space(theme.space_4);
    }

    // ── Final response (plain, no card wrapper) ──
    if let Some(ref msg) = turn.final_response {
        ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
            ui.set_max_width(ui.available_width());
            crate::ui::markdown::render_blocks(ui, &msg.parsed, theme, theme.chat_text);
        });
        ui.add_space(theme.space_12);
    }

    ui.add_space(theme.space_16);
    let height = ui.cursor().min.y - start_y;
    turn.cached_height = Some(height);
    height
}

/// Render an AgentTurn in **Glass style**: existing card aesthetic preserved.
pub fn render_agent_turn_glass(ui: &mut egui::Ui, turn: &mut AgentTurn, theme: &Theme) -> f32 {
    let start_y = ui.cursor().min.y;
    let max_width = (ui.available_width() - 32.0).max(120.0);

    egui::Frame::new()
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
        .stroke(egui::Stroke::NONE)
        .shadow(egui::Shadow::NONE)
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_max_width(max_width);

            // Header inside card
            ui.horizontal(|ui| {
                crate::components::chat::avatar::avatar(ui, "A", theme);
                ui.add_space(8.0);
                ui.label(
                    egui::RichText::new("Agent")
                        .size(theme.text_xs)
                        .color(theme.text_dim),
                );
                if turn.header.tool_count > 0 {
                    ui.label(
                        egui::RichText::new(format!("· {} tools", turn.header.tool_count))
                            .size(theme.text_xs)
                            .color(theme.text_dim),
                    );
                }
            });
            ui.add_space(theme.space_8);

            // Thinking
            if let Some(ref thinking) = turn.thinking {
                let label = format!(
                    "Thinking ({}) · {} tokens",
                    thinking.steps.len(),
                    thinking.token_hint
                );
                egui::CollapsingHeader::new(
                    egui::RichText::new(label)
                        .size(theme.text_sm)
                        .strong()
                        .color(theme.text_muted),
                )
                .id_salt("agent_turn_thinking_glass")
                .default_open(false)
                .show(ui, |ui| {
                    for step in &thinking.steps {
                        ui.label(
                            egui::RichText::new(step)
                                .size(theme.text_sm)
                                .color(theme.chat_text),
                        );
                    }
                });
                ui.add_space(theme.space_4);
            }

            // Tool calls (folded into single summary line)
            if !turn.tool_calls.is_empty() {
                let summary = format!("{} tools", turn.tool_calls.len());
                egui::CollapsingHeader::new(
                    egui::RichText::new(summary)
                        .size(theme.text_sm)
                        .color(theme.text_dim),
                )
                .id_salt("agent_turn_tools_glass")
                .default_open(false)
                .show(ui, |ui| {
                    for tc in &turn.tool_calls {
                        render_tool_call_row_glass(ui, tc, theme);
                    }
                });
                ui.add_space(theme.space_4);
            }

            // Final response
            if let Some(ref msg) = turn.final_response {
                crate::ui::markdown::render_blocks(ui, &msg.parsed, theme, theme.chat_text);
                ui.add_space(theme.space_8);
            }
        });

    ui.add_space(theme.space_16);
    let height = ui.cursor().min.y - start_y;
    turn.cached_height = Some(height);
    height
}

// ============================================================================
// Internal helpers
// ============================================================================

fn render_tool_call_row_cli(ui: &mut egui::Ui, tc: &ToolCallRow, theme: &Theme) {
    let stripe_color = status_color(tc.status, theme);
    let icon = match tc.status {
        ToolCallStatus::Running => crate::theme::ICON_HOURGLASS,
        ToolCallStatus::Success => crate::theme::ICON_CHECK,
        ToolCallStatus::Warning => crate::theme::ICON_WARNING,
        ToolCallStatus::Error => crate::theme::ICON_X,
    };

    ui.horizontal(|ui| {
        // Left indent + status stripe
        let stripe_rect = ui
            .allocate_exact_size(egui::vec2(24.0, 28.0), egui::Sense::hover())
            .0;
        if ui.is_rect_visible(stripe_rect) {
            let line_rect = egui::Rect::from_min_max(
                stripe_rect.left_top() + egui::vec2(10.0, 4.0),
                stripe_rect.left_bottom() + egui::vec2(12.0, -4.0),
            );
            ui.painter()
                .rect_filled(line_rect, egui::CornerRadius::same(1), stripe_color);
        }

        ui.label(egui::RichText::new(icon).font(theme.font_icon(theme.text_sm)));
        ui.label(
            egui::RichText::new(&tc.name)
                .size(theme.text_sm)
                .strong()
                .color(theme.text_muted),
        );
        ui.label(
            egui::RichText::new(&tc.result_preview)
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
    });
    ui.add_space(theme.space_4);
}

fn render_tool_call_row_glass(ui: &mut egui::Ui, tc: &ToolCallRow, theme: &Theme) {
    let stripe_color = status_color(tc.status, theme);
    let icon = match tc.status {
        ToolCallStatus::Running => crate::theme::ICON_HOURGLASS,
        ToolCallStatus::Success => crate::theme::ICON_CHECK,
        ToolCallStatus::Warning => crate::theme::ICON_WARNING,
        ToolCallStatus::Error => crate::theme::ICON_X,
    };

    ui.horizontal(|ui| {
        // Left indent + status stripe (24px to match CLI mode)
        let stripe_rect = ui
            .allocate_exact_size(egui::vec2(24.0, 28.0), egui::Sense::hover())
            .0;
        if ui.is_rect_visible(stripe_rect) {
            let line_rect = egui::Rect::from_min_max(
                stripe_rect.left_top() + egui::vec2(10.0, 4.0),
                stripe_rect.left_bottom() + egui::vec2(12.0, -4.0),
            );
            ui.painter()
                .rect_filled(line_rect, egui::CornerRadius::same(1), stripe_color);
        }

        ui.label(egui::RichText::new(icon).font(theme.font_icon(theme.text_sm)));
        ui.label(
            egui::RichText::new(&tc.name)
                .size(theme.text_sm)
                .strong()
                .color(theme.text_muted),
        );
        ui.label(
            egui::RichText::new(&tc.result_preview)
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
    });
    ui.add_space(theme.space_4);
}

fn status_color(status: ToolCallStatus, theme: &Theme) -> egui::Color32 {
    match status {
        ToolCallStatus::Running => theme.status_busy,
        ToolCallStatus::Success => theme.ok,
        ToolCallStatus::Warning => theme.warn,
        ToolCallStatus::Error => theme.danger,
    }
}
