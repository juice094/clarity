use similar::TextDiff;

#[derive(Debug, Clone, PartialEq)]
pub enum DiffLine {
    Context(String),
    Removed(String),
    Added(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct DiffHunk {
    pub old_start: usize,
    pub new_start: usize,
    pub lines: Vec<DiffLine>,
}

pub fn compute_diff(old: &str, new: &str) -> Vec<DiffHunk> {
    let diff = TextDiff::from_lines(old, new);
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
}
