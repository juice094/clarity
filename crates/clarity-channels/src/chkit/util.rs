//! Utility helpers for channel message normalization.

/// Strip tool-call XML tags from a message before sending it to a channel.
pub fn strip_tool_call_tags(message: &str) -> String {
    const TOOL_CALL_OPEN_TAGS: [&str; 7] = [
        "<function_calls>",
        "<function_call>",
        "<tool_call>",
        "<toolcall>",
        "<tool-call>",
        "<tool>",
        "<invoke>",
    ];

    fn find_first_tag<'a>(haystack: &str, tags: &'a [&'a str]) -> Option<(usize, &'a str)> {
        tags.iter()
            .filter_map(|tag| haystack.find(tag).map(|idx| (idx, *tag)))
            .min_by_key(|(idx, _)| *idx)
    }

    fn matching_close_tag(open_tag: &str) -> Option<&'static str> {
        match open_tag {
            "<function_calls>" => Some("</function_calls>"),
            "<function_call>" => Some("</function_call>"),
            "<tool_call>" => Some("</tool_call>"),
            "<toolcall>" => Some("</toolcall>"),
            "<tool-call>" => Some("</tool-call>"),
            "<tool>" => Some("</tool>"),
            "<invoke>" => Some("</invoke>"),
            _ => None,
        }
    }

    fn extract_first_json_end(input: &str) -> Option<usize> {
        let trimmed = input.trim_start();
        let trim_offset = input.len().saturating_sub(trimmed.len());

        for (byte_idx, ch) in trimmed.char_indices() {
            if ch != '{' && ch != '[' {
                continue;
            }

            let slice = &trimmed[byte_idx..];
            let mut stream =
                serde_json::Deserializer::from_str(slice).into_iter::<serde_json::Value>();
            if let Some(Ok(_value)) = stream.next() {
                let consumed = stream.byte_offset();
                if consumed > 0 {
                    return Some(trim_offset + byte_idx + consumed);
                }
            }
        }

        None
    }

    fn strip_leading_close_tags(mut input: &str) -> &str {
        loop {
            let trimmed = input.trim_start();
            if !trimmed.starts_with("</") {
                return trimmed;
            }

            let Some(close_end) = trimmed.find('>') else {
                return "";
            };
            input = &trimmed[close_end + 1..];
        }
    }

    fn tool_structure_runs_to_end(inner: &str) -> bool {
        let mut rest = inner.trim_start();
        while rest.starts_with('<') {
            match rest.find('>') {
                Some(gt) => rest = rest[gt + 1..].trim_start(),
                None => return true,
            }
        }
        let tail = rest.trim();
        if tail.is_empty() {
            return true;
        }
        !looks_like_prose(tail)
    }

    fn looks_like_prose(text: &str) -> bool {
        let bytes = text.as_bytes();
        for i in 0..bytes.len().saturating_sub(1) {
            if matches!(bytes[i], b'.' | b'!' | b'?')
                && matches!(bytes[i + 1], b' ' | b'\n' | b'\t')
                && text[i + 1..]
                    .trim_start()
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphabetic())
            {
                return true;
            }
        }
        let trimmed = text.trim_end();
        let ends_like_sentence = trimmed
            .chars()
            .last()
            .is_some_and(|c| matches!(c, '.' | '!' | '?'))
            && trimmed
                .chars()
                .rev()
                .nth(1)
                .is_some_and(|c| c.is_alphabetic());
        ends_like_sentence && text.trim().contains(' ')
    }

    let mut kept_segments = Vec::new();
    let mut remaining = message;

    while let Some((start, open_tag)) = find_first_tag(remaining, &TOOL_CALL_OPEN_TAGS) {
        let before = &remaining[..start];
        if !before.is_empty() {
            kept_segments.push(before.to_string());
        }

        let Some(close_tag) = matching_close_tag(open_tag) else {
            break;
        };
        let after_open = &remaining[start + open_tag.len()..];

        if let Some(close_idx) = after_open.find(close_tag) {
            remaining = &after_open[close_idx + close_tag.len()..];
            continue;
        }

        if let Some(consumed_end) = extract_first_json_end(after_open) {
            remaining = strip_leading_close_tags(&after_open[consumed_end..]);
            continue;
        }

        let inner = after_open.trim_start();
        let inner_lower = inner.to_ascii_lowercase();
        let looks_like_tool_structure = inner_lower.starts_with("<invoke")
            || inner_lower.starts_with("<parameter")
            || inner_lower.starts_with("<tool")
            || inner_lower.starts_with("<function")
            || inner.starts_with('{')
            || inner.starts_with('[');
        if looks_like_tool_structure && tool_structure_runs_to_end(inner) {
            remaining = "";
            break;
        }

        kept_segments.push(remaining[start..].to_string());
        remaining = "";
        break;
    }

    if !remaining.is_empty() {
        kept_segments.push(remaining.to_string());
    }

    let mut result = kept_segments.concat();

    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }

    result.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn leaves_plain_text_unchanged() {
        assert_eq!(strip_tool_call_tags("Hello, world!"), "Hello, world!");
    }

    #[test]
    fn strips_closed_tool_call_block() {
        let input = "Before <tool_call>{\"x\":1}</tool_call> After";
        assert_eq!(strip_tool_call_tags(input), "Before  After");
    }

    #[test]
    fn strips_function_calls_block() {
        let input = "Plan: <function_calls><invoke>foo</invoke></function_calls> Done";
        assert_eq!(strip_tool_call_tags(input), "Plan:  Done");
    }

    #[test]
    fn strips_unclosed_json_and_trailing_close_tags() {
        let input = "Text <tool_call>{\"action\":\"x\"}</tool_call> trailing";
        assert_eq!(strip_tool_call_tags(input), "Text  trailing");
    }

    #[test]
    fn strips_unclosed_json_but_keeps_plain_trailing_text() {
        let input = "Text <tool_call>{\"action\":\"x\"} trailing";
        assert_eq!(strip_tool_call_tags(input), "Text trailing");
    }

    #[test]
    fn strips_tool_structure_that_runs_to_end() {
        let input = "Intro <tool_call><invoke><parameter>foo</parameter></invoke>";
        assert_eq!(strip_tool_call_tags(input), "Intro");
    }

    #[test]
    fn preserves_unclosed_tag_that_looks_like_prose() {
        let input = "Note: <tool_call>This is a sentence. Keep it.";
        assert_eq!(
            strip_tool_call_tags(input),
            "Note: <tool_call>This is a sentence. Keep it."
        );
    }

    #[test]
    fn strips_multiple_blocks() {
        let input = "A <tool>1</tool> B <tool>2</tool> C";
        assert_eq!(strip_tool_call_tags(input), "A  B  C");
    }

    #[test]
    fn normalizes_excess_blank_lines() {
        let input = "A\n\n\n\nB";
        assert_eq!(strip_tool_call_tags(input), "A\n\nB");
    }

    #[test]
    fn trims_leading_and_trailing_whitespace() {
        let input = "   \n\nhello\n\n   ";
        assert_eq!(strip_tool_call_tags(input), "hello");
    }

    #[test]
    fn handles_unknown_open_tag_without_matching_close() {
        let input = "Start <unknown>content End";
        assert_eq!(strip_tool_call_tags(input), "Start <unknown>content End");
    }
}
