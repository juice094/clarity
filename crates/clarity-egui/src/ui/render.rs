//! Pure hot-path layout: message bubbles, tool calls, typing indicator.
//!
//! ARCHITECTURE CONSTRAINT (Pretext-aligned):
//!   - This file contains ONLY per-frame layout code.
//!   - Markdown parsing is FORBIDDEN here; use `msg.parsed` (prepared blocks).
//!   - `message_bubble()` writes `msg.cached_height` after measuring actual height.
//!
//! See `crates/clarity-egui/ARCHITECTURE.md` §1.1, §1.2, §2.1.

use crate::theme::Theme;
use crate::ui::types::{Message, Role, ToolCallInfo, ToolCallStatus};

// ============================================================================
// Render — Message bubbles, tool calls, typing indicator
// ============================================================================

/// Render a user or AI message bubble using pre-parsed markdown blocks.
/// Returns the actual rendered height (including trailing space).
pub fn message_bubble(ui: &mut egui::Ui, msg: &Message, theme: &Theme) -> f32 {
    let start_y = ui.cursor().min.y;
    let (align, bg, text_color) = match msg.role {
        Role::User => (
            egui::Align::RIGHT,
            theme.user_bubble,
            egui::Color32::WHITE,
        ),
        Role::Agent => (egui::Align::LEFT, theme.ai_bubble, theme.chat_text),
    };

    ui.with_layout(egui::Layout::top_down(align), |ui| {
        egui::Frame::group(ui.style())
            .fill(bg)
            .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                crate::ui::markdown::render_blocks(ui, &msg.parsed, theme, text_color);
            });
    });
    ui.add_space(theme.space_8);
    ui.cursor().min.y - start_y
}

/// Render a tool-call lifecycle indicator bubble.
pub fn tool_call_bubble(ui: &mut egui::Ui, tc: &ToolCallInfo, theme: &Theme) {
    let bg = theme.surface;
    let icon = match tc.status {
        ToolCallStatus::Running => "⏳",
        ToolCallStatus::Done => "✅",
    };
    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        egui::Frame::group(ui.style())
            .fill(bg)
            .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
            .stroke(egui::Stroke::new(1.0, theme.border))
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(icon).size(14.0));
                    ui.label(
                        egui::RichText::new(&tc.name)
                            .size(12.0)
                            .strong()
                            .color(theme.text_muted),
                    );
                });
                if let Some(ref result) = tc.result {
                    ui.label(
                        egui::RichText::new(truncate(result, 200))
                            .size(11.0)
                            .color(theme.text_muted),
                    );
                }
            });
    });
    ui.add_space(theme.space_8);
}

/// Render a typing indicator "..." bubble.
pub fn typing_indicator(ui: &mut egui::Ui, theme: &Theme) {
    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        egui::Frame::group(ui.style())
            .fill(theme.ai_bubble)
            .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8))
            .stroke(egui::Stroke::NONE)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("● ● ●")
                        .size(12.0)
                        .color(theme.text_muted),
                );
            });
    });
    ui.add_space(theme.space_8);
}

// ============================================================================
// Helpers
// ============================================================================

pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len])
    }
}
