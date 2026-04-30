//! Diff utilities for unified diff parsing and structured representation.
//!
//! Extracted from `clarity-tui/src/diff.rs` and `popups/diff_popup.rs` to serve
//! as a shared foundation for both TUI and egui frontends.

use similar::{Algorithm, TextDiff};

/// A single line in a diff hunk.
#[derive(Debug, Clone, PartialEq)]
pub enum DiffLine {
    /// Unchanged context line.
    Context(String),
    /// Removed line (old version).
    Removed(String),
    /// Added line (new version).
    Added(String),
}

/// A hunk in a unified diff.
#[derive(Debug, Clone, PartialEq)]
pub struct DiffHunk {
    /// Starting line number in the old file (1-based).
    pub old_start: usize,
    /// Starting line number in the new file (1-based).
    pub new_start: usize,
    /// Lines in this hunk.
    pub lines: Vec<DiffLine>,
}

/// Compute a structured diff between old and new content.
///
/// Uses `similar::TextDiff` with the Patience algorithm for cleaner hunks.
pub fn compute_diff(old: &str, new: &str) -> Vec<DiffHunk> {
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Patience)
        .diff_lines(old, new);
    let mut hunks = Vec::new();

    for group in diff.grouped_ops(3) {
        let mut old_start = None;
        let mut new_start = None;
        let mut lines = Vec::new();

        for op in group {
            for change in diff.iter_changes(&op) {
                let text = change.value().to_string();
                match change.tag() {
                    similar::ChangeTag::Delete => {
                        if old_start.is_none() {
                            old_start = change.old_index().map(|i| i + 1);
                        }
                        lines.push(DiffLine::Removed(text));
                    }
                    similar::ChangeTag::Insert => {
                        if new_start.is_none() {
                            new_start = change.new_index().map(|i| i + 1);
                        }
                        lines.push(DiffLine::Added(text));
                    }
                    similar::ChangeTag::Equal => {
                        if old_start.is_none() {
                            old_start = change.old_index().map(|i| i + 1);
                        }
                        if new_start.is_none() {
                            new_start = change.new_index().map(|i| i + 1);
                        }
                        lines.push(DiffLine::Context(text));
                    }
                }
            }
        }

        hunks.push(DiffHunk {
            old_start: old_start.unwrap_or(1),
            new_start: new_start.unwrap_or(1),
            lines,
        });
    }

    hunks
}

/// Parse a unified diff patch string into structured hunks.
///
/// This is the inverse of `generate_unified_diff` — it takes a patch string
/// (with `@@` headers, `+`/`-`/` ` prefixes) and produces `DiffHunk`s.
pub fn parse_unified_diff(patch: &str) -> Vec<DiffHunk> {
    let mut hunks = Vec::new();
    let mut current_hunk: Option<DiffHunk> = None;

    for line in patch.lines() {
        if line.starts_with("@@") {
            if let Some(hunk) = current_hunk.take() {
                hunks.push(hunk);
            }
            if let Some((old_start, new_start)) = parse_hunk_header(line) {
                current_hunk = Some(DiffHunk {
                    old_start,
                    new_start,
                    lines: Vec::new(),
                });
            }
        } else if let Some(ref mut hunk) = current_hunk {
            if let Some(stripped) = line.strip_prefix('+') {
                if !line.starts_with("+++") {
                    hunk.lines
                        .push(DiffLine::Added(stripped.to_string() + "\n"));
                }
            } else if let Some(stripped) = line.strip_prefix('-') {
                if !line.starts_with("---") {
                    hunk.lines
                        .push(DiffLine::Removed(stripped.to_string() + "\n"));
                }
            } else if let Some(stripped) = line.strip_prefix(' ') {
                hunk.lines
                    .push(DiffLine::Context(stripped.to_string() + "\n"));
            } else if line.is_empty() {
                hunk.lines.push(DiffLine::Context("\n".to_string()));
            }
        }
    }

    if let Some(hunk) = current_hunk {
        hunks.push(hunk);
    }

    hunks
}

/// Parse a unified diff hunk header line.
/// Format: `@@ -start[,count] +start[,count] @@`
fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    let line = line.strip_prefix("@@ -")?;
    let parts: Vec<&str> = line.splitn(2, " +").collect();
    if parts.len() != 2 {
        return None;
    }
    let old_start = parts[0].split(',').next()?.parse::<usize>().ok()?;
    let new_part = parts[1].split(" @@").next()?;
    let new_start = new_part.split(',').next()?.parse::<usize>().ok()?;
    Some((old_start, new_start))
}

