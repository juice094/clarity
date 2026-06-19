//! OKF knowledge graph.
//!
//! A graph is built from an [`OkfBundle`] by extracting Markdown links from
//! each concept's body and resolving them to concept ids.

use super::loader::normalize_concept_id;
use super::{OkfBundle, OkfConcept};
use std::collections::HashMap;
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

/// Traversable view over a loaded OKF bundle.
#[derive(Debug, Clone)]
pub struct OkfGraph {
    /// Concepts indexed by id.
    pub concepts: HashMap<String, OkfConcept>,
    /// Directed links between concepts.
    pub links: Vec<OkfLink>,
}

impl OkfGraph {
    /// Build a graph from a bundle.
    pub fn from_bundle(bundle: OkfBundle) -> Self {
        let mut links = Vec::new();
        for concept in bundle.concepts.values() {
            for url in extract_links(&concept.body) {
                if let Some(target) = resolve_link(&concept.id, &url) {
                    links.push(OkfLink {
                        source: concept.id.clone(),
                        target,
                        url,
                    });
                }
            }
        }
        Self {
            concepts: bundle.concepts,
            links,
        }
    }

    /// Look up a concept by id.
    pub fn get(&self, id: &str) -> Option<&OkfConcept> {
        self.concepts.get(id)
    }

    /// Return all links originating from `id`.
    pub fn outgoing(&self, id: &str) -> Vec<&OkfLink> {
        self.links.iter().filter(|l| l.source == id).collect()
    }

    /// Return all links pointing to `id`.
    pub fn incoming(&self, id: &str) -> Vec<&OkfLink> {
        self.links.iter().filter(|l| l.target == id).collect()
    }

    /// Return the ids of concepts reachable directly from `id`.
    pub fn neighbors(&self, id: &str) -> Vec<&OkfConcept> {
        self.outgoing(id)
            .iter()
            .filter_map(|l| self.concepts.get(&l.target))
            .collect()
    }

    /// Return the number of concepts in the graph.
    pub fn len(&self) -> usize {
        self.concepts.len()
    }

    /// Check if the graph contains no concepts.
    pub fn is_empty(&self) -> bool {
        self.concepts.is_empty()
    }

    /// Convert the graph back into a bundle.
    pub fn into_bundle(self) -> OkfBundle {
        OkfBundle {
            root: std::path::PathBuf::new(),
            concepts: self.concepts,
            warnings: Vec::new(),
        }
    }
}

impl OkfBundle {
    /// Build a traversable [`OkfGraph`] from this bundle.
    pub fn into_graph(self) -> OkfGraph {
        OkfGraph::from_bundle(self)
    }
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
    fn test_graph_neighbors() {
        let mut concepts = HashMap::new();
        concepts.insert("metrics/wau".to_string(), concept("metrics/wau", "# WAU"));
        concepts.insert(
            "metrics/mau".to_string(),
            concept("metrics/mau", "See [WAU](./wau.md) for weekly."),
        );
        let _bundle = OkfBundle::new(std::path::PathBuf::new());
        let graph = OkfGraph {
            concepts,
            links: Vec::new(),
        };
        let graph = graph.into_bundle().into_graph();
        let neighbors = graph.neighbors("metrics/mau");
        assert_eq!(neighbors.len(), 1);
        assert_eq!(neighbors[0].id, "metrics/wau");
    }
}
