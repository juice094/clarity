//! Rich inline chip/mention/code layout using pretext.
//!
//! This module is intentionally low-level: it tokenizes a message-like string
//! into text / code / mention fragments, builds `pretext_core::RichInlineItem`
//! items with atomic chip breaks, and exposes the computed line ranges so that
//! callers can render each fragment with egui primitives while pretext decides
//! where to wrap.

use crate::theme::Theme;
use crate::ui::types::InlineSpan;
use pretext_core::Font;
use pretext_core::rich_inline::{RichInlineBreak, RichInlineItem};

/// Horizontal padding included in a chip's `extra_width` (6 px each side).
pub const CHIP_EXTRA_WIDTH: f32 = 12.0;

/// Tokens produced by the lightweight inline tokenizer.
#[derive(Debug, Clone, PartialEq)]
pub enum InlineToken<'a> {
    /// Plain text run.
    Text(&'a str),
    /// Backtick-delimited inline code.
    Code(&'a str),
    /// `@...` mention / agent chip.
    Mention(&'a str),
}

/// Tokenize a message-like string into text / code / mention tokens.
///
/// - `` `...` `` becomes `Code`.
/// - `@identifier` becomes `Mention`.
/// - Everything else is `Text`.
pub fn tokenize_inline(input: &str) -> Vec<InlineToken<'_>> {
    let mut tokens = Vec::new();
    let mut chars = input.char_indices().peekable();

    while let Some((start, c)) = chars.next() {
        match c {
            '`' => {
                let mut end = None;
                while let Some((i, ch)) = chars.peek() {
                    if *ch == '`' {
                        end = Some(*i);
                        chars.next();
                        break;
                    }
                    chars.next();
                }
                tokens.push(InlineToken::Code(
                    &input[start + 1..end.unwrap_or(input.len())],
                ));
            }
            '@' => {
                let mention_start = start;
                let mut end = start + c.len_utf8();
                while let Some((i, ch)) = chars.peek() {
                    if ch.is_alphanumeric() || *ch == '_' || *ch == '.' || *ch == '-' {
                        end = *i + ch.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(InlineToken::Mention(&input[mention_start..end]));
            }
            _ => {
                let text_start = start;
                let mut end = start + c.len_utf8();
                while let Some((i, ch)) = chars.peek() {
                    if *ch == '`' || *ch == '@' {
                        break;
                    }
                    end = *i + ch.len_utf8();
                    chars.next();
                }
                tokens.push(InlineToken::Text(&input[text_start..end]));
            }
        }
    }

    tokens
}

/// Convert tokens into `RichInlineItem`s ready for pretext layout.
///
/// Adjacent text tokens are merged so that pretext only sees the chip
/// boundaries that actually matter for line breaking.
pub fn build_rich_inline_items(tokens: &[InlineToken], theme: &Theme) -> Vec<RichInlineItem> {
    let body_font = body_font(theme);
    let mono_font = mono_font(theme);

    let mut items: Vec<RichInlineItem> = Vec::new();
    let mut pending_text = String::new();

    fn flush_text(items: &mut Vec<RichInlineItem>, pending: &mut String, font: &Font) {
        if !pending.is_empty() {
            items.push(RichInlineItem::new(std::mem::take(pending), font.clone()));
        }
    }

    for token in tokens {
        match token {
            InlineToken::Text(t) => {
                pending_text.push_str(t);
            }
            InlineToken::Code(text) => {
                flush_text(&mut items, &mut pending_text, &body_font);
                items.push(RichInlineItem {
                    text: (*text).to_string(),
                    font: mono_font.clone(),
                    letter_spacing: 0.0,
                    break_mode: RichInlineBreak::Never,
                    extra_width: CHIP_EXTRA_WIDTH,
                });
            }
            InlineToken::Mention(text) => {
                flush_text(&mut items, &mut pending_text, &body_font);
                items.push(RichInlineItem {
                    text: (*text).to_string(),
                    font: body_font.clone(),
                    letter_spacing: 0.0,
                    break_mode: RichInlineBreak::Never,
                    extra_width: CHIP_EXTRA_WIDTH,
                });
            }
        }
    }
    flush_text(&mut items, &mut pending_text, &body_font);

    items
}

/// Convert a raw string with backtick code and @mention markers into
/// `InlineSpan`s so that `rich_paragraph` can render it.
pub fn text_to_spans(input: &str) -> Vec<InlineSpan> {
    tokenize_inline(input)
        .into_iter()
        .map(|token| match token {
            InlineToken::Text(t) | InlineToken::Mention(t) => InlineSpan::Text(t.to_string()),
            InlineToken::Code(t) => InlineSpan::Code(t.to_string()),
        })
        .collect()
}

/// Convert markdown-derived `InlineSpan`s into `RichInlineItem`s.
///
/// - `Text` and `Link` are normal breakable runs.
/// - `Bold` uses the bold font descriptor but remains breakable.
/// - `Code` becomes an atomic chip with monospace font.
pub fn inline_spans_to_items(spans: &[InlineSpan], theme: &Theme) -> Vec<RichInlineItem> {
    use crate::pretext::{font_body, font_bold, font_code};

    spans
        .iter()
        .map(|span| match span {
            InlineSpan::Text(text) | InlineSpan::Link { text, .. } => {
                RichInlineItem::new(text.clone(), font_body(theme))
            }
            InlineSpan::Bold(text) => RichInlineItem::new(text.clone(), font_bold(theme)),
            InlineSpan::Code(text) => RichInlineItem {
                text: text.clone(),
                font: font_code(theme),
                letter_spacing: 0.0,
                break_mode: RichInlineBreak::Never,
                extra_width: CHIP_EXTRA_WIDTH,
            },
        })
        .collect()
}

/// A materialized fragment positioned on a specific line.
#[derive(Debug, Clone, PartialEq)]
pub struct PositionedFragment {
    /// Original item index in the `RichInlineItem` array.
    pub item_index: usize,
    /// Text to render.
    pub text: String,
    /// X offset from the left edge of the content area.
    pub x: f32,
    /// Width contributed by this fragment (text + extra_width).
    pub width: f32,
    /// True if this fragment represents a chip (code or mention).
    pub is_chip: bool,
}

/// Layout a rich inline flow and return per-line positioned fragments.
pub fn layout_rich_inline(
    items: &[RichInlineItem],
    max_width: f32,
    metrics: &dyn pretext_core::FontMetrics,
    profile: &pretext_core::EngineProfile,
) -> Vec<Vec<PositionedFragment>> {
    use pretext_core::rich_inline::{
        materialize_rich_inline_line_range, prepare_rich_inline, walk_rich_inline_line_ranges,
    };

    if items.is_empty() {
        return Vec::new();
    }

    let prepared = prepare_rich_inline(items, metrics, profile);
    let mut lines: Vec<Vec<PositionedFragment>> = Vec::new();

    walk_rich_inline_line_ranges(&prepared, max_width, profile, |line_range| {
        let line = materialize_rich_inline_line_range(&prepared, line_range);
        let mut line_frags = Vec::new();
        let mut x = 0.0f32;
        for fragment in &line.fragments {
            let is_chip = matches!(
                items.get(fragment.item_index).map(|i| i.break_mode),
                Some(RichInlineBreak::Never)
            );
            x += fragment.gap_before;
            line_frags.push(PositionedFragment {
                item_index: fragment.item_index,
                text: fragment.text.clone(),
                x,
                width: fragment.occupied_width,
                is_chip,
            });
            x += fragment.occupied_width;
        }
        lines.push(line_frags);
    });

    lines
}

fn body_font(theme: &Theme) -> Font {
    Font::new(format!("{}px {}", theme.text_base, theme.font_body))
}

fn mono_font(theme: &Theme) -> Font {
    Font::new(format!("{}px {}", theme.text_base, theme.font_mono))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretext_core::{EngineProfile, FontMetrics};

    /// Stub metrics that gives fixed per-grapheme widths for deterministic
    /// line-breaking tests. It does not need real font data.
    struct StubMetrics {
        body_width: f32,
        mono_width: f32,
    }

    impl FontMetrics for StubMetrics {
        fn measure(&self, text: &str, font: &Font) -> f32 {
            let w = if font.family == "Mono" {
                self.mono_width
            } else {
                self.body_width
            };
            text.chars().count() as f32 * w
        }

        fn supports_char(&self, _c: char, _font: &Font) -> bool {
            true
        }
    }

    fn theme_for_test() -> Theme {
        Theme {
            font_body: "Body".to_string(),
            font_mono: "Mono".to_string(),
            text_base: 10.0,
            ..Default::default()
        }
    }

    #[test]
    fn tokenize_mixed_text_code_mention() {
        let input = "Check `this_code` and @kimi please";
        let tokens = tokenize_inline(input);
        assert_eq!(
            tokens,
            vec![
                InlineToken::Text("Check "),
                InlineToken::Code("this_code"),
                InlineToken::Text(" and "),
                InlineToken::Mention("@kimi"),
                InlineToken::Text(" please"),
            ]
        );
    }

    #[test]
    fn chip_is_not_broken_across_lines() {
        let theme = theme_for_test();
        let items = build_rich_inline_items(
            &[
                InlineToken::Text("Hello "),
                InlineToken::Mention("@verylongmention"),
            ],
            &theme,
        );

        // Body 8 px per grapheme, chip text 16 chars * 8 + 12 extra = 140 px.
        // "Hello " is 48 px. With max_width = 60 px the chip cannot fit on the
        // first line, so it must be wrapped as a whole to the second line.
        let metrics = StubMetrics {
            body_width: 8.0,
            mono_width: 9.0,
        };
        let profile = EngineProfile::chromium();
        let lines = layout_rich_inline(&items, 60.0, &metrics, &profile);

        assert_eq!(lines.len(), 2, "chip should force a second line");
        let second_line = &lines[1];
        assert_eq!(second_line.len(), 1);
        assert_eq!(second_line[0].text, "@verylongmention");
        assert!(second_line[0].is_chip);
    }

    #[test]
    fn code_chip_is_atomic() {
        let theme = theme_for_test();
        let items = build_rich_inline_items(
            &[
                InlineToken::Text("Use "),
                InlineToken::Code("very_long_function_name"),
                InlineToken::Text(" here"),
            ],
            &theme,
        );

        let metrics = StubMetrics {
            body_width: 8.0,
            mono_width: 9.0,
        };
        let profile = EngineProfile::chromium();
        // Code text 23 chars * 9 + 12 = 219 px; "Use " 32 px. With max 100 px
        // the code chip must wrap whole.
        let lines = layout_rich_inline(&items, 100.0, &metrics, &profile);

        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].len(), 1);
        assert_eq!(lines[1].len(), 1);
        assert_eq!(lines[1][0].text, "very_long_function_name");
        assert!(lines[1][0].is_chip);
    }

    #[test]
    fn normal_text_can_break() {
        let theme = theme_for_test();
        let items = build_rich_inline_items(&[InlineToken::Text("abcdefgh ijklmnopq")], &theme);

        let metrics = StubMetrics {
            body_width: 8.0,
            mono_width: 9.0,
        };
        let profile = EngineProfile::chromium();
        let lines = layout_rich_inline(&items, 60.0, &metrics, &profile);

        assert!(lines.len() >= 2, "long plain text should wrap");
    }
}
