//! Turn-level rendering — CLI style for AgentTurn.

use crate::components::agent_turn::{AgentTurn, ToolCallRow, TurnHeader};
use crate::design_system::{self, Space, TextStyle};
use crate::i18n::Locale;
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
    locale: Locale,
) -> f32 {
    // ── Frame::Prepared: render content first, then paint background ──
    // This lets us inspect tool call results AFTER rendering and apply
    // dynamic color logic (error tint, activity indicator) without a
    // second pass.
    let mut prepared = clarity_ui::design_system::Elevation::Base
        .frame(theme)
        .fill(egui::Color32::TRANSPARENT)
        .stroke(egui::Stroke::NONE)
        .inner_margin(egui::Margin::symmetric(0, theme.space_4 as i8 + 2))
        .begin(ui);

    // P6h: Kimi-style agent messages have no heavy header. The avatar/model
    // label is removed; turn meta (duration · tools · tokens) is a single
    // caption line, and tool status is surfaced via the collapsible tool
    // group below.
    let header_line = turn_header_summary(&turn.header, locale);
    if !header_line.is_empty() {
        prepared.content_ui.label(
            egui::RichText::new(header_line)
                .size(theme.text_xs)
                .color(theme.text_dim),
        );
        design_system::gap(&mut prepared.content_ui, Space::S0);
    }

    // ── Thinking (collapsed by default) ──
    if let Some(ref thinking) = turn.thinking {
        let label = format!(
            "{} ▼ · {} {}",
            locale.t("Thinking"),
            thinking.token_hint,
            locale.t("tokens")
        );
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
        let summary = format!("{} {}", turn.tool_calls.len(), locale.t("tools"));
        egui::CollapsingHeader::new(
            egui::RichText::new(summary)
                .size(theme.text_sm)
                .color(theme.text_dim),
        )
        .id_salt(format!("agent_turn_tools_cli_{}", turn_idx))
        .default_open(false)
        .show(&mut prepared.content_ui, |ui| {
            for tc in &mut turn.tool_calls {
                render_tool_call_row_cli(ui, tc, theme, locale);
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
                    crate::ui::markdown::render_markdown(ui, &msg.content, theme.chat_text);
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

fn render_tool_call_row_cli(
    ui: &mut egui::Ui,
    tc: &mut ToolCallRow,
    theme: &Theme,
    locale: Locale,
) {
    let stripe_color = status_color(tc.status, theme);
    let icon = match tc.status {
        ToolCallStatus::Running => crate::theme::ICON_HOURGLASS,
        ToolCallStatus::Success => crate::theme::ICON_CHECK,
        ToolCallStatus::Warning => crate::theme::ICON_WARNING,
        ToolCallStatus::Error => crate::theme::ICON_X,
    };

    let row = ui
        .horizontal(|ui| {
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

            clarity_ui::design_system::icon(ui, icon, theme.text_sm);
            clarity_ui::design_system::text_with_color(
                ui,
                &tc.name,
                clarity_ui::design_system::TextStyle::Small.strong(),
                theme.text_muted,
            );
            design_system::text(ui, &tc.result_preview, TextStyle::Small);
        })
        .response
        .interact(egui::Sense::click())
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if row.clicked() {
        tc.expanded = !tc.expanded;
    }

    // Expanded detail: full arguments + full output. The height change is
    // picked up by the render-measured `write_back_unit_height` in
    // `message_list`, so the virtual-list cache self-corrects next frame.
    if tc.expanded {
        render_tool_call_detail(ui, tc, theme, locale);
    }
    design_system::gap(ui, Space::S0);
}

/// Render the expanded detail of a single tool call: pretty-printed arguments
/// and the full output, capped at [`MAX_DETAIL_LINES`] lines.
fn render_tool_call_detail(ui: &mut egui::Ui, tc: &ToolCallRow, theme: &Theme, locale: Locale) {
    ui.horizontal(|ui| {
        ui.add_space(theme.space_24);
        ui.vertical(|ui| {
            ui.set_max_width(ui.available_width());
            if let Some(ref args) = tc.arguments {
                design_system::text(ui, locale.t("Arguments"), TextStyle::CaptionStrong);
                render_code_block(ui, "json", &pretty_json(args), theme);
                design_system::gap(ui, Space::S0);
            }

            design_system::text(ui, locale.t("Output"), TextStyle::CaptionStrong);
            let (capped, total, was_capped) = cap_lines(&tc.full_output, MAX_DETAIL_LINES);
            render_code_block(ui, detect_output_lang(&tc.full_output), &capped, theme);
            if was_capped {
                ui.label(
                    egui::RichText::new(format!(
                        "… {}: {}/{} {}",
                        locale.t("Truncated"),
                        MAX_DETAIL_LINES,
                        total,
                        locale.t("lines")
                    ))
                    .size(theme.text_xs)
                    .color(theme.text_dim),
                );
            }
        });
    });
}

/// Render a syntax-highlighted code block via the existing syntect pipeline.
fn render_code_block(ui: &mut egui::Ui, lang: &str, code: &str, theme: &Theme) {
    let lines = crate::ui::syntax_highlight::highlight_code(lang, code);
    clarity_ui::design_system::code_frame(ui, |ui| {
        ui.set_max_width(ui.available_width());
        for line in lines {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                for (color, text) in line {
                    ui.label(
                        egui::RichText::new(text)
                            .monospace()
                            .size(theme.text_sm)
                            .color(color),
                    );
                }
            });
        }
    });
}

/// Max output lines shown in the expanded tool-call detail before capping.
const MAX_DETAIL_LINES: usize = 200;

/// Build the minimal turn-header meta line, e.g. `"1.2s · 3 tools"`.
/// Unknown pieces (0 values) are omitted; empty when nothing is known.
fn turn_header_summary(header: &TurnHeader, locale: Locale) -> String {
    let mut parts = Vec::new();
    if header.duration_ms > 0 {
        parts.push(format_duration_ms(header.duration_ms));
    }
    if header.tool_count > 0 {
        parts.push(format!("{} {}", header.tool_count, locale.t("tools")));
    }
    if header.token_count > 0 {
        parts.push(format!("{} {}", header.token_count, locale.t("tokens")));
    }
    parts.join(" · ")
}

/// Format a millisecond duration compactly (`"350ms"`, `"1.2s"`, `"2m 05s"`).
fn format_duration_ms(ms: u64) -> String {
    if ms < 1000 {
        format!("{ms}ms")
    } else if ms < 60_000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{}m {:02}s", ms / 60_000, (ms % 60_000) / 1000)
    }
}

/// Pretty-print a JSON payload; returns the raw string when it is not JSON.
fn pretty_json(raw: &str) -> String {
    serde_json::from_str::<serde_json::Value>(raw)
        .and_then(|v| serde_json::to_string_pretty(&v))
        .unwrap_or_else(|_| raw.to_string())
}

/// Cap `text` at `max_lines`; returns the displayed text, the total line
/// count, and whether truncation happened.
fn cap_lines(text: &str, max_lines: usize) -> (String, usize, bool) {
    let total = text.lines().count();
    if total <= max_lines {
        (text.to_string(), total, false)
    } else {
        (
            text.lines().take(max_lines).collect::<Vec<_>>().join("\n"),
            total,
            true,
        )
    }
}

/// Best-effort language guess for a tool output block: JSON when the trimmed
/// output parses as JSON, plain text otherwise.
fn detect_output_lang(output: &str) -> &'static str {
    let trimmed = output.trim_start();
    if (trimmed.starts_with('{') || trimmed.starts_with('['))
        && serde_json::from_str::<serde_json::Value>(trimmed).is_ok()
    {
        "json"
    } else {
        ""
    }
}

fn status_color(status: ToolCallStatus, theme: &Theme) -> egui::Color32 {
    match status {
        ToolCallStatus::Running => theme.status_busy,
        ToolCallStatus::Success => theme.ok,
        ToolCallStatus::Warning => theme.warn,
        ToolCallStatus::Error => theme.danger,
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_duration_ms_compact() {
        assert_eq!(format_duration_ms(0), "0ms");
        assert_eq!(format_duration_ms(350), "350ms");
        assert_eq!(format_duration_ms(999), "999ms");
        assert_eq!(format_duration_ms(1000), "1.0s");
        assert_eq!(format_duration_ms(1234), "1.2s");
        assert_eq!(format_duration_ms(59_999), "60.0s");
        assert_eq!(format_duration_ms(60_000), "1m 00s");
        assert_eq!(format_duration_ms(125_000), "2m 05s");
    }

    #[test]
    fn turn_header_summary_omits_unknown_pieces() {
        let header = TurnHeader {
            duration_ms: 0,
            token_count: 0,
            tool_count: 0,
        };
        assert_eq!(turn_header_summary(&header, Locale::EnUS), "");

        let header = TurnHeader {
            duration_ms: 1200,
            token_count: 0,
            tool_count: 3,
        };
        assert_eq!(turn_header_summary(&header, Locale::EnUS), "1.2s · 3 tools");

        let header = TurnHeader {
            duration_ms: 1200,
            token_count: 800,
            tool_count: 3,
        };
        assert_eq!(
            turn_header_summary(&header, Locale::EnUS),
            "1.2s · 3 tools · 800 tokens"
        );
        // ZhCN: known keys are translated, structure is unchanged.
        let zh = turn_header_summary(&header, Locale::ZhCN);
        assert!(zh.contains("1.2s"), "duration piece kept: {zh}");
        assert!(zh.contains('3'), "tool count kept: {zh}");
    }

    #[test]
    fn pretty_json_formats_valid_and_passes_through_invalid() {
        assert_eq!(pretty_json("{\"a\":1}"), "{\n  \"a\": 1\n}");
        assert_eq!(pretty_json("not json"), "not json");
        assert_eq!(pretty_json(""), "");
    }

    #[test]
    fn cap_lines_under_limit_is_identity() {
        let (text, total, capped) = cap_lines("a\nb\nc", 200);
        assert_eq!(text, "a\nb\nc");
        assert_eq!(total, 3);
        assert!(!capped);
    }

    #[test]
    fn cap_lines_over_limit_truncates_and_reports_total() {
        let input = (0..250)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let (text, total, capped) = cap_lines(&input, 200);
        assert_eq!(total, 250);
        assert!(capped);
        assert_eq!(text.lines().count(), 200);
        assert!(text.ends_with("line 199"));
    }

    #[test]
    fn detect_output_lang_json_vs_plain() {
        assert_eq!(detect_output_lang("{\"k\": 1}"), "json");
        assert_eq!(detect_output_lang("  [1, 2]"), "json");
        assert_eq!(detect_output_lang("{broken"), "");
        assert_eq!(detect_output_lang("plain log output"), "");
        assert_eq!(detect_output_lang(""), "");
    }
}
