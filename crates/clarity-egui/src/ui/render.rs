//! Pure hot-path layout: message bubbles, tool calls, typing indicator.
//!
//! ARCHITECTURE CONSTRAINT (Pretext-aligned):
//!   - This file contains ONLY per-frame layout code.
//!   - Markdown parsing is FORBIDDEN here; use `msg.parsed` (prepared blocks).
//!   - `message_bubble()` writes `msg.cached_height` after measuring actual height.
//!
//! See `crates/clarity-egui/ARCHITECTURE.md` §1.1, §1.2, §2.1.

use crate::theme::Theme;
use crate::ui::types::{Message, RenderBlock, Role, ToolCallInfo, ToolCallStatus};

// ============================================================================
// Render — Message bubbles, tool calls, typing indicator
// ============================================================================

/// Render a user or AI message using pre-parsed markdown blocks.
/// Returns the actual rendered height (including trailing space).
///
/// Dispatches to:
/// - `user_bubble()` for user messages (right-aligned glass card)
/// - `agent_message()` for agent messages (Swiss plain text OR glass card)
/// - `error_bubble()` for error messages (left-aligned glass card)
pub fn message_bubble(ui: &mut egui::Ui, msg: &Message, theme: &Theme) -> f32 {
    if msg.is_error {
        error_bubble(ui, msg, theme)
    } else {
        match msg.role {
            Role::User => user_bubble(ui, msg, theme),
            Role::Agent => agent_message(ui, msg, theme),
        }
    }
}

// ── Agent ──

fn agent_message(ui: &mut egui::Ui, msg: &Message, theme: &Theme) -> f32 {
    let start_y = ui.cursor().min.y;

    if has_structure(msg) {
        agent_structured_card(ui, msg, theme);
    } else {
        agent_text_plain(ui, msg, theme);
    }

    ui.cursor().min.y - start_y
}

/// Agent plain text — Swiss Style: no bubble, full-width, bottom border separator.
fn agent_text_plain(ui: &mut egui::Ui, msg: &Message, theme: &Theme) {
    // Header: avatar + label
    ui.horizontal(|ui| {
        crate::components::chat::avatar::avatar(ui, "A", theme);
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Agent")
                .size(theme.text_xs)
                .color(theme.text_dim),
        );
    });
    ui.add_space(theme.space_4);

    // Content: straight layout, text directly on page background
    let max_width = ui.available_width();
    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        ui.set_max_width(max_width);
        crate::ui::markdown::render_blocks(ui, &msg.parsed, theme, theme.chat_text);
    });

    // Bottom spacing — Swiss Style: whitespace instead of lines
    ui.add_space(theme.space_12);
}

/// Agent structured content — Glassmorphism card for code blocks, tools, etc.
fn agent_structured_card(ui: &mut egui::Ui, msg: &Message, theme: &Theme) {
    // Header: avatar + label (outside the card)
    ui.horizontal(|ui| {
        crate::components::chat::avatar::avatar(ui, "A", theme);
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("Agent")
                .size(theme.text_xs)
                .color(theme.text_dim),
        );
    });
    ui.add_space(theme.space_4);

    // Glass card container — subtract padding to avoid parent overflow
    let max_width = (ui.available_width() - 32.0).max(120.0);
    egui::Frame::new()
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
        .stroke(egui::Stroke::NONE)
        .shadow(egui::Shadow::NONE)
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_max_width(max_width);
            crate::ui::markdown::render_blocks(ui, &msg.parsed, theme, theme.chat_text);
        });

    ui.add_space(theme.space_16);
}

/// Check if message contains structured blocks (code blocks, etc.) that need a card container.
fn has_structure(msg: &Message) -> bool {
    msg.parsed
        .iter()
        .any(|b| matches!(b, RenderBlock::CodeBlock { .. }))
}

// ── User ──

fn user_bubble(ui: &mut egui::Ui, msg: &Message, theme: &Theme) -> f32 {
    let start_y = ui.cursor().min.y;
    let max_width = (ui.available_width() * 0.72).max(280.0);

    ui.with_layout(egui::Layout::top_down(egui::Align::RIGHT), |ui| {
        ui.set_max_width(max_width);
        egui::Frame::new()
            .fill(theme.user_bubble)
            .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8))
            .stroke(egui::Stroke::NONE)
            .shadow(egui::Shadow::NONE)
            .inner_margin(egui::Margin::symmetric(18, 14))
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    ui.set_min_width(48.0);
                    crate::ui::markdown::render_blocks(ui, &msg.parsed, theme, theme.text_strong);
                });
            });
    });
    ui.add_space(theme.space_16);
    ui.cursor().min.y - start_y
}

// ── Error ──

fn error_bubble(ui: &mut egui::Ui, msg: &Message, theme: &Theme) -> f32 {
    let start_y = ui.cursor().min.y;
    let max_width = (ui.available_width() * 0.72).max(280.0);

    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        ui.set_max_width(max_width);
        egui::Frame::new()
            .fill(theme.error_bubble)
            .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8))
            .stroke(egui::Stroke::new(1.0, theme.error_text))
            .shadow(egui::Shadow::NONE)
            .inner_margin(egui::Margin::symmetric(18, 14))
            .show(ui, |ui| {
                ui.set_min_width(48.0);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(crate::theme::ICON_WARNING)
                            .font(theme.font_icon(theme.text_base)),
                    );
                    ui.label(
                        egui::RichText::new("Error")
                            .size(theme.text_sm)
                            .strong()
                            .color(theme.error_text),
                    );
                });
                ui.add_space(theme.space_4);
                crate::ui::markdown::render_blocks(ui, &msg.parsed, theme, theme.error_text);
            });
    });
    ui.add_space(theme.space_16);
    ui.cursor().min.y - start_y
}

// ============================================================================
// Tool call bubble
// ============================================================================

/// Render a tool-call lifecycle indicator bubble.
#[allow(dead_code)]
pub fn tool_call_bubble(ui: &mut egui::Ui, tc: &ToolCallInfo, theme: &Theme) {
    let bg = theme.tool_call_bg;
    let icon = match tc.status {
        ToolCallStatus::Running => crate::theme::ICON_HOURGLASS,
        ToolCallStatus::Done => crate::theme::ICON_CHECK,
    };

    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        ui.set_max_width(ui.available_width() * 0.85);
        egui::Frame::new()
            .fill(bg)
            .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
            .stroke(egui::Stroke::new(1.0, theme.border))
            .shadow(egui::Shadow::NONE)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(icon).font(theme.font_icon(theme.text_base)),
                    );
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

// ============================================================================
// Typing indicator
// ============================================================================

/// Render a typing indicator "..." bubble.
pub fn typing_indicator(ui: &mut egui::Ui, theme: &Theme) {
    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        ui.set_max_width(ui.available_width() * 0.78);
        egui::Frame::new()
            .fill(theme.ai_bubble)
            .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
            .stroke(egui::Stroke::NONE)
            .shadow(egui::Shadow::NONE)
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
