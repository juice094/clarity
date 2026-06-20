//! Open Knowledge Format (OKF) consumer for Clarity.
//!
//! OKF is a vendor-neutral knowledge format published by Google Cloud that
//! represents a knowledge bundle as a directory of Markdown files with YAML
//! frontmatter. This module loads bundles, parses concepts, extracts
//! cross-links, and builds a traversable knowledge graph.
//!
//! See <https://github.com/GoogleCloudPlatform/knowledge-catalog> for the
//! OKF v0.1 specification.

mod cache;
mod graph;
mod loader;

use std::collections::HashMap;

pub use cache::OkfBundleCache;
pub use graph::OkfLink;
pub use loader::{load_bundle, load_concept, parse_concept};

/// Result type for OKF operations.
pub type OkfResult<T> = std::result::Result<T, OkfError>;

/// Errors that can occur when loading or parsing an OKF bundle.
#[derive(Debug, thiserror::Error)]
pub enum OkfError {
    /// I/O error while reading the bundle.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// YAML parsing error in a concept's frontmatter.
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// Frontmatter is malformed or missing.
    #[error("Invalid frontmatter in {path}: {reason}")]
    InvalidFrontmatter {
        /// Concept path that failed validation.
        path: String,
        /// Human-readable reason.
        reason: String,
    },

    /// A non-reserved concept is missing the required `type` field.
    #[error("Missing required 'type' field in concept: {0}")]
    MissingType(String),

    /// Requested concept was not found in the bundle.
    #[error("Concept not found: {0}")]
    NotFound(String),
}

/// Frontmatter metadata for an OKF concept.
///
/// OKF v0.1 requires only `type`. All other fields are recommended and
/// consumers must tolerate their absence. Additional keys are collected in
/// [`OkfFrontmatter::extra`].
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
#[serde(default)]
pub struct OkfFrontmatter {
    /// Concept type. The only required field for non-reserved concepts.
    #[serde(rename = "type")]
    pub r#type: String,

    /// Human-readable title.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,

    /// Short description.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// URI pointing to an authoritative resource (dataset, API, schema, etc.).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource: Option<String>,

    /// Categorical tags. Accepts either a single string or a list of strings.
    #[serde(
        default,
        deserialize_with = "deserialize_tags",
        skip_serializing_if = "Vec::is_empty"
    )]
    pub tags: Vec<String>,

    /// ISO-8601 timestamp for when the concept was authored or updated.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timestamp: Option<String>,

    /// Any additional frontmatter keys not defined by OKF v0.1.
    #[serde(flatten, skip_serializing_if = "HashMap::is_empty")]
    pub extra: HashMap<String, serde_yaml::Value>,
}

/// A single OKF concept.
///
/// Every non-reserved `.md` file in a bundle becomes a concept. Reserved files
/// (`index.md`, `log.md`) are also loaded but do not require a `type`.
#[derive(Debug, Clone)]
pub struct OkfConcept {
    /// Concept identifier: file path within the bundle without the `.md`
    /// extension, using `/` as separator.
    pub id: String,

    /// Absolute filesystem path of the source markdown file.
    pub path: std::path::PathBuf,

    /// Parsed YAML frontmatter.
    pub frontmatter: OkfFrontmatter,

    /// Markdown body (everything after the frontmatter).
    pub body: String,

    /// Whether this concept is a reserved file (`index.md` or `log.md`).
    pub is_reserved: bool,
}

impl OkfConcept {
    /// Build a context string suitable for injection into an LLM prompt.
    ///
    /// Includes the concept title/description header followed by the full
    /// Markdown body.
    pub fn build_context(&self) -> String {
        let header = match (&self.frontmatter.title, &self.frontmatter.description) {
            (Some(title), Some(description)) => format!("# {}\n\n{}\n\n", title, description),
            (Some(title), None) => format!("# {}\n\n", title),
            (None, Some(description)) => format!("# {}\n\n{}\n\n", self.id, description),
            (None, None) => format!("# {}\n\n", self.id),
        };
        format!("{}{}", header, self.body)
    }

