//! Kimi cloud file resolution and download.
//!
//! Replicates the behavior of KimiClaw's `KimiFileResolver`:
//!
//! 1. Parse `kimi-file://{uuid}` URIs from chat resource links.
//! 2. Query `{kimiapiHost}/api-claw/files/{fileId}` with `X-Kimi-Bot-Token`.
//! 3. Extract `blob.signUrl` (or image preview URL as fallback).
//! 4. Download to `{kimiFileDownloadDir}/{fileId}_{sanitizedName}`.
//! 5. Cache-hit by scanning for an existing `{fileId}_*` file.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Header used to authenticate Kimi file API requests.
pub const KIMI_BOT_TOKEN_HEADER: &str = "X-Kimi-Bot-Token";
/// Metadata path prefix for Kimi file API.
pub const KIMI_FILE_METADATA_PATH_PREFIX: &str = "/api-claw/files/";
/// Maximum length for a sanitized local file name.
pub const MAX_KIMI_FILE_NAME_LENGTH: usize = 120;

/// Parsed metadata for a Kimi cloud file.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KimiFileMetadata {
    /// File id (UUID).
    pub file_id: String,
    /// Server-side file id (usually identical to file_id).
    pub id: String,
    /// Original file name.
    pub name: String,
    /// MIME type.
    pub content_type: String,
    /// File size in bytes, when known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,
    /// Signed URL used to download the blob.
    pub download_url: String,
    /// Source of the download URL (`blob.signUrl` or image preview).
    pub download_url_source: String,
}

/// Result of resolving a local download.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KimiFileDownload {
    /// Absolute path to the local file.
    pub local_path: PathBuf,
    /// Local file name.
    pub local_file_name: String,
    /// Size in bytes on disk.
    pub local_size_bytes: u64,
    /// Whether this was a cache hit.
    pub local_cache_hit: bool,
}

/// Parse a `kimi-file://{uuid}` URI and return the file id.
pub fn parse_kimi_file_uri(uri: &str) -> Option<String> {
    const PREFIX: &str = "kimi-file://";
    if !uri.starts_with(PREFIX) {
        return None;
    }
    let file_id = uri[PREFIX.len()..].trim();
    if is_valid_kimi_file_id(file_id) {
        Some(file_id.to_string())
    } else {
        None
    }
}

fn is_valid_kimi_file_id(id: &str) -> bool {
    // UUID format: 8-4-4-4-12 hex digits.
    let parts: Vec<&str> = id.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let expected = [8, 4, 4, 4, 12];
    parts
        .iter()
        .zip(expected.iter())
        .all(|(p, &len)| p.len() == len && p.chars().all(|c| c.is_ascii_hexdigit()))
}

/// Resolve file metadata from Kimi's file API.
///
/// `client` is a `reqwest::Client` reused across requests.
pub async fn resolve_metadata(
    file_id: &str,
    kimi_api_host: &str,
    bot_token: &str,
    client: &reqwest::Client,
) -> Result<KimiFileMetadata> {
    let url = build_metadata_url(kimi_api_host, file_id)?;

    let resp = client
        .get(&url)
        .header(KIMI_BOT_TOKEN_HEADER, bot_token)
        .header("Accept", "application/json")
        .send()
        .await
        .with_context(|| format!("request metadata for {}", file_id))?;

    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!(
            "files api request failed for {}: status {}",
            file_id,
            status
        );
    }

    let body: serde_json::Value = resp
        .json()
        .await
        .context("parse files api response as json")?;

    parse_metadata_response(file_id, &body)
}

fn build_metadata_url(kimi_api_host: &str, file_id: &str) -> Result<String> {
    let base = reqwest::Url::parse(kimi_api_host).context("parse kimi_api_host")?;
    let path = format!("{}{}", KIMI_FILE_METADATA_PATH_PREFIX, file_id);
    let url = base.join(&path).context("join metadata path")?;
    Ok(url.to_string())
}

