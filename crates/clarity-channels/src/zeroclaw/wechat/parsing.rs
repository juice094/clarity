//! Inbound message parsing and attachment extraction for WeChat iLink.

use std::path::Path;
use std::sync::LazyLock;

use crate::zeroclaw::wechat::types::{
    ITEM_TYPE_FILE, ITEM_TYPE_IMAGE, ITEM_TYPE_TEXT, ITEM_TYPE_VIDEO, ITEM_TYPE_VOICE,
    InboundAttachmentSpec, WeChatAttachment, WeChatAttachmentKind, sanitize_attachment_filename,
};

pub(crate) fn infer_attachment_kind_from_target(target: &str) -> Option<WeChatAttachmentKind> {
    let normalized = target
        .split('?')
        .next()
        .unwrap_or(target)
        .split('#')
        .next()
        .unwrap_or(target);

    let extension = Path::new(normalized)
        .extension()
        .and_then(|ext| ext.to_str())?
        .to_ascii_lowercase();

    match extension.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" => Some(WeChatAttachmentKind::Image),
        "mp4" | "mov" | "mkv" | "avi" | "webm" => Some(WeChatAttachmentKind::Video),
        "mp3" | "m4a" | "wav" | "flac" => Some(WeChatAttachmentKind::Audio),
        "ogg" | "oga" | "opus" | "silk" => Some(WeChatAttachmentKind::Voice),
        "pdf" | "txt" | "md" | "csv" | "json" | "zip" | "tar" | "gz" | "doc" | "docx" | "xls"
        | "xlsx" | "ppt" | "pptx" => Some(WeChatAttachmentKind::Document),
        _ => None,
    }
}

pub(crate) fn find_matching_close(s: &str) -> Option<usize> {
    let mut depth = 1usize;
    for (i, ch) in s.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

pub(crate) fn parse_attachment_markers(message: &str) -> (String, Vec<WeChatAttachment>) {
    let mut cleaned = String::with_capacity(message.len());
    let mut attachments = Vec::new();
    let mut cursor = 0usize;

    while cursor < message.len() {
        let Some(open_rel) = message[cursor..].find('[') else {
            cleaned.push_str(&message[cursor..]);
            break;
        };

        let open = cursor + open_rel;
        cleaned.push_str(&message[cursor..open]);

        let Some(close_rel) = find_matching_close(&message[open + 1..]) else {
            cleaned.push_str(&message[open..]);
            break;
        };

        let close = open + 1 + close_rel;
        let marker = &message[open + 1..close];

        let parsed = marker.split_once(':').and_then(|(kind, target)| {
            let kind = WeChatAttachmentKind::from_marker(kind)?;
            let target = target.trim();
            if target.is_empty() {
                return None;
            }
            Some(WeChatAttachment {
                kind,
                target: target.to_string(),
            })
        });

        if let Some(attachment) = parsed {
            attachments.push(attachment);
        } else {
            cleaned.push_str(&message[open..=close]);
        }

        cursor = close + 1;
    }

    (cleaned.trim().to_string(), attachments)
}

pub(crate) fn parse_path_only_attachment(message: &str) -> Option<WeChatAttachment> {
    let trimmed = message.trim();
    if trimmed.is_empty() || trimmed.contains('\n') {
        return None;
    }

    let candidate = trimmed.trim_matches(|c| matches!(c, '`' | '"' | '\''));
    if candidate.chars().any(char::is_whitespace) {
        return None;
    }

    let candidate = candidate.strip_prefix("file://").unwrap_or(candidate);
    let kind = infer_attachment_kind_from_target(candidate)?;

    if !crate::zeroclaw::wechat::types::is_remote_url(candidate) && !Path::new(candidate).exists() {
        return None;
    }

    Some(WeChatAttachment {
        kind,
        target: candidate.to_string(),
    })
}

pub(crate) fn format_attachment_content(
    kind: WeChatAttachmentKind,
    local_filename: &str,
    local_path: &Path,
) -> String {
    if kind == WeChatAttachmentKind::Image {
        format!("[IMAGE:{}]", local_path.display())
    } else {
        format!("[Document: {}] {}", local_filename, local_path.display())
    }
}

/// Compile a constant regex pattern for static use.
///
/// # Panics
///
/// Panics only if `pattern` is invalid; all callers pass literal, known-good
/// patterns, so this is treated as an infallible construction helper.
#[allow(clippy::unwrap_used)]
fn static_regex(pattern: &str) -> regex::Regex {
    regex::Regex::new(pattern).unwrap()
}

// Statically-compiled regexes for markdown-to-plain-text conversion.
static CODE_BLOCK_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(r"```[^\n]*\n?([\s\S]*?)```"));
static IMAGE_RE: LazyLock<regex::Regex> = LazyLock::new(|| static_regex(r"!\[[^\]]*\]\([^)]*\)"));
static LINK_RE: LazyLock<regex::Regex> = LazyLock::new(|| static_regex(r"\[([^\]]+)\]\([^)]*\)"));
static HEADING_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(r"(?m)^\s{0,3}#{1,6}\s+"));
static BLOCKQUOTE_RE: LazyLock<regex::Regex> = LazyLock::new(|| static_regex(r"(?m)^>\s?"));
static BULLET_RE: LazyLock<regex::Regex> = LazyLock::new(|| static_regex(r"(?m)^\s*[-*+]\s+"));
static EMPHASIS_RE: LazyLock<regex::Regex> = LazyLock::new(|| static_regex(r"(\*\*|__|~~|`|\*)"));
static TABLE_SEPARATOR_RE: LazyLock<regex::Regex> =
    LazyLock::new(|| static_regex(r"^\|[\s:|-]+\|$"));
