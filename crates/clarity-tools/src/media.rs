//! Media file reading tool: converts images/videos/audio to base64 for LLM consumption
//!
//! Supports PNG, JPEG, GIF, WebP, BMP, ICO, and common video/audio formats.
//! Integrates sensitive file filtering and MIME type sniffing.
//!
//! ## Decoupling check
//! - Independent module: only depends on `Tool` trait and `file::security` helpers.
//! - Extractable: can be moved to a standalone crate in < half a day.
//! - README: "Read media files as base64 for LLM vision/audio input."

use async_trait::async_trait;
use serde_json::{Value, json};
use std::path::Path;
use tokio::fs;
use tracing::{debug, warn};

use crate::file::{is_sensitive_file, sniff_media_file};
use crate::helpers;
use crate::{Tool, ToolContext, ToolResult};
use clarity_contract::ToolError;

/// Maximum default file size: 5MB (5120 KB)
const DEFAULT_MAX_SIZE_KB: u64 = 5120;

/// Tool for reading media files and returning base64-encoded data
///
/// When the LLM needs to analyze an image, video, or audio file, it uses
/// this tool to read the file and receive base64-encoded data with MIME
/// type information. The tool enforces size limits and rejects sensitive
/// files.
pub struct ReadMediaFileTool;

impl ReadMediaFileTool {
    /// Create a new ReadMediaFileTool instance
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReadMediaFileTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for ReadMediaFileTool {
    fn name(&self) -> &str {
        "read_media"
    }

    fn description(&self) -> &str {
        "Read an image, video, or audio file and return it as base64-encoded data \
         with MIME type information. Useful for analyzing media content or providing \
         visual/audio context. Supports PNG, JPEG, GIF, WebP, BMP, ICO, MP4, WebM, \
         AVI, MP3, WAV, OGG. Max default size: 5MB."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the media file to read"
                },
                "max_size_kb": {
                    "type": "integer",
                    "description": "Maximum file size in KB (default: 5120 = 5MB)",
                    "default": 5120
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> ToolResult<Value> {
        let path_str = helpers::required_str(&args, "path")?;
        let max_size_kb = args
            .get("max_size_kb")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_MAX_SIZE_KB);
        let max_size_bytes = max_size_kb * 1024;

        let path = helpers::resolve_path(&ctx, path_str)?;

        // Security check: reject sensitive files
        if is_sensitive_file(&path) {
            warn!("Blocked read_media on sensitive file: {}", path.display());
            return Err(ToolError::invalid_params(format!(
                "Cannot read sensitive file: '{}'",
                path.display()
            )));
        }

        // Check existence and type
        let metadata = fs::metadata(&path).await.map_err(|e| {
            ToolError::execution_failed(format!(
                "Failed to access file '{}': {}",
                path.display(),
                e
            ))
        })?;

        if !metadata.is_file() {
            return Err(ToolError::invalid_params(format!(
                "'{}' is not a file",
                path.display()
            )));
        }

        let size = metadata.len();
        if size > max_size_bytes {
            return Err(ToolError::invalid_params(format!(
                "File '{}' is {} bytes (max allowed: {} bytes = {} KB)",
                path.display(),
                size,
                max_size_bytes,
                max_size_kb
            )));
        }

        // Determine MIME type via sniffing or extension fallback
        let mime_type = match sniff_media_file(&path).await {
            Some(desc) => mime_from_sniff_description(desc),
            None => guess_mime_from_extension(&path),
        };

        // Read and base64-encode
        let bytes = fs::read(&path).await.map_err(|e| {
            ToolError::execution_failed(format!("Failed to read file '{}': {}", path.display(), e))
        })?;

        let base64_data =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &bytes);
        let size_kb = size as f64 / 1024.0;

        debug!(
            "Read media file: {} ({} bytes, MIME: {})",
            path.display(),
            size,
            mime_type
        );

        Ok(json!({
            "mime_type": mime_type,
            "base64_data": base64_data,
            "size_bytes": size,
            "size_kb": format!("{:.2}", size_kb),
            "description": format!("{} file, {} bytes", mime_type, size)
        }))
    }
}

/// Map sniff description to MIME type string.
fn mime_from_sniff_description(desc: &str) -> &'static str {
    match desc {
        "PNG image" => "image/png",
        "JPEG image" => "image/jpeg",
        "GIF image" => "image/gif",
        "WebP image" => "image/webp",
        "BMP image" => "image/bmp",
        "ICO image" => "image/x-icon",
        "MP4 video" => "video/mp4",
        "WebM video" => "video/webm",
        "AVI video" => "video/x-msvideo",
        "MP3 audio" => "audio/mpeg",
        "WAV audio" => "audio/wav",
        "OGG audio" => "audio/ogg",
        "PDF document" => "application/pdf",
        _ => "application/octet-stream",
    }
}

/// Guess MIME type from file extension as a fallback.
fn guess_mime_from_extension(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("bmp") => "image/bmp",
        Some("ico") => "image/x-icon",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("avi") => "video/x-msvideo",
        Some("mp3") => "audio/mpeg",
        Some("wav") => "audio/wav",
        Some("ogg") => "audio/ogg",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ToolContext;

    #[tokio::test]
    async fn test_read_media_tool_png() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.png");
        // Write a minimal PNG header
        let png_header = [0x89u8, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A];
        tokio::fs::write(&path, &png_header).await.unwrap();

        let tool = ReadMediaFileTool::new();
        let ctx = ToolContext::new().with_working_dir(tmp.path().to_path_buf());
        let args = json!({"path": "test.png"});

        let result = tool.execute(args, ctx).await.unwrap();
        assert_eq!(result["mime_type"].as_str().unwrap(), "image/png");
        assert_eq!(result["size_bytes"].as_u64().unwrap(), 8);
        assert!(!result["base64_data"].as_str().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_read_media_tool_size_limit() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("big.bin");
        tokio::fs::write(&path, vec![0u8; 1024 * 10]).await.unwrap(); // 10KB

        let tool = ReadMediaFileTool::new();
        let ctx = ToolContext::new().with_working_dir(tmp.path().to_path_buf());
        let args = json!({"path": "big.bin", "max_size_kb": 5});

        let result = tool.execute(args, ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("max allowed"));
    }

    #[tokio::test]
    async fn test_read_media_tool_rejects_sensitive() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join(".env");
        tokio::fs::write(&path, "SECRET=xyz").await.unwrap();

        let tool = ReadMediaFileTool::new();
        let ctx = ToolContext::new().with_working_dir(tmp.path().to_path_buf());
        let args = json!({"path": ".env"});

        let result = tool.execute(args, ctx).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("sensitive"));
    }

    #[tokio::test]
    async fn test_read_media_tool_extension_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.jpg");
        // Write non-JPEG data but with .jpg extension
        tokio::fs::write(&path, b"not really a jpeg").await.unwrap();

        let tool = ReadMediaFileTool::new();
        let ctx = ToolContext::new().with_working_dir(tmp.path().to_path_buf());
        let args = json!({"path": "test.jpg"});

        let result = tool.execute(args, ctx).await.unwrap();
        // Falls back to extension-based detection
        assert_eq!(result["mime_type"].as_str().unwrap(), "image/jpeg");
    }
}