fn parse_metadata_response(file_id: &str, body: &serde_json::Value) -> Result<KimiFileMetadata> {
    let id = body["id"]
        .as_str()
        .with_context(|| format!("files api response missing id for {}", file_id))?;
    let meta = body["meta"]
        .as_object()
        .with_context(|| format!("files api response missing meta for {}", file_id))?;
    let name = meta
        .get("name")
        .and_then(|v| v.as_str())
        .with_context(|| format!("files api response missing meta.name for {}", file_id))?;
    let content_type = meta
        .get("contentType")
        .and_then(|v| v.as_str())
        .or_else(|| meta.get("content_type").and_then(|v| v.as_str()))
        .with_context(|| {
            format!(
                "files api response missing meta.contentType for {}",
                file_id
            )
        })?;

    let size_bytes = meta
        .get("sizeBytes")
        .and_then(|v| v.as_u64())
        .or_else(|| meta.get("size_bytes").and_then(|v| v.as_u64()));

    let blob = body["blob"].as_object();
    let sign_url = blob.and_then(|b| b["signUrl"].as_str().or_else(|| b["sign_url"].as_str()));
    let preview_url = read_preview_url(body);

    let (download_url, download_url_source) = match (sign_url, preview_url) {
        (Some(url), _) => (url.to_string(), "blob.signUrl".to_string()),
        (None, Some(url)) => (
            url.to_string(),
            "parseJob.result.image.thumbnail.previewUrl".to_string(),
        ),
        (None, None) => anyhow::bail!(
            "files api response missing blob.signUrl and image preview fallback for {}",
            file_id
        ),
    };

    Ok(KimiFileMetadata {
        file_id: file_id.to_string(),
        id: id.to_string(),
        name: name.to_string(),
        content_type: content_type.to_string(),
        size_bytes,
        download_url,
        download_url_source,
    })
}

fn read_preview_url(body: &serde_json::Value) -> Option<String> {
    let parse_job = body["parseJob"]
        .as_object()
        .or_else(|| body["parse_job"].as_object())?;
    let result = parse_job["result"].as_object()?;
    let image = result["image"].as_object()?;
    let thumbnail = image["thumbnail"].as_object()?;
    thumbnail["previewUrl"]
        .as_str()
        .or_else(|| thumbnail["preview_url"].as_str())
        .map(String::from)
}

/// Ensure a Kimi file is downloaded locally, using cache if available.
///
/// `download_dir` is created if it does not exist. The local file name is
/// `{fileId}_{sanitizedName}`.
pub async fn ensure_downloaded(
    metadata: &KimiFileMetadata,
    download_dir: &Path,
    client: &reqwest::Client,
) -> Result<KimiFileDownload> {
    std::fs::create_dir_all(download_dir)
        .with_context(|| format!("create kimi file download dir {}", download_dir.display()))?;

    if let Some(existing) = find_existing_download(&metadata.file_id, download_dir) {
        return Ok(existing);
    }

    let local_file_name = format!(
        "{}_{}",
        metadata.file_id,
        sanitize_kimi_file_name(&metadata.name)
    );
    let local_path = download_dir.join(&local_file_name);

    let resp = client
        .get(&metadata.download_url)
        .send()
        .await
        .with_context(|| format!("download file {}", metadata.file_id))?;

    let status = resp.status();
    if !status.is_success() {
        anyhow::bail!(
            "file download failed for {}: status {}",
            metadata.file_id,
            status
        );
    }

    let bytes = resp
        .bytes()
        .await
        .with_context(|| format!("read download body for {}", metadata.file_id))?;

    if bytes.is_empty() {
        anyhow::bail!("downloaded file payload is empty for {}", metadata.file_id);
    }

    tokio::fs::write(&local_path, &bytes)
        .await
        .with_context(|| format!("write downloaded file {}", local_path.display()))?;

    let metadata = tokio::fs::metadata(&local_path)
        .await
        .with_context(|| format!("stat downloaded file {}", local_path.display()))?;

    Ok(KimiFileDownload {
        local_path,
        local_file_name,
        local_size_bytes: metadata.len(),
        local_cache_hit: false,
    })
}

fn find_existing_download(file_id: &str, download_dir: &Path) -> Option<KimiFileDownload> {
    let prefix = format!("{}_", file_id);
    let mut entries: Vec<_> = std::fs::read_dir(download_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with(&prefix))
                .unwrap_or(false)
        })
        .collect();

    entries.sort_by_key(|a| a.file_name());

    for entry in entries {
        let path = entry.path();
        if let Ok(meta) = std::fs::metadata(&path) {
            if meta.is_file() && meta.len() > 0 {
                return Some(KimiFileDownload {
                    local_file_name: path.file_name()?.to_str()?.to_string(),
                    local_path: path,
                    local_size_bytes: meta.len(),
                    local_cache_hit: true,
                });
            }
        }
    }
    None
}

