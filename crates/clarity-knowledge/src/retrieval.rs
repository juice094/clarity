//! Hybrid retrieval for the knowledge index.
//!
//! Combines BM25 keyword scoring, TF-IDF cosine similarity, and graph neighbor
//! boosting. Supports a small subset of Obsidian-style search operators.

use crate::error::Result;
use crate::extract::ExtractedDocument;
use crate::graph::{KnowledgeGraph, NodeId};
use crate::search::{SearchQuery, SearchResult};
use clarity_memory::bm25::IncrementalBm25Index;
use clarity_memory::embedding::{CosineIndex, TfidfVectorizer};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

/// A parsed search query with optional filters.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParsedQuery {
    /// Free-text terms after removing operators.
    pub text: String,
    /// Filter by tag ( Obsidian `tag:` operator).
    pub tag_filter: Option<String>,
    /// Filter by path substring ( Obsidian `path:` operator).
    pub path_filter: Option<String>,
    /// Filter by file name ( Obsidian `file:` operator).
    pub file_filter: Option<String>,
}

impl ParsedQuery {
    /// Parse a raw query string into structured filters and text.
    pub fn parse(raw: &str) -> Self {
        let mut text_parts = Vec::new();
        let mut tag_filter = None;
        let mut path_filter = None;
        let mut file_filter = None;

        for token in raw.split_whitespace() {
            if let Some(value) = token.strip_prefix("tag:") {
                tag_filter = Some(value.to_lowercase());
            } else if let Some(value) = token.strip_prefix("path:") {
                path_filter = Some(value.to_lowercase());
            } else if let Some(value) = token.strip_prefix("file:") {
                file_filter = Some(value.to_lowercase());
            } else {
                text_parts.push(token);
            }
        }

        Self {
            text: text_parts.join(" "),
            tag_filter,
            path_filter,
            file_filter,
        }
    }
}

/// Hybrid retriever using BM25 + cosine similarity + graph boost.
#[derive(Debug)]
pub struct HybridRetriever {
    /// Maps document index to file path.
    doc_paths: Vec<PathBuf>,
    /// Maps file path to document index.
    path_to_idx: HashMap<PathBuf, usize>,
    /// BM25 index for keyword scoring.
    bm25: IncrementalBm25Index,
    /// TF-IDF vectorizer.
    vectorizer: TfidfVectorizer,
    /// Whether the cosine index needs rebuilding.
    vector_dirty: bool,
    /// Cached cosine index.
    cosine_index: Option<CosineIndex>,
    /// Documents indexed by path.
    documents: HashMap<PathBuf, ExtractedDocument>,
}

impl HybridRetriever {
    /// Create an empty hybrid retriever.
    pub fn new() -> Self {
        Self {
            doc_paths: Vec::new(),
            path_to_idx: HashMap::new(),
            bm25: IncrementalBm25Index::new(),
            vectorizer: TfidfVectorizer::new(),
            vector_dirty: true,
            cosine_index: None,
            documents: HashMap::new(),
        }
    }

    /// Index or re-index a single document.
    pub fn add_document(&mut self, doc: ExtractedDocument) -> Result<()> {
        let path = doc.path.clone();
        let text = document_search_text(&doc);

        if let Some(&idx) = self.path_to_idx.get(&path) {
            // Re-index: remove old, add new.
            self.bm25.remove_document(idx);
        } else {
            let new_idx = self.doc_paths.len();
            self.doc_paths.push(path.clone());
            self.path_to_idx.insert(path.clone(), new_idx);
        }

        self.bm25.add_document(&text);
        self.documents.insert(path, doc);
        self.vector_dirty = true;
        Ok(())
    }

    /// Remove a document from the index.
    pub fn remove_document(&mut self, path: &Path) -> Result<()> {
        if let Some(&idx) = self.path_to_idx.get(path) {
            self.bm25.remove_document(idx);
            self.documents.remove(path);
            self.vector_dirty = true;
        }
        Ok(())
    }

