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