static TABLE_ROW_RE: LazyLock<regex::Regex> = LazyLock::new(|| static_regex(r"^\|(.+)\|$"));

pub(crate) fn markdown_to_plain_text(text: &str) -> String {
    let mut result = CODE_BLOCK_RE.replace_all(text, "$1").into_owned();
    result = IMAGE_RE.replace_all(&result, "").into_owned();
    result = LINK_RE.replace_all(&result, "$1").into_owned();

    let mut lines = Vec::new();
    for line in result.lines() {
        if TABLE_SEPARATOR_RE.is_match(line) {
            continue;
        }

        if let Some(captures) = TABLE_ROW_RE.captures(line) {
            let inner = captures.get(1).map(|value| value.as_str()).unwrap_or("");
            lines.push(
                inner
                    .split('|')
                    .map(str::trim)
                    .filter(|cell| !cell.is_empty())
                    .collect::<Vec<_>>()
                    .join("  "),
            );
        } else {
            lines.push(line.to_string());
        }
    }

    result = lines.join("\n");
    result = HEADING_RE.replace_all(&result, "").into_owned();
    result = BLOCKQUOTE_RE.replace_all(&result, "").into_owned();
    result = BULLET_RE.replace_all(&result, "").into_owned();
    result = EMPHASIS_RE.replace_all(&result, "").into_owned();

    while result.contains("\n\n\n") {
        result = result.replace("\n\n\n", "\n\n");
    }

    result.trim().to_string()
}

pub(crate) fn extract_text_from_items(items: &[serde_json::Value]) -> String {
    for item in items {
        let item_type = item
            .get("type")
            .and_then(|v| v.as_u64())
            .and_then(|value| u32::try_from(value).ok())
            .unwrap_or(0);
        match item_type {
            ITEM_TYPE_TEXT => {
                if let Some(text) = item
                    .get("text_item")
                    .and_then(|ti| ti.get("text"))
                    .and_then(|t| t.as_str())
                {
                    // Handle ref_msg (quoted message)
                    let ref_prefix = if let Some(ref_msg) = item.get("ref_msg") {
                        let title = ref_msg.get("title").and_then(|t| t.as_str()).unwrap_or("");
                        if title.is_empty() {
                            String::new()
                        } else {
                            format!("[引用: {title}]\n")
                        }
                    } else {
                        String::new()
                    };
                    return format!("{ref_prefix}{text}");
                }
            }
            ITEM_TYPE_VOICE => {
                // Voice-to-text transcription
                if let Some(text) = item
                    .get("voice_item")
                    .and_then(|vi| vi.get("text"))
                    .and_then(|t| t.as_str())
                    && !text.is_empty()
                {
                    return text.to_string();
                }
            }
            _ => {}
        }
    }
    String::new()
}

