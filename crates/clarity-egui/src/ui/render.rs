//! Pure hot-path layout: message bubbles, tool calls, typing indicator.
//!
//! ARCHITECTURE CONSTRAINT (Pretext-aligned):
//!   - This file contains ONLY per-frame layout code.
//!   - Markdown parsing is FORBIDDEN here; use `msg.parsed` (prepared blocks).
//!   - `message_bubble()` writes `msg.cached_height` after measuring actual height.
//!
//! See `crates/clarity-egui/ARCHITECTURE.md` §1.1, §1.2, §2.1.

use crate::theme::Theme;
use crate::ui::types::{ContentBlock, Message, RenderBlock, Role, ToolCallInfo, ToolCallStatus};

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
pub fn message_bubble(ui: &mut egui::Ui, msg: &Message, theme: &Theme, show_header: bool) -> f32 {
    if msg.is_error {
        error_bubble(ui, msg, theme)
    } else {
        match msg.role {
            Role::User => user_bubble(ui, msg, theme),
            Role::Agent => agent_message(ui, msg, theme, show_header),
        }
    }
}

// ── Agent ──

fn agent_message(ui: &mut egui::Ui, msg: &Message, theme: &Theme, show_header: bool) -> f32 {
    let start_y = ui.cursor().min.y;

    if show_header {
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
    }

    if msg.blocks.is_empty() {
        // Fallback: render from parsed content (legacy sessions)
        if has_structure(msg) {
            agent_structured_card_inner(ui, msg, theme);
        } else {
            agent_text_plain_inner(ui, msg, theme);
        }
    } else {
        // Phase 1: render blocks with type-aware strategy
        let visible_blocks: Vec<&ContentBlock> = msg.blocks.iter().filter(|b| should_show_in_chat(b)).collect();
        if !visible_blocks.is_empty() {
            let max_width = (ui.available_width() - 32.0).max(120.0);
            egui::Frame::new()
                .fill(theme.surface)
                .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
                .stroke(egui::Stroke::NONE)
                .shadow(egui::Shadow::NONE)
                .inner_margin(egui::Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    ui.set_max_width(max_width);
                    for block in visible_blocks {
                        render_content_block(ui, block, theme);
                    }
                });
            ui.add_space(theme.space_16);
        }
    }

    ui.cursor().min.y - start_y
}

/// Agent plain text — Swiss Style: no bubble, full-width, bottom border separator.
fn agent_text_plain_inner(ui: &mut egui::Ui, msg: &Message, theme: &Theme) {
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
fn agent_structured_card_inner(ui: &mut egui::Ui, msg: &Message, theme: &Theme) {
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

// ============================================================================
// Phase 1 — ContentBlock rendering
// ============================================================================

fn should_show_in_chat(block: &ContentBlock) -> bool {
    match block {
        ContentBlock::Text { .. } | ContentBlock::Code { .. } => true,
        ContentBlock::ToolResult { name, .. } => should_show_tool_in_chat(name),
        ContentBlock::ToolCall { .. } => false,
        ContentBlock::Think { .. } => true,
        ContentBlock::Plan { .. } => true,
        ContentBlock::FilePreview { .. } => true,
    }
}

fn should_show_tool_in_chat(name: &str) -> bool {
    matches!(name, "file_read" | "file_write" | "plan" | "grep")
}

fn render_content_block(ui: &mut egui::Ui, block: &ContentBlock, theme: &Theme) {
    match block {
        ContentBlock::Text { text } => {
            let parsed = crate::ui::markdown::parse_markdown(text);
            crate::ui::markdown::render_blocks(ui, &parsed, theme, theme.chat_text);
            ui.add_space(theme.space_4);
        }
        ContentBlock::Code { language, code } => {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(language)
                        .size(theme.text_xs)
                        .color(theme.text_muted)
                        .monospace(),
                );
            });
            egui::Frame::new()
                .fill(theme.code_block_bg)
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                .inner_margin(egui::Margin::same(10))
                .show(ui, |ui| {
                    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                        ui.set_min_width(ui.available_width());
                        ui.label(
                            egui::RichText::new(code)
                                .font(theme.font_mono(theme.text_sm))
                                .color(theme.chat_text),
                        );
                    });
                });
            ui.add_space(theme.space_8);
        }
        ContentBlock::ToolResult { name, output, truncated, .. } => {
            let header = format!("🔧 {}", name);
            egui::CollapsingHeader::new(
                egui::RichText::new(header)
                    .size(theme.text_sm)
                    .strong()
                    .color(theme.text_muted),
            )
            .id_salt(format!("tool_result_{}", name))
            .default_open(false)
            .show(ui, |ui| {
                let parsed = crate::ui::markdown::parse_markdown(output);
                crate::ui::markdown::render_blocks(ui, &parsed, theme, theme.chat_text);
                if *truncated {
                    ui.label(
                        egui::RichText::new("(truncated)")
                            .size(theme.text_xs)
                            .color(theme.text_dim)
                            .italics(),
                    );
                }
            });
            ui.add_space(theme.space_4);
        }
        ContentBlock::ToolCall { .. } => {
            // Intentionally not rendered in the main chat area.
        }
        ContentBlock::Think { steps } => {
            egui::CollapsingHeader::new(
                egui::RichText::new(format!("Thinking ({})", steps.len()))
                    .size(theme.text_sm)
                    .strong()
                    .color(theme.text_muted),
            )
            .default_open(false)
            .show(ui, |ui| {
                for step in steps {
                    ui.label(
                        egui::RichText::new(step)
                            .size(theme.text_sm)
                            .color(theme.chat_text),
                    );
                }
            });
            ui.add_space(theme.space_8);
        }
        ContentBlock::Plan { title, steps } => {
            ui.label(
                egui::RichText::new(title)
                    .size(theme.text_base)
                    .strong()
                    .color(theme.chat_text),
            );
            ui.add_space(2.0);
            for step in steps {
                ui.label(
                    egui::RichText::new(format!("• {}", step))
                        .size(theme.text_sm)
                        .color(theme.chat_text),
                );
            }
            ui.add_space(theme.space_8);
        }
        ContentBlock::FilePreview { path, content } => {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("📄")
                        .size(theme.text_sm)
                        .color(theme.text_muted),
                );
                ui.label(
                    egui::RichText::new(path)
                        .size(theme.text_sm)
                        .strong()
                        .color(theme.text_muted),
                );
            });
            ui.add_space(2.0);
            let parsed = crate::ui::markdown::parse_markdown(content);
            crate::ui::markdown::render_blocks(ui, &parsed, theme, theme.chat_text);
            ui.add_space(theme.space_8);
        }
    }
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
    let icon = match tc.inferred_status() {
        ToolCallStatus::Running => crate::theme::ICON_HOURGLASS,
        ToolCallStatus::Success => crate::theme::ICON_CHECK,
        ToolCallStatus::Warning => crate::theme::ICON_WARNING,
        ToolCallStatus::Error => crate::theme::ICON_X,
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

/// Truncate a string to at most `max_chars` characters, appending "…" if truncated.
/// UTF-8 safe: operates on character boundaries, not byte indices.
pub fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max_chars).collect::<String>())
    }
}
