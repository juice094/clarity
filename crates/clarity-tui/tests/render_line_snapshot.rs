//! Snapshot tests for the RenderLine → ratatui pipeline (S7 Phase 3A).
//!
//! These tests act as cross-renderer parity gates: any change to
//! `markdown_to_lines()` or `render_line_to_ratatui()` must preserve the
//! textual output for the canonical fixtures defined here.
//!
//! GUI parity: the egui frontend uses `render_line_text()` (in `clarity-egui`)
//! to extract equivalent plain text. Both frontends MUST agree on content;
//! only styling may differ.

use clarity_core::ui::markdown_to_lines;
use clarity_tui::render_line::render_line_to_ratatui;
use ratatui::style::Style;

/// Concatenate every span's textual content from a ratatui `Line`.
///
/// This is the canonical "plain text projection" used for parity gates.
fn line_to_plain(line: &ratatui::text::Line<'_>) -> String {
    line.spans.iter().map(|s| s.content.as_ref()).collect()
}

fn render_md(md: &str) -> Vec<String> {
    markdown_to_lines(md)
        .iter()
        .map(|l| line_to_plain(&render_line_to_ratatui(l, Style::default())))
        .collect()
}

#[test]
fn heading_snapshot() {
    let texts = render_md("# Heading One\n## Heading Two");
    assert_eq!(texts.len(), 2);
    assert_eq!(texts[0], "Heading One");
    assert_eq!(texts[1], "Heading Two");
}

#[test]
fn code_block_with_language_tag() {
    let texts = render_md("```rust\nlet x = 1;\nlet y = 2;\n```");
    assert_eq!(texts.len(), 2);
    assert!(texts[0].contains("let x = 1;"), "got: {}", texts[0]);
    assert!(texts[1].contains("let y = 2;"), "got: {}", texts[1]);
    // Language tag is appended for non-empty langs.
    assert!(texts[0].contains("[rust]"));
}

#[test]
fn code_block_without_language() {
    let texts = render_md("```\nplain code\n```");
    assert_eq!(texts.len(), 1);
    assert!(texts[0].contains("plain code"));
    assert!(!texts[0].contains("[]"));
}

#[test]
fn unordered_list_preserves_items() {
    let texts = render_md("- alpha\n- beta\n- gamma");
    assert_eq!(texts.len(), 3);
    assert!(texts[0].ends_with("alpha"));
    assert!(texts[1].ends_with("beta"));
    assert!(texts[2].ends_with("gamma"));
}

#[test]
fn ordered_list_preserves_items() {
    let texts = render_md("1. one\n2. two\n3. three");
    assert_eq!(texts.len(), 3);
    assert!(texts[0].contains("one"));
    assert!(texts[1].contains("two"));
    assert!(texts[2].contains("three"));
}

#[test]
fn blockquote_text_preserved() {
    let texts = render_md("> quoted text");
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0], "quoted text");
}

#[test]
fn horizontal_rule_renders_box_drawing() {
    let texts = render_md("before\n\n---\n\nafter");
    assert!(
        texts.iter().any(|t| t.contains('\u{2500}')),
        "expected horizontal rule via box-drawing char, got: {:?}",
        texts
    );
}

#[test]
fn mixed_document_preserves_structure() {
    let md = "# Title\n\nParagraph with text.\n\n- item 1\n- item 2\n\n```\ncode\n```\n\n> quote";
    let texts = render_md(md);
    assert!(texts.len() >= 6, "got {} lines: {:?}", texts.len(), texts);
    assert_eq!(texts[0], "Title");
    let joined = texts.join("\n");
    assert!(joined.contains("Paragraph"));
    assert!(joined.contains("item 1"));
    assert!(joined.contains("item 2"));
    assert!(joined.contains("code"));
    assert!(joined.contains("quote"));
}

#[test]
fn empty_input_yields_empty_line() {
    let texts = render_md("");
    assert_eq!(texts.len(), 1);
    assert_eq!(texts[0], "");
}

#[test]
fn paragraph_with_inline_bold() {
    let texts = render_md("This is **bold** text.");
    assert_eq!(texts.len(), 1);
    // Spans are concatenated; bold marker is consumed by the parser.
    assert!(texts[0].contains("bold"));
    assert!(texts[0].contains("This is"));
}

#[test]
fn two_paragraphs_separated() {
    let texts = render_md("First para.\n\nSecond para.");
    assert_eq!(texts.len(), 2);
    assert!(texts[0].contains("First"));
    assert!(texts[1].contains("Second"));
}

// ============================================================================
// GUI/TUI parity contract — TUI ratatui output must contain the same semantic
// text that `render_line_plain_text()` (the canonical projection) produces.
// Both frontends agree on content; only styling and decorative glyphs differ.
// ============================================================================

#[test]
fn parity_with_plain_text_projection_for_canonical_documents() {
    use clarity_core::ui::render_line_plain_text;

    let fixtures = [
        "# Heading",
        "Plain paragraph.",
        "- list item",
        "1. ordered",
        "> blockquote",
        "```\ncode\n```",
        "# A\n\nB\n\n- C",
    ];

    for md in fixtures {
        let lines = markdown_to_lines(md);
        for line in &lines {
            let plain = render_line_plain_text(line);
            if plain.is_empty() {
                continue; // Empty/Divider/StreamingCursor — TUI may render decoration only.
            }
            let rata_text = line_to_plain(&render_line_to_ratatui(line, Style::default()));
            assert!(
                rata_text.contains(&plain),
                "PARITY VIOLATION for fixture {:?}:\n  plain_text: {:?}\n  ratatui:    {:?}",
                md,
                plain,
                rata_text
            );
        }
    }
}