pub(crate) fn find_inbound_attachment(
    items: &[serde_json::Value],
    message_id: &str,
) -> Option<InboundAttachmentSpec> {
    fn default_name(kind: WeChatAttachmentKind, message_id: &str) -> String {
        let safe_id: String = message_id
            .chars()
            .map(|ch| if ch.is_ascii_alphanumeric() { ch } else { '_' })
            .collect();
        match kind {
            WeChatAttachmentKind::Image => format!("wechat_{safe_id}.jpg"),
            WeChatAttachmentKind::Document => format!("wechat_{safe_id}.bin"),
            WeChatAttachmentKind::Video => format!("wechat_{safe_id}.mp4"),
            WeChatAttachmentKind::Audio => format!("wechat_{safe_id}.mp3"),
            WeChatAttachmentKind::Voice => format!("wechat_{safe_id}.silk"),
        }
    }

    fn parse_item(item: &serde_json::Value, message_id: &str) -> Option<InboundAttachmentSpec> {
        let item_type = item
            .get("type")
            .and_then(|value| value.as_u64())
            .and_then(|value| u32::try_from(value).ok())?;
        match item_type {
            ITEM_TYPE_IMAGE => {
                let image_item = item.get("image_item")?;
                let media = image_item.get("media")?;
                let encrypted_query_param = media.get("encrypt_query_param")?.as_str()?.to_string();
                let aes_key = image_item
                    .get("aeskey")
                    .and_then(|value| value.as_str())
                    .or_else(|| media.get("aes_key").and_then(|value| value.as_str()))
                    .map(str::to_string);
                Some(InboundAttachmentSpec {
                    kind: WeChatAttachmentKind::Image,
                    encrypted_query_param,
                    aes_key,
                    file_name: default_name(WeChatAttachmentKind::Image, message_id),
                })
            }
            ITEM_TYPE_FILE => {
                let file_item = item.get("file_item")?;
                let media = file_item.get("media")?;
                let encrypted_query_param = media.get("encrypt_query_param")?.as_str()?.to_string();
                let aes_key = media
                    .get("aes_key")
                    .and_then(|value| value.as_str())
                    .map(str::to_string);
                let file_name = file_item
                    .get("file_name")
                    .and_then(|value| value.as_str())
                    .and_then(sanitize_attachment_filename)
                    .unwrap_or_else(|| default_name(WeChatAttachmentKind::Document, message_id));
                Some(InboundAttachmentSpec {
                    kind: WeChatAttachmentKind::Document,
                    encrypted_query_param,
                    aes_key,
                    file_name,
                })
            }
            ITEM_TYPE_VIDEO => {
                let video_item = item.get("video_item")?;
                let media = video_item.get("media")?;
                let encrypted_query_param = media.get("encrypt_query_param")?.as_str()?.to_string();
                let aes_key = media
                    .get("aes_key")
                    .and_then(|value| value.as_str())
                    .map(str::to_string);
                Some(InboundAttachmentSpec {
                    kind: WeChatAttachmentKind::Video,
                    encrypted_query_param,
                    aes_key,
                    file_name: default_name(WeChatAttachmentKind::Video, message_id),
                })
            }
            ITEM_TYPE_VOICE => {
                let voice_item = item.get("voice_item")?;
                let media = voice_item.get("media")?;
                let encrypted_query_param = media.get("encrypt_query_param")?.as_str()?.to_string();
                let aes_key = media
                    .get("aes_key")
                    .and_then(|value| value.as_str())
                    .map(str::to_string);
                Some(InboundAttachmentSpec {
                    kind: WeChatAttachmentKind::Voice,
                    encrypted_query_param,
                    aes_key,
                    file_name: default_name(WeChatAttachmentKind::Voice, message_id),
                })
            }
            _ => None,
        }
    }

    for item in items {
        if let Some(spec) = parse_item(item, message_id) {
            return Some(spec);
        }
    }

    for item in items {
        let Some(ref_item) = item
            .get("ref_msg")
            .and_then(|value| value.get("message_item"))
        else {
            continue;
        };

        if let Some(spec) = parse_item(ref_item, message_id) {
            return Some(spec);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use tempfile::tempdir;

    #[test]
    fn extract_text_from_items_text() {
        let items = vec![serde_json::json!({
            "type": 1,
            "text_item": { "text": "hello world" }
        })];
        assert_eq!(extract_text_from_items(&items), "hello world");
    }

    #[test]
    fn extract_text_from_items_voice() {
        let items = vec![serde_json::json!({
            "type": 3,
            "voice_item": { "text": "voice transcription" }
        })];
        assert_eq!(extract_text_from_items(&items), "voice transcription");
    }

    #[test]
    fn extract_text_from_items_empty() {
        let items = vec![serde_json::json!({
            "type": 2,
            "image_item": {}
        })];
        assert_eq!(extract_text_from_items(&items), "");
    }

    #[test]
    fn extract_text_with_ref_msg() {
        let items = vec![serde_json::json!({
            "type": 1,
            "text_item": { "text": "reply text" },
            "ref_msg": { "title": "original message" }
        })];
        assert_eq!(
            extract_text_from_items(&items),
            "[引用: original message]\nreply text"
        );
    }

    #[test]
    fn parse_attachment_markers_extracts_multiple_types() {
        let message = "See this\n[IMAGE:/tmp/a.png]\n[DOCUMENT:https://example.com/a.pdf]";
        let (cleaned, attachments) = parse_attachment_markers(message);

        assert_eq!(cleaned, "See this");
        assert_eq!(attachments.len(), 2);
        assert_eq!(attachments[0].kind, WeChatAttachmentKind::Image);
        assert_eq!(attachments[0].target, "/tmp/a.png");
        assert_eq!(attachments[1].kind, WeChatAttachmentKind::Document);
        assert_eq!(attachments[1].target, "https://example.com/a.pdf");
    }

    #[test]
    fn parse_attachment_markers_keeps_invalid_marker_text() {
        let message = "See [UNKNOWN:/tmp/a.bin]";
        let (cleaned, attachments) = parse_attachment_markers(message);
        assert_eq!(cleaned, message);
        assert!(attachments.is_empty());
    }

    #[test]
    fn parse_path_only_attachment_detects_existing_file() {
        let temp = tempdir().unwrap();
        let image_path = temp.path().join("photo.png");
        std::fs::write(&image_path, b"png").unwrap();

        let parsed = parse_path_only_attachment(image_path.to_string_lossy().as_ref())
            .expect("expected attachment");
        assert_eq!(parsed.kind, WeChatAttachmentKind::Image);
        assert_eq!(parsed.target, image_path.to_string_lossy());
    }

    #[test]
    fn parse_path_only_attachment_rejects_sentence_text() {
        assert!(parse_path_only_attachment("saved to /tmp/photo.png").is_none());
    }

    #[test]
    fn format_attachment_content_uses_image_marker_for_images() {
        let path = PathBuf::from("/tmp/workspace/photo.png");
        assert_eq!(
            format_attachment_content(WeChatAttachmentKind::Image, "photo.png", &path),
            "[IMAGE:/tmp/workspace/photo.png]"
        );
    }

    #[test]
    fn format_attachment_content_uses_document_marker_for_non_images() {
        let path = PathBuf::from("/tmp/workspace/report.pdf");
        assert_eq!(
            format_attachment_content(WeChatAttachmentKind::Document, "report.pdf", &path),
            "[Document: report.pdf] /tmp/workspace/report.pdf"
        );
    }

    #[test]
    fn markdown_to_plain_text_strips_common_formatting() {
        let input = "# Title\n**bold** [link](https://example.com)\n\n```rust\nlet x = 1;\n```";
        assert_eq!(
            markdown_to_plain_text(input),
            "Title\nbold link\n\nlet x = 1;"
        );
    }

    #[test]
    fn find_inbound_attachment_prefers_direct_media() {
        let items = vec![
            serde_json::json!({
                "type": 1,
                "text_item": { "text": "caption" },
                "ref_msg": {
                    "message_item": {
                        "type": 4,
                        "file_item": {
                            "media": {
                                "encrypt_query_param": "quoted"
                            },
                            "file_name": "quoted.pdf"
                        }
                    }
                }
            }),
            serde_json::json!({
                "type": 2,
                "image_item": {
                    "media": {
                        "encrypt_query_param": "direct"
                    }
                }
            }),
        ];

        let spec = find_inbound_attachment(&items, "123").unwrap();
        assert_eq!(spec.kind, WeChatAttachmentKind::Image);
        assert_eq!(spec.encrypted_query_param, "direct");
    }
}
