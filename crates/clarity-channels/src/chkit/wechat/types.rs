//! Shared types, constants, and small helpers for the WeChat iLink channel.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

/// Default iLink API base URL.
pub(crate) const DEFAULT_API_BASE_URL: &str = "https://ilinkai.weixin.qq.com";
/// Default CDN base URL.
pub(crate) const CDN_BASE_URL: &str = "https://novac2c.cdn.weixin.qq.com/c2c";

/// Long-poll timeout for getUpdates (server may hold the request up to this).
pub(crate) const LONG_POLL_TIMEOUT_MS: u64 = 35_000;
/// Regular API request timeout.
pub(crate) const API_TIMEOUT: Duration = Duration::from_secs(15);

/// Session-expired error code returned by the iLink API.
pub(crate) const SESSION_EXPIRED_ERRCODE: i64 = -14;
/// Pause duration after session expiry before retrying.
pub(crate) const SESSION_PAUSE_DURATION: Duration = Duration::from_secs(60 * 60);
/// Maximum consecutive API failures before backing off.
pub(crate) const MAX_CONSECUTIVE_FAILURES: u32 = 3;
/// Back-off delay after reaching max consecutive failures.
pub(crate) const BACKOFF_DELAY: Duration = Duration::from_secs(30);
/// Retry delay for a single failure.
pub(crate) const RETRY_DELAY: Duration = Duration::from_secs(2);
/// QR code long-poll timeout.
pub(crate) const QR_POLL_TIMEOUT: Duration = Duration::from_secs(35);
/// Maximum QR code refresh attempts.
pub(crate) const MAX_QR_REFRESH: u32 = 3;
/// Total QR scan wait timeout.
pub(crate) const QR_SCAN_TIMEOUT: Duration = Duration::from_secs(480);

pub(crate) const WECHAT_BIND_COMMAND: &str = "/bind";

/// iLink Bot message types.
pub(crate) const MESSAGE_TYPE_BOT: u32 = 2;
/// iLink Bot message state.
pub(crate) const MESSAGE_STATE_FINISH: u32 = 2;
/// iLink Bot message item type: text.
pub(crate) const ITEM_TYPE_TEXT: u32 = 1;
/// iLink Bot message item type: image.
pub(crate) const ITEM_TYPE_IMAGE: u32 = 2;
/// iLink Bot message item type: voice.
pub(crate) const ITEM_TYPE_VOICE: u32 = 3;
/// iLink Bot message item type: file.
pub(crate) const ITEM_TYPE_FILE: u32 = 4;
/// iLink Bot message item type: video.
pub(crate) const ITEM_TYPE_VIDEO: u32 = 5;

/// getUploadUrl media type: image.
pub(crate) const UPLOAD_MEDIA_TYPE_IMAGE: u32 = 1;
/// getUploadUrl media type: video.
pub(crate) const UPLOAD_MEDIA_TYPE_VIDEO: u32 = 2;
/// getUploadUrl media type: file/document.
pub(crate) const UPLOAD_MEDIA_TYPE_FILE: u32 = 3;

/// Shared max size for inbound/outbound media handling.
pub(crate) const WECHAT_MEDIA_MAX_BYTES: u64 = 100 * 1024 * 1024;

pub(crate) fn long_poll_client_timeout(timeout_ms: u64) -> Duration {
    Duration::from_millis(timeout_ms + 5_000)
}

pub(crate) fn wechat_cli_string(key: &str) -> String {
    crate::chkit::i18n::get_required_cli_string(key)
}

pub(crate) fn wechat_cli_string_with_args(key: &str, args: &[(&str, &str)]) -> String {
    crate::chkit::i18n::get_required_cli_string_with_args(key, args)
}

pub(crate) fn https_base_url(
    field_name: &str,
    value: Option<String>,
    default: &str,
) -> anyhow::Result<String> {
    let url = value.unwrap_or_else(|| default.to_string());
    let url = url.trim().trim_end_matches('/').to_string();
    if !url.starts_with("https://") {
        anyhow::bail!("{field_name} must use https://, got {url}");
    }
    Ok(url)
}

