//! Pure hot-path layout: message bubbles, tool calls, typing indicator.
//!
//! ARCHITECTURE CONSTRAINT (Pretext-aligned):
//!   - This file contains ONLY per-frame layout code.
//!   - Markdown parsing is FORBIDDEN here; use `msg.parsed` (prepared blocks).
//!   - `message_bubble()` writes `msg.cached_height` after measuring actual height.
//!
//! See `crates/clarity-egui/ARCHITECTURE.md` §1.1, §1.2, §2.1.

#![allow(dead_code)] // line-mode feature toggles which functions are active

use crate::pretext::EguiFontMetrics;
use crate::theme::Theme;
use crate::ui::rich_inline::text_to_spans;
use crate::ui::types::{
    ContentBlock, InlineSpan, Message, RenderBlock, Role, ToolCallInfo, ToolCallStatus,
};
use std::time::Duration;

// ============================================================================
// Render — Message bubbles, tool calls, typing indicator
// ============================================================================

/// Format a `Duration` as a human-readable elapsed time string.
fn format_elapsed(d: Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}s ago", secs)
    } else if secs < 3600 {
        format!("{}m ago", secs / 60)
    } else if secs < 86400 {
        format!("{}h ago", secs / 3600)
    } else {
        format!("{}d ago", secs / 86400)
    }
}

/// Render a user or AI message using pre-parsed markdown blocks.
/// Returns the actual rendered height (including trailing space).
///
/// Dispatches to:
/// - `user_bubble()` for user messages (right-aligned glass card)
/// - `agent_message()` for agent messages (Swiss plain text OR glass card)
/// - `error_bubble()` for error messages (left-aligned glass card)
#[allow(clippy::too_many_arguments)]
pub fn message_bubble(
    ui: &mut egui::Ui,
    msg: &Message,
    theme: &Theme,
    show_header: bool,
    msg_idx: usize,
    retry_idx: &mut Option<usize>,
    switch_model: &mut bool,
    selected_idx: Option<usize>,
    metrics: Option<&EguiFontMetrics>,
) -> f32 {
    if msg.is_error {
        error_bubble(ui, msg, theme, msg_idx, retry_idx, switch_model)
    } else if msg.role == Role::System {
        system_message(ui, msg, theme)
    } else {
        #[cfg(feature = "line-mode")]
        {
            let _ = metrics;
            match msg.role {
                Role::User => line_mode_user(ui, msg, theme, selected_idx),
                Role::Agent => line_mode_agent(ui, msg, theme, show_header, selected_idx),
                Role::System => system_message(ui, msg, theme),
            }
        }
        #[cfg(not(feature = "line-mode"))]
        {
            let _ = selected_idx;
            match msg.role {
                Role::User => user_bubble(ui, msg, theme, metrics),
                Role::Agent => agent_message(ui, msg, theme, show_header, metrics),
                Role::System => system_message(ui, msg, theme),
            }
        }
    }
}

// ── System ──

/// Render a system message as a subtle center-aligned pill.
fn system_message(ui: &mut egui::Ui, msg: &Message, theme: &Theme) -> f32 {
    let start_y = ui.cursor().min.y;
    ui.add_space(theme.space_4);
    ui.vertical_centered(|ui| {
        egui::Frame::new()
            .fill(theme.glass)
            .corner_radius(egui::CornerRadius::same(theme.radius_full as u8))
            .inner_margin(egui::Margin::symmetric(16, 4))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(&msg.content)
                        .size(theme.text_xs)
                        .color(theme.text_dim),
                );
            });
    });
    ui.add_space(theme.space_4);
    ui.cursor().min.y - start_y
}

// ── Agent ──