/// Generate a unified diff patch string from old and new content.
///
/// The output is compatible with `parse_unified_diff`.
pub fn generate_unified_diff(old: &str, new: &str, path: &str) -> String {
    let diff = TextDiff::configure()
        .algorithm(Algorithm::Patience)
        .diff_lines(old, new);
    diff.unified_diff().header(path, path).to_string()
}

/// Flatten hunks into a list of display-ready lines with their type.
/// Each line has its trailing newline stripped for display.
pub fn flatten_hunks(hunks: &[DiffHunk]) -> Vec<(&'static str, String)> {
    let mut out = Vec::new();
    for hunk in hunks {
        let old_count = hunk
            .lines
            .iter()
            .filter(|l| !matches!(l, DiffLine::Added(_)))
            .count();
        let new_count = hunk
            .lines
            .iter()
            .filter(|l| !matches!(l, DiffLine::Removed(_)))
            .count();
        out.push((
            "header",
            format!(
                "@@ -{},{} +{},{} @@",
                hunk.old_start, old_count, hunk.new_start, new_count
            ),
        ));
        for line in &hunk.lines {
            let (prefix, content) = match line {
                DiffLine::Context(s) => (" ", s.as_str()),
                DiffLine::Removed(s) => ("-", s.as_str()),
                DiffLine::Added(s) => ("+", s.as_str()),
            };
            let display = content.strip_suffix('\n').unwrap_or(content);
            out.push((prefix, format!("{}{}", prefix, display)));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_diff_basic() {
        let old = "line1\nline2\nline3\n";
        let new = "line1\nmodified\nline3\n";
        let hunks = compute_diff(old, new);
        assert!(!hunks.is_empty());

        let hunk = &hunks[0];
        assert!(hunk.old_start > 0);
        assert!(hunk.new_start > 0);

        let has_removed = hunk
            .lines
            .iter()
            .any(|l| matches!(l, DiffLine::Removed(s) if s.contains("line2")));
        let has_added = hunk
            .lines
            .iter()
            .any(|l| matches!(l, DiffLine::Added(s) if s.contains("modified")));
        assert!(has_removed);
        assert!(has_added);
    }

    #[test]
    fn test_parse_unified_diff_basic() {
        let patch =
            "--- a/test.txt\n+++ b/test.txt\n@@ -1,3 +1,3 @@\n line1\n-line2\n+modified\n line3\n";
        let hunks = parse_unified_diff(patch);
        assert_eq!(hunks.len(), 1);
        let hunk = &hunks[0];
        assert_eq!(hunk.old_start, 1);
        assert_eq!(hunk.new_start, 1);
        assert!(hunk
            .lines
            .iter()
            .any(|l| matches!(l, DiffLine::Removed(s) if s.contains("line2"))));
        assert!(hunk
            .lines
            .iter()
            .any(|l| matches!(l, DiffLine::Added(s) if s.contains("modified"))));
    }

    #[test]
    fn test_generate_and_parse_roundtrip() {
        let old = "a\nb\nc\n";
        let new = "a\nX\nc\n";
        let patch = generate_unified_diff(old, new, "test.txt");
        let hunks = parse_unified_diff(&patch);
        assert!(!hunks.is_empty());
        let flat = flatten_hunks(&hunks);
        assert!(flat.iter().any(|(t, _)| *t == "-"));
        assert!(flat.iter().any(|(t, _)| *t == "+"));
    }

    #[test]
    fn test_parse_hunk_header() {
        assert_eq!(parse_hunk_header("@@ -10,5 +20,7 @@"), Some((10, 20)));
        assert_eq!(parse_hunk_header("@@ -1 +1 @@"), Some((1, 1)));
        assert!(parse_hunk_header("not a header").is_none());
    }

    #[test]
    fn test_flatten_hunks() {
        let hunks = vec![DiffHunk {
            old_start: 1,
            new_start: 1,
            lines: vec![
                DiffLine::Context("a\n".into()),
                DiffLine::Removed("b\n".into()),
                DiffLine::Added("c\n".into()),
            ],
        }];
        let flat = flatten_hunks(&hunks);
        assert_eq!(flat[0].0, "header");
        assert!(flat[0].1.starts_with("@@"));
        assert_eq!(flat[1].0, " ");
        assert_eq!(flat[2].0, "-");
        assert_eq!(flat[3].0, "+");
    }
}