/// Sanitize a Kimi file name for safe local storage.
///
/// Mirrors the JS implementation: remove control chars, path separators,
/// collapse special chars to underscores, trim to 120 chars.
pub fn sanitize_kimi_file_name(name: &str) -> String {
    // First normalize path separators to underscores so that names like
    // "a/b\\c" become "a_b_c" on all platforms.
    let base: String = name
        .chars()
        .map(|c| if c == '\\' || c == '/' { '_' } else { c })
        .collect();
    let base = base.trim();

    let mut out = String::with_capacity(base.len().min(MAX_KIMI_FILE_NAME_LENGTH));
    let mut prev_underscore = false;

    for ch in base.chars() {
        if ch.is_ascii_control() {
            continue;
        }
        if ch.is_alphanumeric() || ch == '.' || ch == '-' || ch == '_' {
            out.push(ch);
            prev_underscore = false;
        } else {
            if !prev_underscore {
                out.push('_');
                prev_underscore = true;
            }
        }
    }

    // Trim leading dots and underscores; trim trailing underscores.
    let trimmed: String = out
        .trim_start_matches(['.', '_'])
        .trim_end_matches('_')
        .to_string();

    if trimmed.is_empty() {
        return "file".to_string();
    }

    trimmed.chars().take(MAX_KIMI_FILE_NAME_LENGTH).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_valid_kimi_file_uri() {
        let uri = "kimi-file://123e4567-e89b-12d3-a456-426614174000";
        assert_eq!(
            parse_kimi_file_uri(uri),
            Some("123e4567-e89b-12d3-a456-426614174000".to_string())
        );
    }

    #[test]
    fn parse_invalid_kimi_file_uri() {
        assert!(parse_kimi_file_uri("kimi-file://not-a-uuid").is_none());
        assert!(parse_kimi_file_uri("https://example.com/file").is_none());
    }

    #[test]
    fn sanitize_name_matches_js_behavior() {
        assert_eq!(
            sanitize_kimi_file_name("hello world.txt"),
            "hello_world.txt"
        );
        assert_eq!(sanitize_kimi_file_name("a/b\\c"), "a_b_c");
        assert_eq!(sanitize_kimi_file_name("..secret"), "secret");
        assert_eq!(
            sanitize_kimi_file_name(&"a".repeat(200)),
            "a".repeat(MAX_KIMI_FILE_NAME_LENGTH)
        );
        assert_eq!(sanitize_kimi_file_name(""), "file");
    }

    #[test]
    fn parse_metadata_response_extracts_sign_url() {
        let body = serde_json::json!({
            "id": "f1",
            "meta": { "name": "doc.pdf", "contentType": "application/pdf", "sizeBytes": 1234 },
            "blob": { "signUrl": "https://cdn.example.com/f1?sig=abc" }
        });
        let meta = parse_metadata_response("f1", &body).unwrap();
        assert_eq!(meta.name, "doc.pdf");
        assert_eq!(meta.content_type, "application/pdf");
        assert_eq!(meta.size_bytes, Some(1234));
        assert_eq!(meta.download_url, "https://cdn.example.com/f1?sig=abc");
        assert_eq!(meta.download_url_source, "blob.signUrl");
    }

    #[test]
    fn parse_metadata_response_falls_back_to_preview_url() {
        let body = serde_json::json!({
            "id": "f2",
            "meta": { "name": "img.png", "contentType": "image/png" },
            "parseJob": {
                "result": {
                    "image": {
                        "thumbnail": { "previewUrl": "https://cdn.example.com/f2-thumb" }
                    }
                }
            }
        });
        let meta = parse_metadata_response("f2", &body).unwrap();
        assert_eq!(meta.download_url, "https://cdn.example.com/f2-thumb");
    }

    #[test]
    fn find_existing_download_hits_cache() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("fid_hello.txt");
        std::fs::write(&path, "data").unwrap();

        let result = find_existing_download("fid", dir.path()).unwrap();
        assert!(result.local_cache_hit);
        assert_eq!(result.local_size_bytes, 4);
    }
}