fn agent_message(
    ui: &mut egui::Ui,
    msg: &Message,
    theme: &Theme,
    show_header: bool,
    metrics: Option<&EguiFontMetrics>,
) -> f32 {
    let start_y = ui.cursor().min.y;

    if show_header {
        // Header: avatar + label + elapsed time (outside the card)
        ui.horizontal(|ui| {
            crate::components::chat::avatar::avatar(ui, "A", theme);
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Agent")
                    .size(theme.text_xs)
                    .color(theme.text_dim),
            );
            ui.add_space(theme.space_4);
            ui.label(
                egui::RichText::new(format_elapsed(msg.timestamp.elapsed()))
                    .size(theme.text_xs)
                    .color(theme.text_dim),
            );
        });
        ui.add_space(theme.space_4);
    }

    if msg.parsed.is_empty() {
        // Lazy parse fallback: streaming phase, show raw text without markdown parsing.
        agent_text_plain_inner(ui, msg, theme, metrics);
    } else if msg.blocks.is_empty() {
        // Fallback: render from parsed content (legacy sessions)
        if has_structure(msg) {
            agent_structured_card_inner(ui, msg, theme);
        } else {
            agent_text_plain_inner(ui, msg, theme, metrics);
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
                        render_content_block(ui, block, theme, idx, metrics);
                    }
                });
            ui.add_space(theme.space_16);
        }
    }

    ui.cursor().min.y - start_y
}

/// Agent plain text — Swiss Style: no bubble, full-width, bottom border separator.
fn agent_text_plain_inner(
    ui: &mut egui::Ui,
    msg: &Message,
    theme: &Theme,
    metrics: Option<&EguiFontMetrics>,
) {
    // Content: straight layout, text directly on page background
    let max_width = ui.available_width();
    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        ui.set_max_width(max_width);
        if let Some(metrics) = metrics {
            let profile = pretext_core::EngineProfile::chromium();
            let spans = if msg.parsed.is_empty() || is_simple_paragraph(&msg.parsed) {
                text_to_spans(&msg.content)
            } else {
                first_paragraph_spans(&msg.parsed)
            };
            crate::widgets::rich_paragraph::rich_paragraph(
                ui, &spans, theme, metrics, &profile, max_width,
            );
        } else if msg.parsed.is_empty() {
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

/// True if the parsed blocks are a single paragraph with no block-level elements.
fn is_simple_paragraph(blocks: &[RenderBlock]) -> bool {
    blocks.len() == 1 && matches!(blocks.first(), Some(RenderBlock::Paragraph(_)))
}

/// Extract spans from the first paragraph, or return an empty span list.
fn first_paragraph_spans(blocks: &[RenderBlock]) -> Vec<InlineSpan> {
    match blocks.first() {
        Some(RenderBlock::Paragraph(spans)) => spans.clone(),
        _ => Vec::new(),
    }
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

fn render_content_block(
    ui: &mut egui::Ui,
    block: &ContentBlock,
    theme: &Theme,
    block_idx: usize,
    metrics: Option<&EguiFontMetrics>,
) {
    match block {
        ContentBlock::Text { text } => {
            if let Some(metrics) = metrics {
                let profile = pretext_core::EngineProfile::chromium();
                let spans = text_to_spans(text);
                let max_width = ui.available_width();
                crate::widgets::rich_paragraph::rich_paragraph(
                    ui, &spans, theme, metrics, &profile, max_width,
                );
            } else {
                let parsed = crate::ui::markdown::parse_markdown(text);
                crate::ui::markdown::render_blocks(ui, &parsed, theme, theme.chat_text);
            }
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

fn user_bubble(
    ui: &mut egui::Ui,
    msg: &Message,
    theme: &Theme,
    metrics: Option<&EguiFontMetrics>,
) -> f32 {
    let start_y = ui.cursor().min.y;
    let max_width = (ui.available_width() * 0.72).max(280.0);

    // User messages: right-aligned within the content column, with a small
    // inset from the right edge so the bubble clearly reads as "sent".
    ui.with_layout(egui::Layout::right_to_left(egui::Align::TOP), |ui| {
        ui.add_space(theme.space_8);
        ui.set_max_width(max_width);
        let bubble_resp = egui::Frame::new()
            .fill(theme.user_bubble)
            .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8))
            .stroke(egui::Stroke::NONE)
            .shadow(egui::Shadow::NONE)
            .inner_margin(egui::Margin::symmetric(14, 10))
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                    ui.set_min_width(48.0);
                    if let Some(metrics) = metrics {
                        let profile = pretext_core::EngineProfile::chromium();
                        let spans = if msg.parsed.is_empty() || is_simple_paragraph(&msg.parsed) {
                            text_to_spans(&msg.content)
                        } else {
                            first_paragraph_spans(&msg.parsed)
                        };
                        let inner_max_width = ui.available_width();
                        crate::widgets::rich_paragraph::rich_paragraph(
                            ui,
                            &spans,
                            theme,
                            metrics,
                            &profile,
                            inner_max_width,
                        );
                    } else {
                        crate::ui::markdown::render_blocks(
                            ui,
                            &msg.parsed,
                            theme,
                            theme.text_strong,
                        );
                    }
                });
            });
        // Hover timestamp.
        if bubble_resp.response.hovered() {
            let ts = format_elapsed(msg.timestamp.elapsed());
            egui::show_tooltip_at_pointer(
                ui.ctx(),
                egui::LayerId::new(egui::Order::Tooltip, ui.id().with("user_ts_layer")),
                ui.id().with("user_ts"),
                |ui| {
                    ui.label(
                        egui::RichText::new(ts)
                            .size(theme.text_xs)
                            .color(theme.text_dim),
                    );
                },
            );
        }
        ui.add_space(theme.space_8);
    });
    ui.add_space(theme.space_16);
    ui.cursor().min.y - start_y
}

