//! Markdown cold-path parser + hot-path renderer.
//!
//! ARCHITECTURE CONSTRAINT (Pretext-aligned):
//!   - `parse_markdown()` is the ONLY place where string parsing happens.
//!   - `render_blocks()` is the hot path: it ONLY iterates pre-parsed blocks.
//!   - NEVER parse `Message::content` inside the hot path.
//!
//! See `crates/clarity-egui/ARCHITECTURE.md` §1.1, §2.2.

use crate::theme::Theme;
use crate::ui::types::{InlineSpan, RenderBlock};

// ============================================================================
// Markdown Parser — Cold path (called once when Message content changes)
// ============================================================================

/// Parse markdown.
pub fn parse_markdown(text: &str) -> Vec<RenderBlock> {
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();
    let mut in_code_block = false;
    let mut code_buffer = String::new();
    let mut code_lang = String::new();
    let mut paragraph_lines: Vec<&str> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start();

        // Code block fence
        if trimmed.starts_with("```") {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            if in_code_block {
                // Trim trailing newline that we added per line
                if code_buffer.ends_with('\n') {
                    code_buffer.pop();
                }
                blocks.push(RenderBlock::CodeBlock {
                    lang: std::mem::take(&mut code_lang),
                    code: std::mem::take(&mut code_buffer),
                });
                in_code_block = false;
            } else {
                in_code_block = true;
                code_lang = trimmed.strip_prefix("```").unwrap_or("").trim().to_string();
            }
            i += 1;
            continue;
        }

        if in_code_block {
            code_buffer.push_str(line);
            code_buffer.push('\n');
            i += 1;
            continue;
        }

        // Empty line → flush paragraph
        if trimmed.is_empty() {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            i += 1;
            continue;
        }

        // Table detection: consecutive lines starting with '|'
        if trimmed.starts_with('|') {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            let table_end = scan_table(&lines, i);
            if let Some((headers, rows, _end_idx)) = parse_table_lines(&lines[i..table_end]) {
                blocks.push(RenderBlock::Table { headers, rows });
                i = table_end;
                continue;
            }
            // Not a valid table → fall through to paragraph
        }

        // Headings — match longest prefix first to avoid false matches.
        if let Some(rest) = trimmed.strip_prefix("##### ") {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            blocks.push(RenderBlock::Heading(5, parse_inline(rest)));
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("#### ") {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            blocks.push(RenderBlock::Heading(4, parse_inline(rest)));
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("### ") {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            blocks.push(RenderBlock::Heading(3, parse_inline(rest)));
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("## ") {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            blocks.push(RenderBlock::Heading(2, parse_inline(rest)));
            i += 1;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("# ") {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            blocks.push(RenderBlock::Heading(1, parse_inline(rest)));
            i += 1;
            continue;
        }

        // Unordered list
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            blocks.push(RenderBlock::ListItem(parse_inline(&trimmed[2..])));
            i += 1;
            continue;
        }

        // Ordered list
        let digits_end = trimmed.find(|c: char| !c.is_ascii_digit()).unwrap_or(0);
        if digits_end > 0 && trimmed[digits_end..].starts_with(". ") {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            blocks.push(RenderBlock::ListItem(parse_inline(
                &trimmed[digits_end + 2..],
            )));
            i += 1;
            continue;
        }

        // Blockquote
        if let Some(rest) = trimmed.strip_prefix("> ") {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            blocks.push(RenderBlock::Blockquote(parse_inline(rest)));
            i += 1;
            continue;
        }

        // Horizontal rule
        if trimmed.chars().all(|c| c == '-' || c == '*' || c == '_') && trimmed.len() >= 3 {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            blocks.push(RenderBlock::HorizontalRule);
            i += 1;
            continue;
        }

        // Unified diff detection: --- path +++ path then @@ hunk headers.
        if trimmed.starts_with("--- ") {
            flush_paragraph(&mut paragraph_lines, &mut blocks);
            let file_path = trimmed
                .strip_prefix("--- a/")
                .or_else(|| trimmed.strip_prefix("--- b/"))
                .or_else(|| trimmed.strip_prefix("--- "))
                .map(|s| s.to_string())
                .filter(|s| s != "/dev/null");
            if let Some((hunks, consumed)) = try_parse_unified_diff(&lines, i) {
                blocks.push(RenderBlock::Diff { hunks, file_path });
                i += consumed;
                continue;
            }
        }

        // Regular paragraph line
        paragraph_lines.push(line);
        i += 1;
    }

    flush_paragraph(&mut paragraph_lines, &mut blocks);

    if in_code_block && !code_buffer.is_empty() {
        if code_buffer.ends_with('\n') {
            code_buffer.pop();
        }
        blocks.push(RenderBlock::CodeBlock {
            lang: code_lang,
            code: code_buffer,
        });
    }

    blocks
}

/// Scan forward to find the end of a potential table block.
fn scan_table(lines: &[&str], start: usize) -> usize {
    let mut end = start;
    while end < lines.len() {
        let trimmed = lines[end].trim_start();
        if trimmed.starts_with('|') {
            end += 1;
        } else if trimmed.is_empty() {
            end += 1;
            // Allow a single empty line inside table? No, stop at empty line.
            break;
        } else {
            break;
        }
    }
    end
}

/// Parse table lines into headers and rows.
/// Returns (headers, rows, consumed_count) if valid.
fn parse_table_lines(lines: &[&str]) -> Option<(Vec<String>, Vec<Vec<String>>, usize)> {
    if lines.len() < 2 {
        return None;
    }
    let first = lines[0].trim();
    let second = lines[1].trim();

    // First line must be header row
    let headers = parse_table_row(first);
    if headers.is_empty() {
        return None;
    }

    // Second line must be separator row (contains only |, -, :, spaces)
    let is_separator = second.starts_with('|')
        && second.ends_with('|')
        && second
            .chars()
            .all(|c| c == '|' || c == '-' || c == ':' || c.is_whitespace());
    if !is_separator {
        return None;
    }

    let col_count = headers.len();
    let mut rows = Vec::new();
    let mut consumed = 2;

    for line in &lines[2..] {
        let trimmed = line.trim();
        if !trimmed.starts_with('|') {
            break;
        }
        let cells = parse_table_row(trimmed);
        if cells.is_empty() {
            break;
        }
        // Pad or truncate to match header column count
        let mut row = cells;
        while row.len() < col_count {
            row.push(String::new());
        }
        row.truncate(col_count);
        rows.push(row);
        consumed += 1;
    }

    Some((headers, rows, consumed))
}

fn parse_table_row(line: &str) -> Vec<String> {
    let mut cells = Vec::new();
    let trimmed = line.trim();
    if !trimmed.starts_with('|') {
        return cells;
    }
    let inner = &trimmed[1..];
    // Split by '|' and trim each cell
    for cell in inner.split('|') {
        let c = cell.trim().to_string();
        // Skip trailing empty cell after last '|'
        if c.is_empty() && cell == inner.split('|').next_back().unwrap_or("") {
            continue;
        }
        cells.push(c);
    }
    cells
}

fn flush_paragraph(lines: &mut Vec<&str>, blocks: &mut Vec<RenderBlock>) {
    if lines.is_empty() {
        return;
    }
    let text = lines.join("\n");
    blocks.push(RenderBlock::Paragraph(parse_inline(&text)));
    lines.clear();
}

fn parse_inline(text: &str) -> Vec<InlineSpan> {
    let mut spans = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        if let Some(pos) = rest.find("**") {
            if pos > 0 {
                spans.push(InlineSpan::Text(rest[..pos].to_string()));
            }
            rest = &rest[pos + 2..];
            if let Some(end) = rest.find("**") {
                spans.push(InlineSpan::Bold(rest[..end].to_string()));
                rest = &rest[end + 2..];
            } else {
                spans.push(InlineSpan::Bold(rest.to_string()));
                break;
            }
        } else if let Some(pos) = rest.find('`') {
            if pos > 0 {
                spans.push(InlineSpan::Text(rest[..pos].to_string()));
            }
            rest = &rest[pos + 1..];
            if let Some(end) = rest.find('`') {
                spans.push(InlineSpan::Code(rest[..end].to_string()));
                rest = &rest[end + 1..];
            } else {
                spans.push(InlineSpan::Code(rest.to_string()));
                break;
            }
        } else if let Some(pos) = rest.find('[') {
            if pos > 0 {
                spans.push(InlineSpan::Text(rest[..pos].to_string()));
            }
            rest = &rest[pos + 1..];
            if let Some(end_bracket) = rest.find("](") {
                let link_text = &rest[..end_bracket];
                rest = &rest[end_bracket + 2..];
                if let Some(end_paren) = rest.find(')') {
                    let url = &rest[..end_paren];
                    spans.push(InlineSpan::Link {
                        text: link_text.to_string(),
                        url: url.to_string(),
                    });
                    rest = &rest[end_paren + 1..];
                } else {
                    spans.push(InlineSpan::Text(rest.to_string()));
                    break;
                }
            } else {
                spans.push(InlineSpan::Text(rest.to_string()));
                break;
            }
        } else {
            spans.push(InlineSpan::Text(rest.to_string()));
            break;
        }
    }
    spans
}

/// Try to parse a unified diff starting at `lines[i]`.
/// Returns parsed hunks and number of lines consumed if valid.
fn try_parse_unified_diff(
    lines: &[&str],
    start: usize,
) -> Option<(Vec<clarity_core::diff::DiffHunk>, usize)> {
    if start + 2 >= lines.len() {
        return None;
    }
    let first = lines[start].trim();
    let second = lines.get(start + 1)?.trim();
    // Must start with --- (a/ or /dev/null or bare path after space)
    if !first.starts_with("--- ") {
        return None;
    }
    // Followed by +++ (b/ or /dev/null or bare path after space)
    if !second.starts_with("+++ ") {
        return None;
    }
    let mut end = start + 2;
    while end < lines.len() {
        let line = lines[end].trim();
        if line.starts_with("@@")
            || line.starts_with('+')
            || line.starts_with('-')
            || line.starts_with(' ')
            || line.is_empty()
        {
            end += 1;
        } else {
            break;
        }
    }
    let consumed = end - start;
    let patch: String = lines[start..end].join("\n");
    let hunks = clarity_core::diff::parse_unified_diff(&patch);
    if hunks.is_empty() {
        return None;
    }
    Some((hunks, consumed))
}

// ============================================================================
// Layout — Hot path (called every frame, only iterates pre-parsed blocks)
// ============================================================================

/// Renders the blocks UI.
pub fn render_blocks(
    ui: &mut egui::Ui,
    blocks: &[RenderBlock],
    theme: &Theme,
    text_color: egui::Color32,
) {
    for (i, block) in blocks.iter().enumerate() {
        if i > 0 {
            ui.add_space(theme.space_4);
        }
        match block {
            RenderBlock::Paragraph(spans) => {
                render_spans(ui, spans, theme, text_color, theme.text_base, false);
            }
            RenderBlock::Heading(level, spans) => {
                // Map heading levels to the theme typography scale.
                // H1=2xl(36px), H2=xl(22px), H3=lg(18px), H4=md(15px), H5=base(14px)
                let size = match level {
                    1 => theme.text_2xl,
                    2 => theme.text_xl,
                    3 => theme.text_lg,
                    4 => theme.text_md,
                    _ => theme.text_base,
                };
                render_spans(ui, spans, theme, text_color, size, true);
            }
            RenderBlock::CodeBlock { lang, code } => {
                render_code_block(ui, lang, code, theme);
            }
            RenderBlock::ListItem(spans) => {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("•")
                            .size(theme.text_base)
                            .color(text_color),
                    );
                    render_spans(ui, spans, theme, text_color, theme.text_base, false);
                });
            }
            RenderBlock::Blockquote(spans) => {
                ui.horizontal(|ui| {
                    let (rect, _) =
                        ui.allocate_exact_size(egui::vec2(3.0, 16.0), egui::Sense::hover());
                    ui.painter()
                        .rect_filled(rect, egui::CornerRadius::same(2), theme.accent);
                    ui.add_space(theme.space_8);
                    render_spans(ui, spans, theme, theme.text_muted, theme.text_base, false);
                });
            }
            RenderBlock::HorizontalRule => {
                ui.add_space(theme.space_4);
                // TUI-style separator with box-drawing characters.
                let w = ui.available_width();
                let dash = "\u{2500}";
                let count = (w / 10.0) as usize;
                ui.add(egui::Label::new(
                    egui::RichText::new(dash.repeat(count.max(1)))
                        .size(theme.text_xs)
                        .color(theme.border)
                        .monospace(),
                ));
                ui.add_space(theme.space_4);
            }
            RenderBlock::Table { headers, rows } => {
                render_table(ui, i, headers, rows, theme, text_color);
            }
            RenderBlock::Diff { hunks, file_path } => {
                ui.add_space(theme.space_4);
                if let Some(path) = file_path {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "{} {}",
                                crate::theme::ICON_FILE_CODE,
                                path
                            ))
                            .size(theme.text_xs)
                            .color(theme.accent)
                            .monospace(),
                        );
                    });
                    ui.add_space(theme.space_4);
                }
                let config = crate::widgets::diff_viewer::DiffViewConfig {
                    show_file_header: true,
                    show_line_numbers: true,
                    collapse_unchanged: true,
                    collapse_threshold: 6,
                    compact: false,
                    max_height: None,
                    show_actions: false,
                    side_by_side: false,
                };
                crate::widgets::diff_viewer::render_diff_view(ui, hunks, theme, &config);
                ui.add_space(theme.space_4);
            }
        }
    }
}

