//! Markdown cold-path parser + hot-path renderer.
//!
//! The hot-path renderer now delegates to [`egui_commonmark`]. `parse_markdown`
//! and [`RenderBlock`] are kept as a compatibility shim for height estimation
//! and layout decisions.
//!
//! See `crates/clarity-egui/ARCHITECTURE.md` §1.1, §2.2.

use crate::ui::types::{InlineSpan, RenderBlock};
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
use parking_lot::Mutex;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

// ============================================================================
// CommonMark cache — survives across frames, but bounded to avoid unbounded
// growth. When the accumulated rendered-text budget is exceeded the cache is
// dropped and rebuilt. ponytail: a per-session cache would be more precise,
// but this bounds memory without changing every call site.
// ============================================================================

static COMMONMARK_CACHE: OnceLock<Mutex<CommonMarkCache>> = OnceLock::new();
static COMMONMARK_BYTES: AtomicUsize = AtomicUsize::new(0);
/// Rough budget for rendered markdown text kept alive in the cache. Images and
// syntax-highlighted code can push actual memory higher, so this is a text-byte cap.
const COMMONMARK_BUDGET: usize = 8 * 1024 * 1024;

fn with_commonmark_cache<R>(f: impl FnOnce(&mut CommonMarkCache) -> R) -> R {
    let cache = COMMONMARK_CACHE.get_or_init(|| Mutex::new(CommonMarkCache::default()));
    f(&mut cache.lock())
}

/// Render raw Markdown text via [`egui_commonmark`].
pub fn render_markdown(ui: &mut egui::Ui, text: &str, text_color: egui::Color32) {
    // Scope the override so it does not leak to sibling widgets.
    let text_len = text.len();
    ui.scope(|ui| {
        ui.visuals_mut().override_text_color = Some(text_color);
        with_commonmark_cache(|cache| {
            CommonMarkViewer::new().show(ui, cache, text);
            let prev = COMMONMARK_BYTES.fetch_add(text_len, Ordering::Relaxed);
            if prev + text_len > COMMONMARK_BUDGET {
                *cache = CommonMarkCache::default();
                COMMONMARK_BYTES.store(0, Ordering::Relaxed);
            }
        });
    });
}

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

/// Detect strings that look like file paths with optional line numbers.
#[allow(dead_code)]
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
            .next_back()
            .is_some_and(|p| p.parse::<usize>().is_ok());
    has_separator || has_ext || has_lineno
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
