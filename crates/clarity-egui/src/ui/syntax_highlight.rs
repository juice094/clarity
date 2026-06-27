//! Syntax highlighting for code blocks via syntect.
//!
//! Runs on the cold path (parse phase) so the hot path only iterates
//! pre-computed `(color, text)` spans. Uses One Half Dark as the
//! base theme, mapped to egui `Color32`.

use once_cell::sync::Lazy;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::SyntaxSet;

/// Lazily-initialized syntax definitions (embedded, no filesystem I/O).
static SYNTAX_SET: Lazy<SyntaxSet> = Lazy::new(SyntaxSet::load_defaults_newlines);

/// Lazily-initialized theme (One Half Dark, mapped to Claude-like palette).
static THEME: Lazy<Theme> = Lazy::new(|| {
    let ts = ThemeSet::load_defaults();
    ts.themes
        .get("base16-ocean.dark")
        .cloned()
        .unwrap_or_else(|| ts.themes["InspiredGitHub"].clone())
});

/// Map common short language identifiers to syntect token names.
fn normalize_lang(lang: &str) -> &str {
    let lower = lang.to_lowercase();
    match lower.as_str() {
        "rs" | "rust" => "Rust",
        "py" | "python" => "Python",
        "js" | "javascript" => "JavaScript",
        "ts" | "typescript" => "TypeScript",
        "json" => "JSON",
        "html" => "HTML",
        "css" => "CSS",
        "sh" | "bash" | "shell" => "Bash",
        "go" | "golang" => "Go",
        "java" => "Java",
        "md" | "markdown" => "Markdown",
        "yaml" | "yml" => "YAML",
        "toml" => "TOML",
        "c" => "C",
        "cpp" | "c++" => "C++",
        "sql" => "SQL",
        "rb" | "ruby" => "Ruby",
        _ => lang,
    }
}

/// Highlight a block of code and return per-line styled spans.
///
/// Each line is a `Vec<(egui::Color32, String)>` where each pair is a
/// token color and its text content. Lines without styling are returned
/// as single-element vectors with the default text color.
pub fn highlight_code(lang: &str, code: &str) -> Vec<Vec<(egui::Color32, String)>> {
    let normalized = normalize_lang(lang);
    let syntax = SYNTAX_SET
        .find_syntax_by_token(normalized)
        .or_else(|| SYNTAX_SET.find_syntax_by_extension(normalized))
        .unwrap_or_else(|| SYNTAX_SET.find_syntax_plain_text());

    let mut highlighter = syntect::easy::HighlightLines::new(syntax, &THEME);
    let mut result = Vec::new();

    for line in code.lines() {
        let ranges: Vec<(syntect::highlighting::Style, &str)> =
            match highlighter.highlight_line(line, &SYNTAX_SET) {
                Ok(r) => r,
                Err(_) => {
                    result.push(vec![(egui::Color32::from_gray(210), line.to_string())]);
                    continue;
                }
            };

        if ranges.is_empty() {
            result.push(vec![(egui::Color32::from_gray(210), line.to_string())]);
            continue;
        }

        let styled: Vec<(egui::Color32, String)> = ranges
            .into_iter()
            .map(|(style, text)| (syntect_color_to_egui(style.foreground), text.to_string()))
            .collect();
        result.push(styled);
    }

    result
}

/// Map a syntect `Color` to an `egui::Color32`.
fn syntect_color_to_egui(c: syntect::highlighting::Color) -> egui::Color32 {
    egui::Color32::from_rgb(c.r, c.g, c.b)
}

/// Try to highlight code; if the language is unknown or parsing fails,
/// returns plain monochrome lines (no syntax coloring).
pub fn try_highlight(lang: &str, code: &str) -> Vec<Vec<(egui::Color32, String)>> {
    if lang.is_empty() || code.is_empty() {
        return code
            .lines()
            .map(|l| vec![(egui::Color32::from_gray(210), l.to_string())])
            .collect();
    }
    highlight_code(lang, code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlight_rust_code() {
        let code = "fn main() {\n    println!(\"hello\");\n}";
        let lines = highlight_code("rust", code);
        assert!(!lines.is_empty(), "Should produce at least one line");
        let total_tokens: usize = lines.iter().map(|l| l.len()).sum();
        assert!(total_tokens > 0, "Should produce at least some tokens");
    }

    #[test]
    fn try_highlight_empty_lang() {
        let lines = try_highlight("", "hello world");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].len(), 1);
    }

    #[test]
    fn unknown_lang_uses_plain_text() {
        let lines = highlight_code("zzzunknownzzz", "some code");
        assert!(!lines.is_empty());
    }
}
