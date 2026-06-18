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

        // Headings
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
                let size = match level {
                    1 => theme.text_lg,
                    2 => theme.text_md,
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
                ui.separator();
                ui.add_space(theme.space_4);
            }
            RenderBlock::Table { headers, rows } => {
                render_table(ui, i, headers, rows, theme, text_color);
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
            let mut rt = egui::RichText::new(text).color(base_color).size(size);
            if strong {
                rt = rt.strong();
            }
            ui.label(rt);
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

fn render_code_block(ui: &mut egui::Ui, lang: &str, code: &str, theme: &Theme) {
    ui.add_space(theme.space_4);
    egui::Frame::new()
        .fill(theme.code_block_bg)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .inner_margin(egui::Margin::symmetric(14, 12))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                if !lang.is_empty() {
                    ui.label(
                        egui::RichText::new(lang)
                            .size(theme.text_sm)
                            .color(theme.text_dim)
                            .monospace(),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Copy")
                                    .size(theme.text_xs)
                                    .color(theme.text_dim),
                            )
                            .frame(false),
                        )
                        .clicked()
                    {
                        ui.ctx().copy_text(code.to_string());
                    }
                });
            });
            ui.add_space(2.0);
            // Immutable monospace label — no per-frame String allocation
            ui.label(
                egui::RichText::new(code)
                    .monospace()
                    .color(theme.text)
                    .size(theme.text_base)
                    .line_height(Some(22.0)),
            );
        });
    ui.add_space(theme.space_4);
}
