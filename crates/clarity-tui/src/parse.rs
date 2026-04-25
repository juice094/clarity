//! Pure-logic parsers for TUI commands.

/// Parse `/parallel` command arguments.
///
/// Input format: `<type>:<prompt> [| <type>:<prompt>...]`
/// Returns a Vec of `(agent_type, prompt)` pairs.
///
/// # Example
/// ```
/// use clarity_tui::parse::parse_parallel_args;
/// let specs = parse_parallel_args("coder:实现斐波那契函数 | explore:查找所有测试文件");
/// assert_eq!(specs.len(), 2);
/// assert_eq!(specs[0].0, "coder");
/// assert_eq!(specs[0].1, "实现斐波那契函数");
/// assert_eq!(specs[1].0, "explore");
/// assert_eq!(specs[1].1, "查找所有测试文件");
/// ```
pub fn parse_parallel_args(raw: &str) -> Vec<(String, String)> {
    let segments: Vec<&str> = raw
        .split('|')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut specs = Vec::with_capacity(segments.len());
    for seg in segments {
        let (agent_type, prompt) = match seg.find(':') {
            Some(idx) => (seg[..idx].trim(), seg[idx + 1..].trim()),
            None => ("coder", seg.trim()),
        };
        specs.push((agent_type.to_string(), prompt.to_string()));
    }
    specs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_parallel_single() {
        let specs = parse_parallel_args("coder:实现斐波那契函数");
        assert_eq!(
            specs,
            vec![("coder".to_string(), "实现斐波那契函数".to_string())]
        );
    }

    #[test]
    fn test_parse_parallel_multiple() {
        let specs = parse_parallel_args("coder:实现斐波那契函数 | explore:查找所有测试文件");
        assert_eq!(specs.len(), 2);
        assert_eq!(
            specs[0],
            ("coder".to_string(), "实现斐波那契函数".to_string())
        );
        assert_eq!(
            specs[1],
            ("explore".to_string(), "查找所有测试文件".to_string())
        );
    }

    #[test]
    fn test_parse_parallel_no_type() {
        let specs = parse_parallel_args("just a prompt");
        assert_eq!(
            specs,
            vec![("coder".to_string(), "just a prompt".to_string())]
        );
    }

    #[test]
    fn test_parse_parallel_empty() {
        let specs = parse_parallel_args("");
        assert!(specs.is_empty());
    }

    #[test]
    fn test_parse_parallel_whitespace() {
        let specs = parse_parallel_args("  coder : hello  |  explore : world  ");
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0], ("coder".to_string(), "hello".to_string()));
        assert_eq!(specs[1], ("explore".to_string(), "world".to_string()));
    }

    #[test]
    fn test_parse_parallel_extra_pipes() {
        let specs = parse_parallel_args("a:1 || b:2 |");
        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0], ("a".to_string(), "1".to_string()));
        assert_eq!(specs[1], ("b".to_string(), "2".to_string()));
    }
}
