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

    if msg.parsed.is_empty() {
        // Lazy parse fallback: streaming phase, show raw text without markdown parsing.
        agent_text_plain_inner(ui, msg, theme);
    } else if msg.blocks.is_empty() {
        // Fallback: render from parsed content (legacy sessions)
        if has_structure(msg) {
            agent_structured_card_inner(ui, msg, theme);
        } else {
            agent_text_plain_inner(ui, msg, theme);
        }
    } else {
        // Phase 1: render blocks with type-aware strategy
        let visible_blocks: Vec<&ContentBlock> = msg
            .blocks
            .iter()
            .filter(|b| should_show_in_chat(b))
            .collect();
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
                    for (idx, block) in visible_blocks.iter().enumerate() {
                        render_content_block(ui, block, theme, idx);
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
        if msg.parsed.is_empty() {
            // Lazy parse fallback: streaming phase, show raw text.
            ui.label(
                egui::RichText::new(&msg.content)
                    .size(theme.text_base)
                    .color(theme.chat_text),
            );
        } else {
            crate::ui::markdown::render_blocks(ui, &msg.parsed, theme, theme.chat_text);
        }
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
        .any(|b| matches!(b, RenderBlock::CodeBlock { .. } | RenderBlock::Table { .. }))
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

fn render_content_block(ui: &mut egui::Ui, block: &ContentBlock, theme: &Theme, block_idx: usize) {
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
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let copy_btn = egui::Button::new(
                        egui::RichText::new(crate::theme::ICON_COPY)
                            .font(theme.font_icon(theme.text_xs))
                            .color(theme.text_muted),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
                    if ui.add(copy_btn).on_hover_text("Copy code").clicked() {
                        ui.ctx().copy_text(code.clone());
                    }
                });
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
        ContentBlock::ToolResult {
            name,
            output,
            truncated,
            ..
        } => {
            let header = format!("🔧 {}", name);
            egui::CollapsingHeader::new(
                egui::RichText::new(header)
                    .size(theme.text_sm)
                    .strong()
                    .color(theme.text_muted),
            )
            .id_salt(format!("tool_result_{}_{}", name, block_idx))
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
            .id_salt(format!("think_{}", block_idx))
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

    let content = msg.content.trim();
    let is_long = content.len() > 120 || content.lines().count() > 2;

    // Stable fold state keyed by message content hash.
    let fold_id = ui.id().with({
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        let mut h = DefaultHasher::new();
        content.hash(&mut h);
        h.finish()
    });
    let mut folded = ui.data_mut(|d| *d.get_temp_mut_or(fold_id, is_long));

    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        ui.set_max_width(max_width);
        egui::Frame::new()
            .fill(theme.error_bubble)
            .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8))
            .stroke(egui::Stroke::new(1.0_f32, theme.error_text))
            .shadow(egui::Shadow::NONE)
            .inner_margin(egui::Margin::symmetric(18, 14))
            .show(ui, |ui| {
                ui.set_min_width(48.0);

                // Header: icon + label + action buttons
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

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Copy button
                        let copy_btn = egui::Button::new(
                            egui::RichText::new(crate::theme::ICON_COPY)
                                .font(theme.font_icon(theme.text_xs))
                                .color(theme.error_text),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
                        if ui.add(copy_btn).on_hover_text("Copy error").clicked() {
                            ui.ctx().copy_text(content.to_string());
                        }

                        // Fold / Unfold
                        if is_long {
                            let fold_label = if folded { "Expand" } else { "Collapse" };
                            let fold_btn = egui::Button::new(
                                egui::RichText::new(fold_label)
                                    .size(theme.text_xs)
                                    .color(theme.error_text),
                            )
                            .fill(egui::Color32::TRANSPARENT)
                            .stroke(egui::Stroke::NONE)
                            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
                            if ui.add(fold_btn).clicked() {
                                folded = !folded;
                                ui.data_mut(|d| d.insert_temp(fold_id, folded));
                            }
                        }
                    });
                });

                ui.add_space(theme.space_4);

                if folded && is_long {
                    // Collapsed: show first line truncated to ~80 chars
                    let first_line = content.lines().next().unwrap_or(content);
                    let summary: String = first_line.chars().take(80).collect();
                    let ellipsis = if first_line.len() > 80 { "…" } else { "" };
                    ui.label(
                        egui::RichText::new(format!("{}{}", summary, ellipsis))
                            .size(theme.text_sm)
                            .color(theme.error_text),
                    );
                } else {
                    // Expanded: render full parsed blocks
                    crate::ui::markdown::render_blocks(ui, &msg.parsed, theme, theme.error_text);
                }
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
            .stroke(egui::Stroke::new(1.0_f32, theme.border))
            .shadow(egui::Shadow::NONE)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new(icon).font(theme.font_icon(theme.text_base)));
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
pub fn estimate_height(
    msg: &crate::ui::types::Message,
    content_max_width: f32,
    theme: &crate::theme::Theme,
) -> f32 {
    use crate::ui::types::RenderBlock;
    let mut height = 28.0; // bubble padding + trailing space_8

    // Approximate chars per line based on available width and base font size.
    // Average glyph width ≈ text_base * 0.65 (mix of Latin and CJK).
    let chars_per_line = ((content_max_width / (theme.text_base * 0.65)).max(20.0)) as usize;
    let line_height = theme.text_base * 1.5; // ~18px when text_base=12.0

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
                let lines = (chars / chars_per_line).max(1);
                height += lines as f32 * line_height;
            }
            RenderBlock::Heading(_, _) => height += theme.text_lg + theme.space_8,
            RenderBlock::CodeBlock { code, .. } => {
                let lines = code.lines().count().max(1);
                height += lines as f32 * (theme.text_sm + theme.space_4) + 30.0;
            }
            RenderBlock::ListItem(_) => height += line_height + theme.space_4,
            RenderBlock::Blockquote(_) => height += line_height + theme.space_4,
            RenderBlock::HorizontalRule => height += theme.space_12,
            RenderBlock::Table { rows, .. } => {
                height += line_height + theme.space_8; // header row
                height += rows.len() as f32 * (line_height + theme.space_4);
                height += theme.space_8; // padding
            }
        }
        height += theme.space_4; // inter-block spacing
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
