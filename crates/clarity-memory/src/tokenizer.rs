//! Shared tokenization for BM25 and TF-IDF vectorization.
//!
//! When the `jieba` feature is enabled, Chinese/CJK text is segmented with
//! jieba-rs. English and numeric tokens are still extracted with a regex so
//! mixed-language documents work out of the box.

use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

#[cfg(feature = "jieba")]
#[allow(clippy::unwrap_used)]
static WORD_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[a-zA-Z0-9_\-]+").unwrap());

#[cfg(feature = "jieba")]
#[allow(clippy::unwrap_used)]
static CJK_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\p{Han}").unwrap());

#[cfg(feature = "jieba")]
#[allow(clippy::unwrap_used)]
static JIEBA: LazyLock<jieba_rs::Jieba> = LazyLock::new(jieba_rs::Jieba::new);

/// Tokenize `text` into filtered terms.
///
/// * English/numeric tokens are matched by `[a-zA-Z0-9_\-]+`.
/// * When the `jieba` feature is enabled, CJK runs are segmented by jieba and
///   the resulting words are kept as tokens.
/// * Stop words and tokens shorter than two bytes are dropped.
pub fn tokenize(text: &str, stop_words: &HashSet<String>) -> Vec<String> {
    let lowered = text.to_lowercase();

    #[cfg(feature = "jieba")]
    {
        let mut tokens = Vec::new();
        for raw in JIEBA.cut(&lowered, false) {
            if raw.is_empty() {
                continue;
            }
            if CJK_RE.is_match(raw) {
                // CJK token: keep it if it is not pure punctuation/whitespace.
                let trimmed = raw.trim_matches(|c: char| {
                    !c.is_alphanumeric() && !CJK_RE.is_match(&c.to_string())
                });
                if !trimmed.is_empty() && !stop_words.contains(trimmed) {
                    tokens.push(trimmed.to_string());
                }
            } else {
                // Non-CJK token: extract word-like sub-tokens.
                for m in WORD_RE.find_iter(raw) {
                    let t = m.as_str();
                    if !stop_words.contains(t) && t.len() > 1 {
                        tokens.push(t.to_string());
                    }
                }
            }
        }
        tokens
    }

    #[cfg(not(feature = "jieba"))]
    {
        // Fallback: match ASCII/Unicode word-like tokens and individual CJK
        // ideographs, just like the original tokenizer.
        #[allow(clippy::unwrap_used)]
        static FALLBACK_RE: LazyLock<Regex> =
            LazyLock::new(|| Regex::new(r"(?:[a-zA-Z0-9_\-]+)|(?:\p{Han})").unwrap());

        FALLBACK_RE
            .find_iter(&lowered)
            .map(|m| m.as_str().to_string())
            .filter(|t| !stop_words.contains(t) && t.len() > 1)
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stop_words() -> HashSet<String> {
        ["the", "a", "is"].iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn english_tokenization() {
        let tokens = tokenize("Rust is a systems programming language", &stop_words());
        assert!(tokens.contains(&"rust".to_string()));
        assert!(tokens.contains(&"programming".to_string()));
        assert!(!tokens.contains(&"is".to_string()));
        assert!(!tokens.contains(&"a".to_string()));
    }

    #[test]
    fn cjk_fallback_tokenization() {
        // Without jieba, CJK characters are emitted one at a time.
        #[cfg(not(feature = "jieba"))]
        {
            let tokens = tokenize("中文笔记内容", &stop_words());
            assert_eq!(tokens, vec!["中", "文", "笔", "记", "内", "容"]);
        }
    }

    #[test]
    #[cfg(feature = "jieba")]
    fn jieba_segments_chinese() {
        let tokens = tokenize("中文笔记内容", &stop_words());
        // jieba should produce multi-character words rather than single chars.
        assert!(
            tokens.iter().any(|t| t.chars().count() >= 2),
            "expected at least one multi-character CJK token, got {:?}",
            tokens
        );
        assert!(tokens.contains(&"中文".to_string()));
    }

    #[test]
    #[cfg(feature = "jieba")]
    fn mixed_language_tokenization() {
        let tokens = tokenize("Rust 编程语言适合系统编程", &stop_words());
        assert!(tokens.contains(&"rust".to_string()));
        assert!(tokens.contains(&"编程语言".to_string()) || tokens.contains(&"编程".to_string()));
        assert!(tokens.contains(&"系统".to_string()) || tokens.contains(&"适合".to_string()));
    }
}