    /// One-line summary for listing.
    pub fn summary(&self) -> String {
        let title = self
            .frontmatter
            .title
            .as_deref()
            .unwrap_or(self.id.as_str());
        let type_label = if self.frontmatter.r#type.is_empty() {
            String::new()
        } else {
            format!(" [{}]", self.frontmatter.r#type)
        };
        if let Some(description) = &self.frontmatter.description {
            format!("{} — {}{}", title, description, type_label)
        } else {
            format!("{}{}", title, type_label)
        }
    }
}

/// A loaded OKF knowledge bundle.
#[derive(Debug, Clone)]
pub struct OkfBundle {
    /// Absolute path to the bundle root directory.
    pub root: std::path::PathBuf,

    /// All concepts indexed by their concept id.
    pub concepts: HashMap<String, OkfConcept>,

    /// Non-fatal warnings collected while loading, e.g. files that could not
    /// be parsed as OKF concepts.
    pub warnings: Vec<String>,
}

impl OkfBundle {
    /// Create an empty bundle rooted at the given path.
    pub fn new(root: std::path::PathBuf) -> Self {
        Self {
            root,
            concepts: HashMap::new(),
            warnings: Vec::new(),
        }
    }

    /// Look up a concept by id.
    pub fn get(&self, id: &str) -> Option<&OkfConcept> {
        self.concepts.get(id)
    }

    /// Iterate over all concepts.
    pub fn iter(&self) -> impl Iterator<Item = &OkfConcept> {
        self.concepts.values()
    }

    /// Return concepts whose `type` matches `type_name`.
    pub fn by_type(&self, type_name: &str) -> Vec<&OkfConcept> {
        self.concepts
            .values()
            .filter(|c| c.frontmatter.r#type == type_name)
            .collect()
    }

    /// Search concept ids, titles, descriptions, and tags for `query`
    /// (case-insensitive).
    pub fn search(&self, query: &str) -> Vec<&OkfConcept> {
        let q = query.to_lowercase();
        self.concepts
            .values()
            .filter(|c| {
                c.id.to_lowercase().contains(&q)
                    || c.frontmatter
                        .title
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q)
                    || c.frontmatter
                        .description
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&q)
                    || c.frontmatter
                        .tags
                        .iter()
                        .any(|t| t.to_lowercase().contains(&q))
                    || c.body.to_lowercase().contains(&q)
            })
            .collect()
    }

    /// Return the number of concepts in the bundle.
    pub fn len(&self) -> usize {
        self.concepts.len()
    }

    /// Check if the bundle contains no concepts.
    pub fn is_empty(&self) -> bool {
        self.concepts.is_empty()
    }
}

/// Deserialize `tags` accepting either a single string or a sequence of
/// strings.
fn deserialize_tags<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::{self, SeqAccess, Visitor};
    use std::fmt;

    struct TagsVisitor;

    impl<'de> Visitor<'de> for TagsVisitor {
        type Value = Vec<String>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string or a list of strings")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(vec![value.to_string()])
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: SeqAccess<'de>,
        {
            let mut tags = Vec::new();
            while let Some(tag) = seq.next_element::<String>()? {
                tags.push(tag);
            }
            Ok(tags)
        }
    }

    deserializer.deserialize_any(TagsVisitor)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frontmatter_parsing() {
        let yaml = r#"type: Metric
title: Weekly Active Users
description: Count of distinct users in a 7-day window
tags:
  - metric
  - engagement
"#;
        let meta: OkfFrontmatter = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(meta.r#type, "Metric");
        assert_eq!(meta.title.as_deref().unwrap(), "Weekly Active Users");
        assert_eq!(meta.tags, vec!["metric", "engagement"]);
    }

    #[test]
    fn test_frontmatter_tags_as_string() {
        let yaml = r#"type: Dataset
tags: analytics
"#;
        let meta: OkfFrontmatter = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(meta.tags, vec!["analytics"]);
    }

    #[test]
    fn test_concept_summary() {
        let concept = OkfConcept {
            id: "metrics/wau".to_string(),
            path: std::path::PathBuf::from("/tmp/metrics/wau.md"),
            frontmatter: OkfFrontmatter {
                r#type: "Metric".to_string(),
                title: Some("WAU".to_string()),
                description: Some("Weekly active users".to_string()),
                ..Default::default()
            },
            body: "# Details".to_string(),
            is_reserved: false,
        };
        assert!(concept.summary().contains("WAU"));
        assert!(concept.summary().contains("[Metric]"));
    }
}