    /// Execute a hybrid search.
    pub fn search(
        &mut self,
        query: &SearchQuery,
        graph: &KnowledgeGraph,
    ) -> Result<Vec<SearchResult>> {
        let parsed = ParsedQuery::parse(&query.text);

        // Build candidate set from filters.
        let candidates: HashSet<PathBuf> = self
            .documents
            .values()
            .filter(|doc| matches_filters(doc, &parsed))
            .map(|doc| doc.path.clone())
            .collect();

        if candidates.is_empty() && parsed.text.is_empty() {
            return Ok(Vec::new());
        }

        // Score candidates with BM25.
        let mut scores: HashMap<PathBuf, f64> = HashMap::new();
        let mut direct_hits: HashSet<PathBuf> = HashSet::new();
        if !parsed.text.is_empty() {
            for path in &candidates {
                let idx = self.path_to_idx[path];
                let bm25_score = self.bm25.score(&parsed.text, idx);
                if bm25_score > 0.0 {
                    *scores.entry(path.clone()).or_insert(0.0) += f64::from(bm25_score);
                    direct_hits.insert(path.clone());
                }
            }
        }

        // Score candidates with cosine similarity.
        if !parsed.text.is_empty() {
            self.ensure_cosine_index();
            if let Some(index) = &self.cosine_index {
                let results = index.search(&parsed.text, candidates.len().max(1));
                for (doc_text, similarity) in results {
                    if similarity <= 0.0 {
                        continue;
                    }
                    if let Some(path) = self.find_path_by_text(&doc_text) {
                        if candidates.contains(&path) {
                            *scores.entry(path.clone()).or_insert(0.0) +=
                                f64::from(similarity) * 2.0;
                            direct_hits.insert(path);
                        }
                    }
                }
            }
        }

        // Title boost for exact substring matches.
        let text_lower = parsed.text.to_lowercase();
        for path in &candidates {
            if let Some(doc) = self.documents.get(path) {
                if let Some(title) = &doc.title {
                    if title.to_lowercase().contains(&text_lower) {
                        *scores.entry(path.clone()).or_insert(0.0) += 5.0;
                        direct_hits.insert(path.clone());
                    }
                }
            }
        }

        // Graph neighbor boost.
        let mut neighbor_scores: HashMap<PathBuf, f64> = HashMap::new();
        if query.include_graph_neighbors {
            for (path, score) in &scores {
                if let Some(neighbors) = graph_neighbors(graph, path) {
                    for neighbor in neighbors {
                        if !scores.contains_key(&neighbor) {
                            *neighbor_scores.entry(neighbor).or_insert(0.0) += score * 0.3;
                        }
                    }
                }
            }
        }

        for (path, score) in neighbor_scores {
            *scores.entry(path).or_insert(0.0) += score;
        }

        // Build results.
        let mut results: Vec<SearchResult> = scores
            .into_iter()
            .filter_map(|(path, score)| {
                let doc = self.documents.get(&path)?;
                let graph_distance = if direct_hits.contains(&path) { 0 } else { 1 };
                Some(SearchResult {
                    path,
                    title: doc.title.clone(),
                    snippet: make_snippet(&doc.content, &text_lower),
                    score,
                    matched_tags: matched_tags(doc, &parsed.text),
                    graph_distance,
                })
            })
            .collect();

        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(query.limit);
        Ok(results)
    }

    fn ensure_cosine_index(&mut self) {
        if !self.vector_dirty && self.cosine_index.is_some() {
            return;
        }

        let texts: Vec<String> = self
            .doc_paths
            .iter()
            .map(|p| {
                self.documents
                    .get(p)
                    .map(document_search_text)
                    .unwrap_or_default()
            })
            .collect();

        self.vectorizer.fit(&texts);
        self.cosine_index = Some(CosineIndex::new(&self.vectorizer, &texts));
        self.vector_dirty = false;
    }

    fn find_path_by_text(&self, text: &str) -> Option<PathBuf> {
        self.doc_paths
            .iter()
            .find(|p| {
                self.documents
                    .get(*p)
                    .map(document_search_text)
                    .map(|t| t == text)
                    .unwrap_or(false)
            })
            .cloned()
    }

    /// Return the extracted document for a path, if indexed.
    pub fn get_document(&self, path: &Path) -> Option<&ExtractedDocument> {
        self.documents.get(path)
    }
}

impl Default for HybridRetriever {
    fn default() -> Self {
        Self::new()
    }
}

fn document_search_text(doc: &ExtractedDocument) -> String {
    let mut parts = Vec::new();
    if let Some(title) = &doc.title {
        parts.push(title.clone());
        parts.push(title.clone());
    }
    parts.push(doc.content.clone());
    parts.extend(doc.tags.iter().cloned());
    parts.join(" ")
}

