//! String truncation utilities — single source of truth for UI text
//! truncation across all panels, widgets, and handlers.

/// Maximum display characters for tool output before truncation.
pub const TOOL_OUTPUT_MAX_CHARS: usize = 2000;

/// Truncate a string to `max_chars` visible characters, appending `…` if
/// the string was shortened. Returns the truncated string and a boolean
/// indicating whether truncation occurred.
pub fn truncate_str(s: &str, max_chars: usize) -> (String, bool) {
    let chars: Vec<char> = s.chars().collect();
    if chars.len() <= max_chars {
        return (s.to_string(), false);
    }
    let mut out: String = chars
        .into_iter()
        .take(max_chars.saturating_sub(1))
        .collect();
    out.push('\u{2026}');
    (out, true)
}

/// Truncate a string for display only (no truncation flag).
pub fn truncate(s: &str, max_chars: usize) -> String {
    truncate_str(s, max_chars).0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_str_short_string_is_unchanged() {
        let (result, truncated) = truncate_str("hello", 10);
        assert_eq!(result, "hello");
        assert!(!truncated);
    }

    #[test]
    fn truncate_str_exact_length_is_unchanged() {
        let input = "12345";
        let (result, truncated) = truncate_str(input, 5);
        assert_eq!(result, "12345");
        assert!(!truncated);
    }

    #[test]
    fn truncate_str_long_string_is_truncated() {
        let (result, truncated) = truncate_str("hello world this is long", 10);
        assert!(truncated);
        assert!(result.len() <= 10 + 3); // room for ellipsis char
        assert!(result.ends_with('\u{2026}'));
    }

    #[test]
    fn truncate_str_empty_input() {
        let (result, truncated) = truncate_str("", 5);
        assert_eq!(result, "");
        assert!(!truncated);
    }

    #[test]
    fn truncate_str_max_chars_zero() {
        let (result, truncated) = truncate_str("hello", 0);
        assert_eq!(result, "\u{2026}");
        assert!(truncated);
    }

    #[test]
    fn truncate_str_max_chars_one() {
        let (result, truncated) = truncate_str("hello", 1);
        assert_eq!(result, "\u{2026}");
        assert!(truncated);
    }

    #[test]
    fn truncate_str_max_chars_two_shows_one_char() {
        let (result, truncated) = truncate_str("hello", 2);
        assert_eq!(result, "h\u{2026}");
        assert!(truncated);
    }

    #[test]
    fn truncate_str_cjk_characters() {
        let (result, truncated) = truncate_str("你好世界这是测试", 3);
        assert!(truncated);
        assert_eq!(result.chars().count(), 3); // 2 chars + 1 ellipsis
        assert!(result.ends_with('\u{2026}'));
    }

    #[test]
    fn truncate_str_emoji_preserved() {
        let (result, truncated) = truncate_str("🚀🌟✨💫🔥", 3);
        assert!(truncated);
        assert!(result.starts_with("🚀"));
        assert!(result.ends_with('\u{2026}'));
    }

    #[test]
    fn truncate_helper_returns_string_only() {
        let result = truncate("hello world", 5);
        assert_eq!(result, "hell\u{2026}");
    }

    #[test]
    fn truncate_at_tool_output_max() {
        let long = "x".repeat(TOOL_OUTPUT_MAX_CHARS + 500);
        let (result, truncated) = truncate_str(&long, TOOL_OUTPUT_MAX_CHARS);
        assert!(truncated);
        assert!(result.len() <= TOOL_OUTPUT_MAX_CHARS + 3);
        assert!(result.ends_with('\u{2026}'));
    }
}