pub(crate) fn build_base_info() -> serde_json::Value {
    serde_json::json!({
        "channel_version": env!("CARGO_PKG_VERSION")
    })
}

pub(crate) fn is_remote_url(target: &str) -> bool {
    target.starts_with("http://") || target.starts_with("https://")
}

pub(crate) fn sanitize_attachment_filename(file_name: &str) -> Option<String> {
    let cleaned = Path::new(file_name)
        .file_name()
        .and_then(|name| name.to_str())?
        .trim();
    if cleaned.is_empty() || cleaned == "." || cleaned == ".." {
        return None;
    }
    Some(cleaned.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WeChatAttachmentKind {
    Image,
    Document,
    Video,
    Audio,
    Voice,
}

impl WeChatAttachmentKind {
    pub(crate) fn from_marker(marker: &str) -> Option<Self> {
        match marker.trim().to_ascii_uppercase().as_str() {
            "IMAGE" | "PHOTO" => Some(Self::Image),
            "DOCUMENT" | "FILE" => Some(Self::Document),
            "VIDEO" => Some(Self::Video),
            "AUDIO" => Some(Self::Audio),
            "VOICE" => Some(Self::Voice),
            _ => None,
        }
    }

    pub(crate) fn default_extension(self) -> &'static str {
        match self {
            Self::Image => "png",
            Self::Document => "bin",
            Self::Video => "mp4",
            Self::Audio => "mp3",
            Self::Voice => "silk",
        }
    }

    pub(crate) fn upload_media_type(self) -> u32 {
        match self {
            Self::Image => UPLOAD_MEDIA_TYPE_IMAGE,
            Self::Video => UPLOAD_MEDIA_TYPE_VIDEO,
            Self::Document | Self::Audio | Self::Voice => UPLOAD_MEDIA_TYPE_FILE,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WeChatAttachment {
    pub(crate) kind: WeChatAttachmentKind,
    pub(crate) target: String,
}

#[derive(Debug, Clone)]
pub(crate) struct WeChatMediaPayload {
    pub(crate) bytes: Vec<u8>,
    pub(crate) file_name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct InboundAttachmentSpec {
    pub(crate) kind: WeChatAttachmentKind,
    pub(crate) encrypted_query_param: String,
    pub(crate) aes_key: Option<String>,
    pub(crate) file_name: String,
}

#[derive(Debug, Clone)]
pub(crate) struct UploadedWeChatMedia {
    pub(crate) encrypted_query_param: String,
    pub(crate) aes_key_base64: String,
    pub(crate) raw_size: usize,
    pub(crate) encrypted_size: usize,
}

/// Persistent account data (token + metadata).
#[derive(serde::Serialize, serde::Deserialize, Default)]
pub(crate) struct AccountData {
    #[serde(default)]
    pub(crate) token: Option<String>,
    #[serde(default)]
    pub(crate) base_url: Option<String>,
    #[serde(default)]
    pub(crate) account_id: Option<String>,
    #[serde(default)]
    pub(crate) user_id: Option<String>,
    #[serde(default)]
    pub(crate) saved_at: Option<String>,
}

/// Persistent sync cursor and context tokens.
#[derive(serde::Serialize, serde::Deserialize, Default)]
pub(crate) struct SyncData {
    #[serde(default)]
    pub(crate) get_updates_buf: String,
    #[serde(default)]
    pub(crate) context_tokens: HashMap<String, String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_base_info_includes_channel_version() {
        let base_info = build_base_info();
        let version = base_info
            .get("channel_version")
            .and_then(|value| value.as_str())
            .unwrap_or("");
        assert!(!version.is_empty());
    }

    #[test]
    fn sanitize_attachment_filename_rejects_special() {
        assert!(sanitize_attachment_filename(".").is_none());
        assert!(sanitize_attachment_filename("..").is_none());
        assert_eq!(
            sanitize_attachment_filename("/tmp/foo.txt"),
            Some("foo.txt".to_string())
        );
    }
}