fn render_table(
    ui: &mut egui::Ui,
    idx: usize,
    headers: &[String],
    rows: &[Vec<String>],
    theme: &Theme,
    text_color: egui::Color32,
) {
    if headers.is_empty() {
        return;
    }
    ui.add_space(theme.space_4);
    egui::Frame::new()
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .inner_margin(egui::Margin::same(8))
        .show(ui, |ui| {
            // Namespace the grid id to avoid collisions with other grids that may
            // share the same parent id and numeric index.
            egui::Grid::new(ui.id().with(("md_table", idx)))
                .spacing([12.0, 6.0])
                .show(ui, |ui| {
                    // Header row
                    for h in headers {
                        ui.label(
                            egui::RichText::new(h)
                                .size(theme.text_sm)
                                .strong()
                                .color(text_color),
                        );
                    }
                    ui.end_row();

                    // Separator line
                    let available = ui.available_width();
                    let row_y = ui.cursor().min.y;
                    ui.painter().line_segment(
                        [
                            egui::pos2(ui.cursor().min.x, row_y),
                            egui::pos2(ui.cursor().min.x + available, row_y),
                        ],
                        egui::Stroke::new(1.0_f32, theme.border),
                    );
                    ui.end_row();

                    // Data rows
                    for row in rows {
                        for cell in row {
                            ui.label(
                                egui::RichText::new(cell)
                                    .size(theme.text_sm)
                                    .color(theme.text_muted),
                            );
                        }
                        ui.end_row();
                    }
                });
        });
    ui.add_space(theme.space_4);
}