fn matches_filters(doc: &ExtractedDocument, query: &ParsedQuery) -> bool {
    if let Some(tag) = &query.tag_filter {
        if !doc.tags.iter().any(|t| t.to_lowercase().contains(tag)) {
            return false;
        }
    }

    if let Some(path) = &query.path_filter {
        if !doc.path.to_string_lossy().to_lowercase().contains(path) {
            return false;
        }
    }

    if let Some(file) = &query.file_filter {
        let name = doc
            .path
            .file_stem()
            .map(|s| s.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        if !name.contains(file) {
            return false;
        }
    }

    true
}

fn graph_neighbors(graph: &KnowledgeGraph, path: &Path) -> Option<Vec<PathBuf>> {
    let node_id = NodeId::new(path.to_string_lossy());
    let nodes = graph.neighbors(&node_id)?;
    Some(
        nodes
            .iter()
            .filter_map(|n| {
                if n.kind == crate::graph::NodeKind::File {
                    Some(PathBuf::from(&n.id.0))
                } else {
                    None
                }
            })
            .collect(),
    )
}

fn make_snippet(content: &str, query: &str) -> String {
    let lower = content.to_lowercase();
    if let Some(byte_pos) = lower.find(query) {
        // The match position is a byte index in the lowercased string, which
        // may not align with byte boundaries in the original content (e.g.
        // multi-byte characters like Chinese punctuation). Work in character
        // indices so slicing is always valid.
        let match_start_char = lower[..byte_pos].chars().count();
        let match_end_char = match_start_char + query.chars().count();
        let start = match_start_char.saturating_sub(80);
        let end = (match_end_char + 120).min(content.chars().count());
        content.chars().skip(start).take(end - start).collect()
    } else {
        content.chars().take(200).collect()
    }
}

fn matched_tags(doc: &ExtractedDocument, query: &str) -> Vec<String> {
    let q = query.to_lowercase();
    doc.tags
        .iter()
        .filter(|t| t.to_lowercase().contains(&q))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn doc(path: &str, title: &str, content: &str, tags: &[&str]) -> ExtractedDocument {
        ExtractedDocument {
            path: PathBuf::from(path),
            title: Some(title.to_string()),
            content: content.to_string(),
            frontmatter: serde_json::Value::Null,
            links: Vec::new(),
            tags: tags.iter().map(|t| t.to_string()).collect(),
            headings: Vec::new(),
        }
    }

    #[test]
    fn parse_operators() {
        let q = ParsedQuery::parse("rust async tag:programming path:projects");
        assert_eq!(q.text, "rust async");
        assert_eq!(q.tag_filter, Some("programming".to_string()));
        assert_eq!(q.path_filter, Some("projects".to_string()));
    }

    #[test]
    fn hybrid_search_finds_by_tag_and_text() {
        let mut retriever = HybridRetriever::new();
        retriever
            .add_document(doc(
                "rust.md",
                "Rust",
                "Rust is a systems programming language.",
                &["programming"],
            ))
            .unwrap();
        retriever
            .add_document(doc(
                "python.md",
                "Python",
                "Python is great for data science.",
                &["programming"],
            ))
            .unwrap();
        retriever
            .add_document(doc(
                "cooking.md",
                "Cooking",
                "Italian recipes are delicious.",
                &["food"],
            ))
            .unwrap();

        let query = SearchQuery::new("programming rust").with_limit(5);
        let results = retriever.search(&query, &KnowledgeGraph::new()).unwrap();

        assert!(!results.is_empty());
        assert_eq!(results[0].path, PathBuf::from("rust.md"));
    }

    #[test]
    fn tag_operator_filters() {
        let mut retriever = HybridRetriever::new();
        retriever
            .add_document(doc("a.md", "A", "content", &["rust"]))
            .unwrap();
        retriever
            .add_document(doc("b.md", "B", "content", &["python"]))
            .unwrap();

        let query = SearchQuery::new("tag:rust").with_limit(5);
        let results = retriever.search(&query, &KnowledgeGraph::new()).unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].path, PathBuf::from("a.md"));
    }

    #[test]
    fn make_snippet_handles_multibyte_characters() {
        // Regression: searching near a multi-byte character used to panic because
        // byte indices from the lowercased string were applied to the original.
        let content = "前言：这是一些中文内容，用于测试搜索摘要功能。";
        let snippet = make_snippet(content, "中文内容");
        assert!(snippet.contains("中文内容"));
    }
}
