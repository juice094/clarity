//! Key-reference resolution utilities.
//!
//! Supports `${env:VAR}`, `${file:path:field}`, and plain-string fallbacks.

#![cfg_attr(test, allow(unsafe_code))]

use std::path::PathBuf;

/// Expand a key reference string.
///
/// Supported syntax:
/// - `${file:path:field}` — read `field` from JSON file at `path` (`~` is expanded).
/// - `${env:VAR}` — read environment variable `VAR`.
/// - plain string — treated as an env-var name for backward compat, or returned as-is.
pub fn resolve_key_ref(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return None;
    }

    // ${file:path:field}
    if let Some(inner) = raw
        .strip_prefix("${file:")
        .and_then(|s| s.strip_suffix('}'))
    {
        // Use rsplit_once so Windows absolute paths (e.g. C:\...) work:
        // the last ':' separates the path from the JSON field name.
        let (path_part, field) = inner.rsplit_once(':')?;

        let path = if path_part.starts_with("~/") {
            dirs::home_dir()
                .map(|h| h.join(&path_part[2..]))
                .unwrap_or_else(|| PathBuf::from(path_part))
        } else {
            PathBuf::from(path_part)
        };
        let content = std::fs::read_to_string(&path).ok()?;
        let json: serde_json::Value = serde_json::from_str(&content).ok()?;
        return json
            .get(field)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
    }

    // ${env:VAR}
    if let Some(var) = raw.strip_prefix("${env:").and_then(|s| s.strip_suffix('}')) {
        return std::env::var(var).ok();
    }

    // Try env var, fall back to literal
    std::env::var(raw).ok().or_else(|| Some(raw.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn empty_returns_none() {
        assert_eq!(resolve_key_ref(""), None);
        assert_eq!(resolve_key_ref("   "), None);
    }

    #[test]
    fn env_ref_when_set() {
        // SAFE: test-only env var setup; no concurrent reads of this name.
        unsafe { std::env::set_var("RESOLVE_TEST_KEY", "secret123") };
        assert_eq!(
            resolve_key_ref("${env:RESOLVE_TEST_KEY}"),
            Some("secret123".into())
        );
        // SAFE: test-only env var cleanup.
        unsafe { std::env::remove_var("RESOLVE_TEST_KEY") };
    }

    #[test]
    fn env_ref_when_missing() {
        // Use a name that is extremely unlikely to exist.
        assert_eq!(resolve_key_ref("${env:RESOLVE_TEST_MISSING_XYZ}"), None);
    }

    #[test]
    fn file_ref_absolute_path() {
        let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
        write!(tmp, r#"{{"api_key": "file-secret"}}"#).unwrap();
        let path = tmp.path().to_string_lossy();
        assert_eq!(
            resolve_key_ref(&format!("${{file:{path}:api_key}}")),
            Some("file-secret".into())
        );
    }

    #[test]
    fn file_ref_missing_field_returns_none() {
        let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
        write!(tmp, r#"{{"other": "x"}}"#).unwrap();
        let path = tmp.path().to_string_lossy();
        assert_eq!(resolve_key_ref(&format!("${{file:{path}:api_key}}")), None);
    }

    #[test]
    fn plain_string_literal() {
        assert_eq!(resolve_key_ref("sk-mykey"), Some("sk-mykey".into()));
    }

    #[test]
    fn plain_string_env_fallback() {
        // SAFE: test-only env var setup; no concurrent reads of this name.
        unsafe { std::env::set_var("RESOLVE_PLAIN_FALLBACK", "plain-secret") };
        assert_eq!(
            resolve_key_ref("RESOLVE_PLAIN_FALLBACK"),
            Some("plain-secret".into())
        );
        // SAFE: test-only env var cleanup.
        unsafe { std::env::remove_var("RESOLVE_PLAIN_FALLBACK") };
    }
}