fn render_spans(
    ui: &mut egui::Ui,
    spans: &[InlineSpan],
    theme: &Theme,
    base_color: egui::Color32,
    size: f32,
    strong: bool,
) {
    ui.horizontal_wrapped(|ui| {
        for span in spans {
            render_span(ui, span, theme, base_color, size, strong);
        }
    });
}

fn render_span(
    ui: &mut egui::Ui,
    span: &InlineSpan,
    theme: &Theme,
    base_color: egui::Color32,
    size: f32,
    strong: bool,
) {
    match span {
        InlineSpan::Text(text) => {
            // Detect file paths and render as clickable links.
            if looks_like_file_path(text) {
                let label = format!("{} {}", crate::theme::ICON_FILE_CODE, text);
                let resp = ui.add(
                    egui::Label::new(
                        egui::RichText::new(label)
                            .color(theme.accent)
                            .size(size)
                            .monospace(),
                    )
                    .sense(egui::Sense::click()),
                );
                if resp.clicked() {
                    // Open in system editor on primary click, copy path on secondary.
                    let path_str = text.trim().to_string();
                    let file_url = format!("file:///{}", path_str.replace('\\', "/"));
                    let _ = webbrowser::open(&file_url);
                }
                if resp.secondary_clicked() {
                    ui.ctx().copy_text(text.to_string());
                }
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                resp.on_hover_text(format!("Open {} — Right-click to copy path", text));
            } else {
                let mut rt = egui::RichText::new(text).color(base_color).size(size);
                if strong {
                    rt = rt.strong();
                }
                ui.label(rt);
            }
        }
        InlineSpan::Bold(text) => {
            ui.label(
                egui::RichText::new(text)
                    .color(base_color)
                    .size(size)
                    .strong(),
            );
        }
        InlineSpan::Code(text) => {
            ui.label(
                egui::RichText::new(text)
                    .monospace()
                    .color(theme.text_strong)
                    .background_color(theme.code_block_bg)
                    .size(theme.text_base),
            );
        }
        InlineSpan::Link { text, url } => {
            ui.hyperlink_to(
                egui::RichText::new(text).color(theme.accent).size(size),
                url,
            );
        }
    }
}

