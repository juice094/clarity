//! Line-buffering writer that redacts credentials from tracing output.
//!
//! Wraps any [`std::io::Write`] implementation and applies the same regex
//! patterns used by [`scrub_credentials`] before forwarding bytes.  Buffers
//! partial lines until a newline (or flush/drop) so that multi-byte UTF-8
//! sequences and cross-chunk credential tokens are fully captured.

use regex::Regex;
use std::io::{self, Write};
use std::sync::LazyLock;

/// Pre-compiled regex patterns for credential scrubbing.
///
/// Matches:
/// * `api_key=…` / `api-key: …`
/// * `token=…` / `token: …`
/// * `password=…` / `password: …`
/// * OpenAI-style `sk-…` keys
/// * Google-style `AIza…` keys
static CREDENTIAL_PATTERNS: LazyLock<Vec<Regex>> = LazyLock::new(|| {
    let patterns = [
        // api_key / api-key (case-insensitive, optional quotes)
        r#"(?i)api[_-]?key\s*[:=]\s*["']?[a-zA-Z0-9_\-]{16,}["']?"#,
        // token (case-insensitive, optional quotes)
        r#"(?i)token\s*[:=]\s*["']?[a-zA-Z0-9_\-]{16,}["']?"#,
        // password (case-insensitive, optional quotes)
        r#"(?i)password\s*[:=]\s*["']?[^"'\s]{8,}["']?"#,
        // OpenAI secret key
        r"sk-[a-zA-Z0-9]{20,}",
        // Google API key
        r"AIza[a-zA-Z0-9_\-]{30,}",
    ];
    patterns.iter().filter_map(|p| Regex::new(p).ok()).collect()
});

/// Replace every credential match with `[REDACTED]`.
fn scrub_credentials(text: &str) -> String {
    let mut result = text.to_string();
    for re in CREDENTIAL_PATTERNS.iter() {
        result = re.replace_all(&result, "[REDACTED]").to_string();
    }
    result
}

/// A [`Write`] wrapper that buffers by line and redacts credentials.
///
/// `tracing_subscriber::fmt` writes partial records across multiple
/// `write()` calls; we accumulate bytes until a newline (or explicit flush)
/// so that a credential token split across two writes is still caught.
pub struct RedactingWriter<W: Write> {
    inner: W,
    buf: Vec<u8>,
}

impl<W: Write> RedactingWriter<W> {
    /// Create a new instance.
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            buf: Vec::with_capacity(256),
        }
    }

    fn flush_buf(&mut self) -> io::Result<()> {
        if self.buf.is_empty() {
            return Ok(());
        }
        let text = String::from_utf8_lossy(&self.buf);
        let redacted = scrub_credentials(&text);
        self.inner.write_all(redacted.as_bytes())?;
        self.buf.clear();
        Ok(())
    }
}

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Look for the *last* newline in the incoming chunk so we can flush
        // everything up to and including it, then keep any trailing bytes
        // for the next call.
        match buf.iter().rposition(|&b| b == b'\n') {
            Some(pos) => {
                self.buf.extend_from_slice(&buf[..=pos]);
                self.flush_buf()?;
                if pos + 1 < buf.len() {
                    self.buf.extend_from_slice(&buf[pos + 1..]);
                }
                Ok(buf.len())
            }
            None => {
                self.buf.extend_from_slice(buf);
                Ok(buf.len())
            }
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_buf()?;
        self.inner.flush()
    }
}

impl<W: Write> Drop for RedactingWriter<W> {
    fn drop(&mut self) {
        let _ = self.flush_buf();
    }
}

// ---------------------------------------------------------------------------
// tracing-subscriber integration
// ---------------------------------------------------------------------------

use tracing_subscriber::fmt::writer::MakeWriter;

/// [`MakeWriter`] adapter that produces [`RedactingWriter<StderrLock>`].
///
/// Usage with `tracing_subscriber::fmt::Layer::with_writer`:
/// ```ignore
/// tracing_subscriber::fmt()
///     .with_writer(RedactingStderr)
///     .init();
/// ```
pub struct RedactingStderr;

impl<'a> MakeWriter<'a> for RedactingStderr {
    type Writer = RedactingWriter<io::StderrLock<'a>>;

    fn make_writer(&'a self) -> Self::Writer {
        RedactingWriter::new(io::stderr().lock())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redacting_writer_sk_pattern() {
        let mut buf = Vec::new();
        {
            let mut w = RedactingWriter::new(&mut buf);
            w.write_all(b"Error: token sk-abc12345678901234567890 is invalid\n")
                .unwrap();
        }
        let out = String::from_utf8(buf).unwrap();
        assert_eq!(out, "Error: token [REDACTED] is invalid\n");
    }

    #[test]
    fn test_redacting_writer_api_key_pattern() {
        let mut buf = Vec::new();
        {
            let mut w = RedactingWriter::new(&mut buf);
            w.write_all(b"config: api_key=supersecret1234567890abcdef\n")
                .unwrap();
        }
        let out = String::from_utf8(buf).unwrap();
        assert_eq!(out, "config: [REDACTED]\n");
    }

    #[test]
    fn test_redacting_writer_split_across_writes() {
        let mut buf = Vec::new();
        {
            let mut w = RedactingWriter::new(&mut buf);
            w.write_all(b"Error: token sk-abc123").unwrap();
            w.write_all(b"45678901234567890 is invalid\n").unwrap();
        }
        let out = String::from_utf8(buf).unwrap();
        assert_eq!(out, "Error: token [REDACTED] is invalid\n");
    }

    #[test]
    fn test_redacting_writer_no_match() {
        let mut buf = Vec::new();
        {
            let mut w = RedactingWriter::new(&mut buf);
            w.write_all(b"Hello world, nothing sensitive here.\n")
                .unwrap();
        }
        let out = String::from_utf8(buf).unwrap();
        assert_eq!(out, "Hello world, nothing sensitive here.\n");
    }

    #[test]
    fn test_redacting_writer_drop_flushes_remainder() {
        let mut buf = Vec::new();
        {
            let mut w = RedactingWriter::new(&mut buf);
            w.write_all(b"secret=shh-no-newline").unwrap();
            // drop should flush the unterminated buffer
        }
        let out = String::from_utf8(buf).unwrap();
        assert_eq!(out, "secret=shh-no-newline");
    }
}