// ── Error ──

fn error_bubble(
    ui: &mut egui::Ui,
    msg: &Message,
    theme: &Theme,
    msg_idx: usize,
    retry_idx: &mut Option<usize>,
    switch_model: &mut bool,
) -> f32 {
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
            .stroke(egui::Stroke::new(1.0_f32, theme.danger))
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

                        // Retry button
                        let retry_btn = egui::Button::new(
                            egui::RichText::new(crate::theme::ICON_REFRESH)
                                .font(theme.font_icon(theme.text_xs))
                                .color(theme.error_text),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
                        if ui.add(retry_btn).on_hover_text("Retry").clicked() {
                            *retry_idx = Some(msg_idx);
                        }

                        // Switch Model button
                        let switch_btn = egui::Button::new(
                            egui::RichText::new(crate::theme::ICON_SETTINGS)
                                .font(theme.font_icon(theme.text_xs))
                                .color(theme.error_text),
                        )
                        .fill(egui::Color32::TRANSPARENT)
                        .stroke(egui::Stroke::NONE)
                        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
                        if ui.add(switch_btn).on_hover_text("Switch Model").clicked() {
                            *switch_model = true;
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
// Line-mode renderers (S6 Phase 2C)
// ============================================================================

#[cfg(feature = "line-mode")]
fn line_mode_agent(
    ui: &mut egui::Ui,
    msg: &Message,
    theme: &Theme,
    show_header: bool,
    selected_idx: Option<usize>,
) -> f32 {
    let start_y = ui.cursor().min.y;

    if show_header {
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

    let max_width = (ui.available_width() - 32.0).max(120.0);
    egui::Frame::new()
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
        .stroke(egui::Stroke::NONE)
        .shadow(egui::Shadow::NONE)
        .inner_margin(egui::Margin::symmetric(16, 12))
        .show(ui, |ui| {
            ui.set_max_width(max_width);
            crate::ui::line_renderer::render_lines(
                ui,
                &msg.lines,
                theme,
                0.0,
                1_000_000.0,
                selected_idx,
            );
        });
    ui.add_space(theme.space_16);
    ui.cursor().min.y - start_y
}

#[cfg(feature = "line-mode")]
fn line_mode_user(
    ui: &mut egui::Ui,
    msg: &Message,
    theme: &Theme,
    selected_idx: Option<usize>,
) -> f32 {
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
                    crate::ui::line_renderer::render_lines(
                        ui,
                        &msg.lines,
                        theme,
                        0.0,
                        1_000_000.0,
                        selected_idx,
                    );
                });
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
            .stroke(egui::Stroke::NONE)
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

/// Height estimation for virtual list culling.
/// Called on the cold path (once per message when height cache is missing).
///
/// Pretext is now the only supported path; the legacy character-count
/// heuristic has been removed in Phase 4.
pub fn estimate_height(
    msg: &crate::ui::types::Message,
    content_max_width: f32,
    theme: &crate::theme::Theme,
    metrics: &EguiFontMetrics,
) -> f32 {
    #[cfg(feature = "line-mode")]
    {
        let _ = content_max_width;
        let _ = metrics;
        let line_h = crate::ui::line_renderer::LINE_HEIGHT;
        let lines_h = msg.lines.len() as f32 * line_h;
        // Padding for bubble frame + trailing space.
        return lines_h + theme.space_16 + theme.space_8;
    }

    #[cfg(not(feature = "line-mode"))]
    {
        estimate_height_pretext(msg, content_max_width, theme, metrics)
    }
}

/// Pretext-based estimator. Measures paragraph text width against the actual
/// bubble width to compute line count.
fn estimate_height_pretext(
    msg: &crate::ui::types::Message,
    content_max_width: f32,
    theme: &crate::theme::Theme,
    metrics: &EguiFontMetrics,
) -> f32 {
    use crate::ui::types::RenderBlock;

    // Bubble padding matches the actual render path:
    // - user bubble: inner_margin vertical 14*2 + trailing space_16 = 44
    // - agent plain text: trailing space_12
    // - agent card (blocks / structured): inner_margin vertical 12*2 + trailing space_16 = 40
    let base_padding = match msg.role {
        Role::User => 44.0,
        Role::System => 12.0,
        Role::Agent => {
            let has_visible_blocks = !msg.blocks.is_empty()
                && msg.blocks.iter().any(|b| {
                    matches!(
                        b,
                        crate::ui::types::ContentBlock::Text { .. }
                            | crate::ui::types::ContentBlock::Code { .. }
                            | crate::ui::types::ContentBlock::Think { .. }
                            | crate::ui::types::ContentBlock::Plan { .. }
                            | crate::ui::types::ContentBlock::FilePreview { .. }
                            | crate::ui::types::ContentBlock::ToolResult { .. }
                    )
                });
            if has_visible_blocks || has_structure(msg) {
                40.0
            } else {
                12.0
            }
        }
    };
    let mut height = base_padding;
    let font = crate::pretext::font_body(theme);
    let line_height = metrics.row_height(&font) * 1.2;
    let options = pretext_core::PrepareOptions::default();
    let profile = pretext_core::EngineProfile::chromium();

    // Available width for the text itself.
    // User bubble has inner margin; agent plain text uses the full available width.
    let text_width = match msg.role {
        Role::User => ((content_max_width * 0.72).max(280.0) - 36.0).max(120.0),
        Role::Agent => content_max_width.max(120.0),
        Role::System => content_max_width.min(360.0).max(120.0),
    };

    for block in &msg.parsed {
        match block {
            RenderBlock::Paragraph(spans) => {
                let text = paragraph_text(spans);
                let lines = pretext_core::prepare_with_segments(&text, &font, metrics, &options)
                    .and_then(|p| pretext_core::layout::layout_with_lines(&p, text_width, &profile))
                    .map(|r| r.line_count.max(1))
                    .unwrap_or(1);
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
                height += line_height + theme.space_8;
                height += rows.len() as f32 * (line_height + theme.space_4);
                height += theme.space_8;
            }
            RenderBlock::Diff { hunks, .. } => {
                // Rough estimate: each hunk is ~8 lines + header.
                let hunk_lines: usize = hunks.iter().map(|h| h.lines.len()).sum();
                height += hunk_lines as f32 * line_height + theme.space_16;
            }
        }
        height += theme.space_4; // inter-block spacing
    }
    height
}

fn paragraph_text(spans: &[InlineSpan]) -> String {
    spans
        .iter()
        .map(|s| match s {
            InlineSpan::Text(t) | InlineSpan::Bold(t) | InlineSpan::Code(t) => t.as_str(),
            InlineSpan::Link { text, .. } => text.as_str(),
        })
        .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_elapsed_seconds() {
        assert_eq!(format_elapsed(Duration::from_secs(0)), "0s ago");
        assert_eq!(format_elapsed(Duration::from_secs(42)), "42s ago");
        assert_eq!(format_elapsed(Duration::from_secs(59)), "59s ago");
    }

    #[test]
    fn format_elapsed_minutes() {
        assert_eq!(format_elapsed(Duration::from_secs(60)), "1m ago");
        assert_eq!(format_elapsed(Duration::from_secs(3599)), "59m ago");
    }

    #[test]
    fn format_elapsed_hours() {
        assert_eq!(format_elapsed(Duration::from_secs(3600)), "1h ago");
        assert_eq!(format_elapsed(Duration::from_secs(86399)), "23h ago");
    }

    #[test]
    fn format_elapsed_days() {
        assert_eq!(format_elapsed(Duration::from_secs(86400)), "1d ago");
        assert_eq!(format_elapsed(Duration::from_secs(172800)), "2d ago");
    }
}
