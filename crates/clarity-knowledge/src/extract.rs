//! Document extraction from Markdown and plain text files.

use crate::error::{KnowledgeError, Result};
use pulldown_cmark::{Event, Parser, Tag, TagEnd};
use regex::Regex;
use serde_json::Value;
use std::path::Path;

/// A wikilink found inside a Markdown document.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WikiLink {
    /// Target file or heading reference.
    pub target: String,
    /// Optional display alias.
    pub alias: Option<String>,
    /// Optional heading anchor within the target.
    pub heading: Option<String>,
    /// Optional block identifier within the target.
    pub block_id: Option<String>,
    /// Whether this is an embed (`![[...]]`) rather than a link.
    pub is_embed: bool,
    /// Raw text of the link as it appeared in the source.
    pub raw: String,
}

/// A document extracted from the file system, ready for indexing.
#[derive(Debug, Clone, PartialEq)]
pub struct ExtractedDocument {
    /// Absolute or source-relative path of the file.
    pub path: std::path::PathBuf,
    /// Title derived from frontmatter or first heading.
    pub title: Option<String>,
    /// Raw Markdown content.
    pub content: String,
    /// Parsed YAML frontmatter as JSON value.
    pub frontmatter: Value,
    /// Outgoing wikilinks.
    pub links: Vec<WikiLink>,
    /// Tags found in the document.
    pub tags: Vec<String>,
    /// Top-level headings in document order.
    pub headings: Vec<String>,
}

impl ExtractedDocument {
    /// Create a new empty extracted document for the given path.
    pub fn new(path: impl Into<std::path::PathBuf>) -> Self {
        Self {
            path: path.into(),
            title: None,
            content: String::new(),
            frontmatter: Value::Null,
            links: Vec::new(),
            tags: Vec::new(),
            headings: Vec::new(),
        }
    }
}

/// Extracts structured knowledge from Markdown source text.
#[derive(Debug, Clone)]
pub struct MarkdownExtractor {
    wikilink_re: Regex,
    tag_re: Regex,
}

impl MarkdownExtractor {
    /// Create a new extractor, compiling the internal regexes.
    ///
    /// # Errors
    ///
    /// Returns an error if the built-in wikilink or tag regex fails to compile.
    /// This should not happen in practice because the patterns are static.
    pub fn new() -> Result<Self> {
        let wikilink_re =
            Regex::new(r"!?\[\[([^\]|]+?)(?:#([^\]|^]+))?(?:\^([^\]|]+))?(?:\|([^\]]+))?\]\]")
                .map_err(|e| {
                    KnowledgeError::Io(std::io::Error::other(format!("wikilink regex: {e}")))
                })?;
        let tag_re = Regex::new(r"#([a-zA-Z0-9_\-\u{4e00}-\u{9fff}]+)")
            .map_err(|e| KnowledgeError::Io(std::io::Error::other(format!("tag regex: {e}"))))?;

        Ok(Self {
            wikilink_re,
            tag_re,
        })
    }

    /// Extract a document from raw Markdown text.
    ///
    /// The `path` argument is stored as-is in the returned document and is used
    /// only for error reporting and relative link resolution.
    pub fn extract(&self, path: &Path, content: &str) -> Result<ExtractedDocument> {
        let (frontmatter_yaml, body) = split_frontmatter(content);

        let frontmatter = match frontmatter_yaml {
            Some(yaml) => match serde_yaml::from_str(yaml) {
                Ok(v) => v,
                Err(e) => {
                    tracing::warn!(
                        "Failed to parse frontmatter in {:?}: {}; indexing body only",
                        path,
                        e
                    );
                    Value::Null
                }
            },
            None => Value::Null,
        };

        let title = extract_title(&frontmatter, body);
        let headings = extract_headings(body);
        let links = self.extract_wikilinks(body);
        let tags = self.extract_tags(body, &frontmatter);

        Ok(ExtractedDocument {
            path: path.to_path_buf(),
            title,
            content: content.to_string(),
            frontmatter,
            links,
            tags,
            headings,
        })
    }
}

/// Split content into optional YAML frontmatter and body.
pub(crate) fn split_frontmatter(content: &str) -> (Option<&str>, &str) {
    if !content.starts_with("---") {
        return (None, content);
    }

    let after_open = &content[3..];
    let Some(end) = after_open.find("\n---") else {
        return (None, content);
    };

    // Ensure the closing --- is at line start.
    let yaml = &after_open[..end];
    let body = &after_open[end + 4..];
    (Some(yaml), body)
}

