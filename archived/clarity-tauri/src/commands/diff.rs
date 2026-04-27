use serde::Serialize;

#[derive(Serialize, Clone, Debug)]
pub struct DiffLine {
    pub tag: String, // "equal", "delete", "insert"
    pub content: String,
}

#[derive(Serialize, Clone, Debug)]
pub struct DiffHunk {
    pub old_start: usize,
    pub new_start: usize,
    pub lines: Vec<DiffLine>,
}

#[tauri::command]
pub fn compute_diff(old_text: String, new_text: String) -> Vec<DiffHunk> {
    let diff = similar::TextDiff::from_lines(&old_text, &new_text);
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
                        lines.push(DiffLine {
                            tag: "delete".into(),
                            content: text,
                        });
                    }
                    similar::ChangeTag::Insert => {
                        if new_start.is_none() {
                            new_start = change.new_index().map(|i| i + 1);
                        }
                        lines.push(DiffLine {
                            tag: "insert".into(),
                            content: text,
                        });
                    }
                    similar::ChangeTag::Equal => {
                        if old_start.is_none() {
                            old_start = change.old_index().map(|i| i + 1);
                        }
                        if new_start.is_none() {
                            new_start = change.new_index().map(|i| i + 1);
                        }
                        lines.push(DiffLine {
                            tag: "equal".into(),
                            content: text,
                        });
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
