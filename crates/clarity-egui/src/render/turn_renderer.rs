//! Turn-level rendering — CLI style for AgentTurn.

use crate::components::agent_turn::{AgentTurn, ToolCallRow};
use crate::design_system::{self, Space};
use crate::theme::Theme;
use crate::ui::types::ToolCallStatus;

// ============================================================================
// Public dispatch
// ============================================================================

/// Render an AgentTurn in **CLI style**: zero borders, single avatar, indented tools.
///
/// Uses [`egui::Frame::Prepared`] to detect tool error/success state after
/// rendering content and apply dynamic background tinting — errors get a
/// subtle red left-edge accent, multi-tool turns get an activity indicator.
pub fn render_agent_turn(
    ui: &mut egui::Ui,
    turn: &mut AgentTurn,
    theme: &Theme,
    turn_idx: usize,
) -> f32 {
    // ── Frame::Prepared: render content first, then paint background ──
    // This lets us inspect tool call results AFTER rendering and apply
    // dynamic color logic (error tint, activity indicator) without a
    // second pass.
    let mut prepared = egui::Frame::new()
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .inner_margin(egui::Margin::symmetric(0, theme.space_4 as i8 + 2))
        .begin(ui);

    // ── Header: avatar + "Agent" + meta ──
    prepared.content_ui.horizontal(|ui| {
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
    design_system::gap(&mut prepared.content_ui, Space::S1);

    // ── Thinking (collapsed by default) ──
    if let Some(ref thinking) = turn.thinking {
        let label = format!("思考过程 ▼ · {} tokens", thinking.token_hint);
        egui::CollapsingHeader::new(
            egui::RichText::new(label)
                .size(theme.text_sm)
                .color(theme.text_muted),
        )
        .id_salt(format!("agent_turn_thinking_cli_{}", turn_idx))
        .default_open(false)
        .show(&mut prepared.content_ui, |ui| {
            for step in &thinking.steps {
                ui.label(
                    egui::RichText::new(step)
                        .size(theme.text_sm)
                        .color(theme.chat_text),
                );
            }
        });
        design_system::gap(&mut prepared.content_ui, Space::S0);
    }

    // ── Tool calls (folded into single summary line) ──
    let has_errors = turn
        .tool_calls
        .iter()
        .any(|tc| tc.status == ToolCallStatus::Error);
    let has_warnings = turn
        .tool_calls
        .iter()
        .any(|tc| tc.status == ToolCallStatus::Warning);

    if !turn.tool_calls.is_empty() {
        let summary = format!("{} tools", turn.tool_calls.len());
        egui::CollapsingHeader::new(
            egui::RichText::new(summary)
                .size(theme.text_sm)
                .color(theme.text_dim),
        )
        .id_salt(format!("agent_turn_tools_cli_{}", turn_idx))
        .default_open(false)
        .show(&mut prepared.content_ui, |ui| {
            for tc in &turn.tool_calls {
                render_tool_call_row_cli(ui, tc, theme);
            }
        });
        design_system::gap(&mut prepared.content_ui, Space::S0);
    }

    // ── Final response (plain, no card wrapper) ──
    if let Some(ref msg) = turn.final_response {
        prepared
            .content_ui
            .with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                ui.set_max_width(ui.available_width());
                if msg.parsed.is_empty() {
                    ui.label(
                        egui::RichText::new(&msg.content)
                            .size(theme.text_base)
                            .color(theme.chat_text),
                    );
                } else {
                    crate::ui::markdown::render_blocks(ui, &msg.parsed, theme, theme.chat_text);
                }
            });
        design_system::gap(&mut prepared.content_ui, Space::S2);
    }

    design_system::gap(&mut prepared.content_ui, Space::S3);

    // ── Dynamic coloring via Frame::Prepared ──
    // After content is rendered, inspect tool call status and apply
    // a colored left-edge accent when errors or warnings occurred.
    if has_errors {
        prepared.frame.fill = theme.danger.linear_multiply(0.04);
        prepared.frame.stroke = egui::Stroke::new(2.0, theme.danger.linear_multiply(0.30));
        prepared.frame.corner_radius = egui::CornerRadius::same(theme.radius_sm as u8);
    } else if has_warnings {
        prepared.frame.fill = theme.warn.linear_multiply(0.04);
        prepared.frame.stroke = egui::Stroke::new(2.0, theme.warn.linear_multiply(0.25));
        prepared.frame.corner_radius = egui::CornerRadius::same(theme.radius_sm as u8);
    }

    let response = prepared.end(ui);
    let height = response.rect.height();
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
    design_system::gap(ui, Space::S0);
}

fn status_color(status: ToolCallStatus, theme: &Theme) -> egui::Color32 {
    match status {
        ToolCallStatus::Running => theme.status_busy,
        ToolCallStatus::Success => theme.ok,
        ToolCallStatus::Warning => theme.warn,
        ToolCallStatus::Error => theme.danger,
    }
}