/// Detect strings that look like file paths with optional line numbers.
fn looks_like_file_path(text: &str) -> bool {
    let s = text.trim();
    if s.len() < 4 || s.len() > 200 {
        return false;
    }
    // Path separators, common extensions, or line-number suffix.
    let has_separator = s.contains('/') || s.contains('\\');
    let has_ext = s.contains(".rs")
        || s.contains(".py")
        || s.contains(".js")
        || s.contains(".ts")
        || s.contains(".go")
        || s.contains(".java")
        || s.contains(".c")
        || s.contains(".cpp")
        || s.contains(".h")
        || s.contains(".json")
        || s.contains(".yaml")
        || s.contains(".toml")
        || s.contains(".md")
        || s.contains(".txt")
        || s.contains(".css")
        || s.contains(".html");
    let has_lineno = s.contains(':')
        && s.split(':')
            .last()
            .map_or(false, |p| p.parse::<usize>().is_ok());
    has_separator || has_ext || has_lineno
}

fn render_code_block(ui: &mut egui::Ui, lang: &str, code: &str, theme: &Theme) {
    ui.add_space(theme.space_4);
    // Calibrate line height from the monospace font metrics so inter-line
    // spacing tracks font scale, DPI, and font family changes.
    let code_line_h = theme.line_height_mono_at(ui.ctx(), theme.text_base);

    // Split into lines for line-number rendering.
    let lines: Vec<&str> = code.lines().collect();
    let line_count = lines.len();
    let collapse_threshold: usize = 30;
    // Unique key per code block (hash of content prevents cross-block state sharing).
    let collapse_key = ui.id().with(("code_collapse", code));
    let collapsed: bool = line_count > collapse_threshold
        && ui.ctx().data(|d| d.get_temp(collapse_key).unwrap_or(true));
    let visible_lines: usize = if collapsed {
        collapse_threshold.min(lines.len())
    } else {
        lines.len()
    };
    let ln_width = if visible_lines > 1 {
        // Right-aligned line number gutter.
        let digits = line_count.to_string().len().max(1);
        (digits as f32) * 9.0 + theme.space_12
    } else {
        0.0
    };

    egui::Frame::new()
        .fill(theme.code_block_bg)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .shadow(theme.shadow_card)
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());

            // ── Header: lang badge + copy button ──
            ui.horizontal(|ui| {
                if !lang.is_empty() {
                    let badge = egui::Frame::new()
                        .fill(theme.bg_hover)
                        .corner_radius(egui::CornerRadius::same(4))
                        .inner_margin(egui::Margin::symmetric(8, 2))
                        .show(ui, |ui| {
                            ui.label(
                                egui::RichText::new(lang)
                                    .size(theme.text_xs)
                                    .color(theme.accent)
                                    .monospace(),
                            );
                        });
                    let _ = badge; // Frame response consumed.
                }
                ui.add_space(theme.space_4);
                if !lang.is_empty() {
                    ui.label(
                        egui::RichText::new(format!("{} lines", line_count))
                            .size(theme.text_xs)
                            .color(theme.text_dim),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Copy button with "Copied!" feedback
                    let copy_key = ui.id().with(("copy_feedback", code));
                    let copied_at: Option<f64> = ui.ctx().data(|d| d.get_temp(copy_key));
                    let just_copied = copied_at
                        .map(|t| ui.ctx().input(|i| i.time) - t < 1.5)
                        .unwrap_or(false);
                    let copy_label = if just_copied {
                        format!("{} Copied!", crate::theme::ICON_CHECK)
                    } else {
                        format!("{} Copy", crate::theme::ICON_COPY)
                    };
                    let copy_color = if just_copied {
                        theme.ok
                    } else {
                        theme.text_dim
                    };
                    if ui
                        .add_sized(
                            [if just_copied { 72.0 } else { 64.0 }, 22.0],
                            egui::Button::new(
                                egui::RichText::new(copy_label)
                                    .size(theme.text_xs)
                                    .color(copy_color),
                            )
                            .fill(theme.surface)
                            .corner_radius(egui::CornerRadius::same(4)),
                        )
                        .clicked()
                    {
                        ui.ctx().copy_text(code.to_string());
                        ui.ctx()
                            .data_mut(|d| d.insert_temp(copy_key, ui.ctx().input(|i| i.time)));
                        ui.ctx().request_repaint();
                    }
                    // Run button for shell/python code
                    let runnable = matches!(
                        lang.to_lowercase().as_str(),
                        "sh" | "bash" | "shell" | "py" | "python"
                    );
                    if runnable {
                        ui.add_space(4.0);
                        if ui
                            .add_sized(
                                [56.0, 22.0],
                                egui::Button::new(
                                    egui::RichText::new(format!(
                                        "{} Run",
                                        crate::theme::ICON_TERMINAL,
                                    ))
                                    .size(theme.text_xs)
                                    .color(theme.accent),
                                )
                                .fill(theme.surface)
                                .corner_radius(egui::CornerRadius::same(4)),
                            )
                            .clicked()
                        {
                            // Copy code to clipboard so user can paste into terminal.
                            ui.ctx().copy_text(code.to_string());
                        }
                    }
                });
            });
            ui.add_space(theme.space_4);

            // ── Code lines with syntax highlighting ──
            let highlighted = crate::ui::syntax_highlight::try_highlight(lang, code);
            let digits = line_count.to_string().len().max(1);
            let show_collapse = line_count > collapse_threshold;

            if visible_lines > 1 {
                for (idx, styled_tokens) in highlighted.iter().enumerate() {
                    if collapsed && idx >= collapse_threshold {
                        break;
                    }
                    let ln = idx + 1;
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = theme.space_8;
                        // Line number gutter.
                        ui.add_sized(
                            [ln_width, theme.text_base],
                            egui::Label::new(
                                egui::RichText::new(format!("{:>width$}", ln, width = digits))
                                    .size(theme.text_xs)
                                    .color(theme.text_dim)
                                    .monospace(),
                            ),
                        );
                        // Code line content with per-token syntax colors.
                        if styled_tokens.is_empty() || styled_tokens[0].1.is_empty() {
                            ui.label(
                                egui::RichText::new(" ")
                                    .monospace()
                                    .color(theme.text)
                                    .size(theme.text_base)
                                    .line_height(Some(code_line_h)),
                            );
                        } else {
                            for (color, text) in styled_tokens {
                                ui.label(
                                    egui::RichText::new(text.as_str())
                                        .monospace()
                                        .color(*color)
                                        .size(theme.text_base)
                                        .line_height(Some(code_line_h)),
                                );
                            }
                        }
                    });
                }
                // Show expand button for collapsed large blocks.
                if show_collapse && collapsed {
                    ui.add_space(theme.space_4);
                    let remaining = line_count - collapse_threshold;
                    if ui
                        .add_sized(
                            [ui.available_width(), 24.0],
                            egui::Button::new(
                                egui::RichText::new(format!(
                                    "{} Show all {} lines  {}",
                                    crate::theme::ICON_CARET_DOWN,
                                    line_count,
                                    crate::theme::ICON_CARET_DOWN,
                                ))
                                .size(theme.text_xs)
                                .color(theme.text_dim),
                            )
                            .fill(theme.bg_hover),
                        )
                        .clicked()
                    {
                        ui.ctx().data_mut(|d| d.insert_temp(collapse_key, false));
                        ui.ctx().request_repaint();
                    }
                    // prevent unused variable warning
                    let _ = remaining;
                }
            } else {
                // Single line with syntax highlighting.
                if let Some(tokens) = highlighted.first() {
                    ui.horizontal(|ui| {
                        for (color, text) in tokens {
                            ui.label(
                                egui::RichText::new(text.as_str())
                                    .monospace()
                                    .color(*color)
                                    .size(theme.text_base)
                                    .line_height(Some(code_line_h)),
                            );
                        }
                    });
                } else {
                    ui.label(
                        egui::RichText::new(code)
                            .monospace()
                            .color(theme.text)
                            .size(theme.text_base)
                            .line_height(Some(code_line_h)),
                    );
                }
            }
        });
    ui.add_space(theme.space_4);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_diff_in_markdown() {
        let md = "--- a/src/main.rs\n+++ b/src/main.rs\n@@ -1,2 +1,2 @@\n-old\n+new\n";
        let blocks = parse_markdown(md);
        let has_diff = blocks.iter().any(|b| matches!(b, RenderBlock::Diff { .. }));
        assert!(has_diff, "Should detect unified diff in markdown");
    }

    #[test]
    fn looks_like_path_detection() {
        assert!(looks_like_file_path("src/main.rs"));
        assert!(looks_like_file_path("/absolute/path/file.py"));
        assert!(!looks_like_file_path("hello world"));
    }
}
