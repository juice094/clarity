//! OKF knowledge graph.
//!
//! Links between concepts are computed on demand by scanning a concept's
//! Markdown body and resolving internal `.md` references to concept ids.

use super::loader::normalize_concept_id;
use super::{OkfBundle, OkfConcept};
use std::path::Path;

/// A directed link between two OKF concepts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OkfLink {
    /// Source concept id.
    pub source: String,
    /// Target concept id after path normalization.
    pub target: String,
    /// Original URL as written in the Markdown body.
    pub url: String,
}

impl OkfBundle {
    /// Return all links originating from `id`.
    ///
    /// Links are computed on demand by scanning the source concept's body.
    pub fn outgoing_links(&self, id: &str) -> Vec<OkfLink> {
        let Some(concept) = self.concepts.get(id) else {
            return Vec::new();
        };
        collect_links(concept)
    }

    /// Return all links pointing to `id`.
    ///
    /// Links are computed on demand by scanning every concept's body.
    pub fn incoming_links(&self, id: &str) -> Vec<OkfLink> {
        self.concepts
            .values()
            .flat_map(|concept| {
                collect_links(concept)
                    .into_iter()
                    .filter(|link| link.target == id)
            })
            .collect()
    }
}

/// Collect resolved internal links for a single concept.
fn collect_links(concept: &OkfConcept) -> Vec<OkfLink> {
    let mut links = Vec::new();
    for url in extract_links(&concept.body) {
        if let Some(target) = resolve_link(&concept.id, &url) {
            links.push(OkfLink {
                source: concept.id.clone(),
                target,
                url,
            });
        }
    }
    links
}

/// Extract Markdown links whose destination ends with `.md`.
fn extract_links(body: &str) -> Vec<String> {
    use pulldown_cmark::{Event, Parser, Tag};

    let parser = Parser::new(body);
    parser
        .filter_map(|event| {
            if let Event::Start(Tag::Link { dest_url, .. }) = event {
                let dest = dest_url.to_string();
                if dest.ends_with(".md") || dest.contains(".md#") || dest.contains(".md?") {
                    Some(dest)
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

/// Resolve a Markdown URL relative to the source concept id.
///
/// Only internal `.md` links are resolved. External URLs and fragment-only
/// references are ignored.
fn resolve_link(source_id: &str, url: &str) -> Option<String> {
    let clean = url.split_once('#').map(|(s, _)| s).unwrap_or(url);
    let clean = clean.split_once('?').map(|(s, _)| s).unwrap_or(clean);

    if !clean.ends_with(".md") {
        return None;
    }

    let target = clean.trim_end_matches(".md");
    if target.starts_with('/') {
        return Some(target.trim_start_matches('/').to_string());
    }

    let source_dir = Path::new(source_id).parent()?;
    let resolved = source_dir.join(target);
    Some(normalize_concept_id(
        &resolved.to_string_lossy().replace('\\', "/"),
    ))
}

#[cfg(test)]
mod tests {
    use super::super::{OkfConcept, OkfFrontmatter};
    use super::*;

    fn concept(id: &str, body: &str) -> OkfConcept {
        OkfConcept {
            id: id.to_string(),
            path: std::path::PathBuf::new(),
            frontmatter: OkfFrontmatter {
                r#type: "Concept".to_string(),
                title: Some(id.to_string()),
                ..Default::default()
            },
            body: body.to_string(),
            is_reserved: false,
        }
    }

    fn bundle_with(concepts: Vec<OkfConcept>) -> OkfBundle {
        let mut map = std::collections::HashMap::new();
        for concept in concepts {
            map.insert(concept.id.clone(), concept);
        }
        OkfBundle {
            root: std::path::PathBuf::new(),
            concepts: map,
            warnings: Vec::new(),
        }
    }

    #[test]
    fn test_extract_links() {
        let body = "See [WAU](metrics/wau.md) and [MAU](../metrics/mau.md). Also [external](https://example.com).";
        let links = extract_links(body);
        assert_eq!(links.len(), 2);
        assert!(links.contains(&"metrics/wau.md".to_string()));
        assert!(links.contains(&"../metrics/mau.md".to_string()));
    }

    #[test]
    fn test_resolve_relative_link() {
        assert_eq!(
            resolve_link("tables/events", "./schema.md"),
            Some("tables/schema".to_string())
        );
        assert_eq!(
            resolve_link("references/metrics/wau", "../joins/events.md"),
            Some("references/joins/events".to_string())
        );
    }

    #[test]
    fn test_resolve_absolute_link() {
        assert_eq!(
            resolve_link("tables/events", "/index.md"),
            Some("index".to_string())
        );
    }

    #[test]
    fn test_resolve_ignores_external() {
        assert_eq!(resolve_link("a/b", "https://example.com"), None);
        assert_eq!(resolve_link("a/b", "#section"), None);
    }

    #[test]
    fn test_outgoing_links() {
        let bundle = bundle_with(vec![
            concept("metrics/wau", "# WAU"),
            concept(
                "metrics/mau",
                "See [WAU](./wau.md) for weekly. Also [external](https://example.com).",
            ),
        ]);

        let outgoing = bundle.outgoing_links("metrics/mau");
        assert_eq!(outgoing.len(), 1);
        assert_eq!(outgoing[0].target, "metrics/wau");
        assert_eq!(outgoing[0].source, "metrics/mau");
    }

    #[test]
    fn test_incoming_links() {
        let bundle = bundle_with(vec![
            concept("metrics/wau", "# WAU"),
            concept("metrics/mau", "See [WAU](./wau.md) for weekly."),
        ]);

        let incoming = bundle.incoming_links("metrics/wau");
        assert_eq!(incoming.len(), 1);
        assert_eq!(incoming[0].source, "metrics/mau");
        assert_eq!(incoming[0].target, "metrics/wau");
    }

    #[test]
    fn test_unknown_concept_has_no_links() {
        let bundle = bundle_with(vec![concept("a", "[B](b.md)")]);
        assert!(bundle.outgoing_links("missing").is_empty());
        assert!(bundle.incoming_links("missing").is_empty());
    }
}
