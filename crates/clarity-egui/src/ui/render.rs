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
    let max_width = (ui.available_width() * 0.82).max(280.0);

    let (align, bg, text_color, radius, stroke) = if msg.is_error {
        (
            egui::Align::LEFT,
            theme.error_bubble,
            theme.error_text,
            egui::CornerRadius::same(theme.radius_lg as u8),
            egui::Stroke::new(1.0, theme.danger),
        )
    } else {
        match msg.role {
            Role::User => (
                egui::Align::RIGHT,
                theme.user_bubble,
                egui::Color32::WHITE,
                egui::CornerRadius {
                    nw: (theme.radius_lg as u8),
                    ne: 4,
                    sw: (theme.radius_lg as u8),
                    se: 4,
                },
                egui::Stroke::NONE,
            ),
            Role::Agent => (
                egui::Align::LEFT,
                theme.ai_bubble,
                theme.chat_text,
                egui::CornerRadius {
                    nw: 4,
                    ne: (theme.radius_lg as u8),
                    sw: 4,
                    se: (theme.radius_lg as u8),
                },
                egui::Stroke::NONE,
            ),
        }
    };

    ui.with_layout(egui::Layout::top_down(align), |ui| {
        ui.set_max_width(max_width);
        egui::Frame::group(ui.style())
            .fill(bg)
            .corner_radius(radius)
            .stroke(stroke)
            .shadow(theme.shadow_card)
            .inner_margin(egui::Margin::symmetric(18, 14))
            .show(ui, |ui| {
                ui.set_min_width(48.0);
                if msg.is_error {
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("⚠").size(theme.text_base));
                        ui.label(
                            egui::RichText::new("Error")
                                .size(theme.text_sm)
                                .strong()
                                .color(text_color),
                        );
                    });
                    ui.add_space(theme.space_4);
                }
                crate::ui::markdown::render_blocks(ui, &msg.parsed, theme, text_color);
            });
    });
    ui.add_space(theme.space_16);
    ui.cursor().min.y - start_y
}

/// Render a tool-call lifecycle indicator bubble.
pub fn tool_call_bubble(ui: &mut egui::Ui, tc: &ToolCallInfo, theme: &Theme) {
    let bg = theme.tool_call_bg;
    let icon = match tc.status {
        ToolCallStatus::Running => "⏳",
        ToolCallStatus::Done => "✅",
    };

    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        ui.set_max_width(ui.available_width() * 0.85);
        egui::Frame::group(ui.style())
            .fill(bg)
            .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
            .stroke(egui::Stroke::new(1.0, theme.border))
            .shadow(theme.shadow_card)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(icon).size(theme.text_base));
                    ui.label(
                        egui::RichText::new(&tc.name)
                            .size(theme.text_sm)
                            .strong()
                            .color(theme.text_muted),
                    );
                });
                if let Some(ref result) = tc.result {
                    ui.label(
                        egui::RichText::new(truncate(result, 200))
                            .size(theme.text_sm)
                            .color(theme.text_muted),
                    );
                }
            });
    });
    ui.add_space(theme.space_12);
}

/// Render a typing indicator "..." bubble.
pub fn typing_indicator(ui: &mut egui::Ui, theme: &Theme) {
    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        ui.set_max_width(ui.available_width() * 0.78);
        egui::Frame::group(ui.style())
            .fill(theme.ai_bubble)
            .corner_radius(egui::CornerRadius {
                nw: 4,
                ne: (theme.radius_lg as u8),
                sw: 4,
                se: (theme.radius_lg as u8),
            })
            .stroke(egui::Stroke::NONE)
            .shadow(theme.shadow_card)
            .inner_margin(egui::Margin::symmetric(18, 12))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new("● ● ●")
                        .size(theme.text_sm)
                        .color(theme.text_muted),
                );
            });
    });
    ui.add_space(theme.space_16);
}

// ============================================================================
// Helpers
// ============================================================================

/// Pretext-style height estimation for virtual list culling.
/// Called on the cold path (once per message when height cache is missing).
pub fn estimate_height(msg: &crate::ui::types::Message) -> f32 {
    use crate::ui::types::RenderBlock;
    let mut height = 28.0; // bubble padding + trailing space_8
    for block in &msg.parsed {
        match block {
            RenderBlock::Paragraph(spans) => {
                let chars: usize = spans
                    .iter()
                    .map(|s| match s {
                        crate::ui::types::InlineSpan::Text(t)
                        | crate::ui::types::InlineSpan::Bold(t)
                        | crate::ui::types::InlineSpan::Code(t) => t.len(),
                        crate::ui::types::InlineSpan::Link { text, .. } => text.len(),
                    })
                    .sum();
                let lines = (chars / 55).max(1);
                height += lines as f32 * 18.0;
            }
            RenderBlock::Heading(_, _) => height += 24.0,
            RenderBlock::CodeBlock { code, .. } => {
                let lines = code.lines().count().max(1);
                height += lines as f32 * 16.0 + 30.0;
            }
            RenderBlock::ListItem(_) => height += 20.0,
            RenderBlock::Blockquote(_) => height += 20.0,
            RenderBlock::HorizontalRule => height += 20.0,
        }
        height += 4.0; // inter-block spacing
    }
    height
}

pub fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len])
    }
}