/// Extract title from frontmatter `title` field or the first level-1 heading.
fn extract_title(frontmatter: &Value, body: &str) -> Option<String> {
    if let Some(t) = frontmatter.get("title").and_then(|v| v.as_str()) {
        return Some(t.to_string());
    }

    let parser = Parser::new(body);
    let mut in_heading = false;
    for event in parser {
        match event {
            Event::Start(Tag::Heading {
                level: pulldown_cmark::HeadingLevel::H1,
                ..
            }) => in_heading = true,
            Event::End(TagEnd::Heading(pulldown_cmark::HeadingLevel::H1)) => in_heading = false,
            Event::Text(text) if in_heading => return Some(text.to_string()),
            _ => {}
        }
    }
    None
}

/// Extract all headings from the document body.
fn extract_headings(body: &str) -> Vec<String> {
    let parser = Parser::new(body);
    let mut headings = Vec::new();
    let mut current = String::new();
    let mut in_heading = false;

    for event in parser {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                in_heading = true;
                current.clear();
            }
            Event::End(TagEnd::Heading(_)) => {
                in_heading = false;
                if !current.is_empty() {
                    headings.push(current.trim().to_string());
                }
            }
            Event::Text(text) if in_heading => current.push_str(&text),
            Event::Code(code) if in_heading => current.push_str(&code),
            _ => {}
        }
    }

    headings
}

impl MarkdownExtractor {
    /// Extract wikilinks from Markdown text.
    fn extract_wikilinks(&self, body: &str) -> Vec<WikiLink> {
        let mut links = Vec::new();

        for cap in self.wikilink_re.captures_iter(body) {
            let full_match = cap
                .get(0)
                .map(|m| m.as_str().to_string())
                .unwrap_or_default();
            let is_embed = full_match.starts_with('!');
            let target = cap[1].trim().to_string();
            let heading = cap.get(2).map(|m| m.as_str().trim().to_string());
            let block_id = cap.get(3).map(|m| m.as_str().trim().to_string());
            let alias = cap.get(4).map(|m| m.as_str().trim().to_string());

            // Obsidian allows [[#heading]] for same-file references.
            let target = if target.is_empty() && heading.is_some() {
                String::new()
            } else {
                target
            };

            links.push(WikiLink {
                target,
                alias,
                heading,
                block_id,
                is_embed,
                raw: full_match,
            });
        }

        links
    }

    /// Extract tags from body text and frontmatter `tags` field.
    fn extract_tags(&self, body: &str, frontmatter: &Value) -> Vec<String> {
        let mut tags: Vec<String> = self
            .tag_re
            .captures_iter(body)
            .map(|cap| cap[1].to_string())
            .collect();

        if let Some(arr) = frontmatter.get("tags").and_then(|v| v.as_array()) {
            for v in arr {
                if let Some(s) = v.as_str() {
                    tags.push(s.to_string());
                }
            }
        }

        tags.sort_unstable();
        tags.dedup();
        tags
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_simple_note() {
        let content = r#"---
title: Test Note
tags: [rust, ai]
---
# Heading A
This is a [[Second Note|alias]] and an embed ![[image.png]].
#tag1 #tag2
"#;

        let extractor = MarkdownExtractor::new().unwrap();
        let doc = extractor
            .extract(Path::new("/tmp/test.md"), content)
            .unwrap();

        assert_eq!(doc.title.as_deref(), Some("Test Note"));
        assert_eq!(doc.tags, vec!["ai", "rust", "tag1", "tag2"]);
        assert_eq!(doc.headings, vec!["Heading A"]);
        assert_eq!(doc.links.len(), 2);
        assert_eq!(
            doc.links[0],
            WikiLink {
                target: "Second Note".to_string(),
                alias: Some("alias".to_string()),
                heading: None,
                block_id: None,
                is_embed: false,
                raw: "[[Second Note|alias]]".to_string(),
            }
        );
        assert_eq!(
            doc.links[1],
            WikiLink {
                target: "image.png".to_string(),
                alias: None,
                heading: None,
                block_id: None,
                is_embed: true,
                raw: "![[image.png]]".to_string(),
            }
        );
    }

    #[test]
    fn extract_without_frontmatter() {
        let content = "# Hello\nSome text.";
        let extractor = MarkdownExtractor::new().unwrap();
        let doc = extractor.extract(Path::new("note.md"), content).unwrap();

        assert_eq!(doc.title.as_deref(), Some("Hello"));
        assert!(doc.frontmatter.is_null());
    }

    #[test]
    fn extract_with_invalid_frontmatter_indexes_body() {
        // Regression: malformed YAML frontmatter should not fail the whole file;
        // the body should still be indexed.
        let content = "---\ndate: { not: a: string }\n---\n# Hello\nSome text.";
        let extractor = MarkdownExtractor::new().unwrap();
        let doc = extractor.extract(Path::new("note.md"), content).unwrap();

        assert!(doc.frontmatter.is_null());
        assert_eq!(doc.title.as_deref(), Some("Hello"));
        assert_eq!(doc.content, content);
    }
}
