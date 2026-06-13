//! Semantic search over facts using TF-IDF cosine similarity.
//!
//! This module provides a lightweight, fully local semantic retrieval layer
//! that does not depend on external embedding models. It builds a
//! [`TfidfVectorizer`](crate::embedding::TfidfVectorizer) over the fact corpus
//! and ranks candidates by cosine similarity to the query vector.
//!
//! The [`SemanticIndex`] is intended for small-to-medium fact stores. For
//! larger corpora, replace the in-memory TF-IDF index with an approximate
//! nearest-neighbor index backed by dense embeddings.

use crate::embedding::VectorStore;

/// In-memory semantic index for facts.
///
/// Associates each fact id with its text and supports incremental add/remove
/// by rebuilding the lightweight TF-IDF index on each mutation. This is
/// simple and correct for the current local-memory scale; switch to an
/// incremental vectorizer when the corpus grows beyond a few thousand facts.
#[derive(Debug, Default)]
pub struct SemanticIndex {
    vector_store: VectorStore,
    corpus: Vec<(i64, String)>,
}

impl SemanticIndex {
    /// Create an empty semantic index.
    pub fn new() -> Self {
        Self::default()
    }

    /// Rebuild the internal TF-IDF index from `self.corpus`.
    fn rebuild(&mut self) {
        self.vector_store = VectorStore::new();
        self.vector_store.index_facts(&self.corpus);
    }

    /// Add or replace a fact in the index.
    ///
    /// If `id` already exists, the old text is replaced.
    pub fn add_fact(&mut self, id: i64, text: &str) {
        self.corpus.retain(|(fid, _)| *fid != id);
        self.corpus.push((id, text.to_string()));
        self.rebuild();
    }

    /// Remove a fact from the index.
    ///
    /// Returns `true` if the fact was present.
    pub fn remove_fact(&mut self, id: i64) -> bool {
        let before = self.corpus.len();
        self.corpus.retain(|(fid, _)| *fid != id);
        let removed = self.corpus.len() < before;
        if removed {
            self.rebuild();
        }
        removed
    }

    /// Search the index and return the top-k `(fact_id, cosine_score)` pairs.
    ///
    /// Score is in the range `(0.0, 1.0]`; a score of `0.0` means no token
    /// overlap after stop-word filtering and is filtered out.
    pub fn search(&self, query: &str, top_k: usize) -> Vec<(i64, f32)> {
        self.vector_store
            .search(query, top_k)
            .into_iter()
            .map(|(id, _text, score)| (id, score))
            .collect()
    }

    /// Number of facts currently indexed.
    pub fn len(&self) -> usize {
        self.corpus.len()
    }

    /// Whether the index contains no facts.
    pub fn is_empty(&self) -> bool {
        self.corpus.is_empty()
    }
}

/// Build a semantic index from a slice of `(id, text)` fact tuples.
///
/// This is a convenience helper for backends that rebuild the index from
/// scratch (for example on first access or after a cache invalidation).
pub fn build_index(facts: &[(i64, String)]) -> SemanticIndex {
    let mut index = SemanticIndex::new();
    index.corpus = facts.to_vec();
    index.rebuild();
    index
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_semantic_index_basic() {
        let mut index = SemanticIndex::new();
        index.add_fact(1, "User likes Rust programming language");
        index.add_fact(2, "User enjoys Python programming");
        index.add_fact(3, "User has a dog named Max");

        let results = index.search("programming", 2);
        assert_eq!(results.len(), 2);
        let ids: Vec<i64> = results.iter().map(|(id, _)| *id).collect();
        assert!(ids.contains(&1));
        assert!(ids.contains(&2));
    }

    #[test]
    fn test_semantic_index_sorted_by_score() {
        let mut index = SemanticIndex::new();
        index.add_fact(1, "Rust programming language");
        index.add_fact(2, "Python programming");
        index.add_fact(3, "Cooking Italian recipes");

        let results = index.search("programming", 2);
        assert_eq!(results.len(), 2);
        assert!(results[0].1 >= results[1].1);
    }

    #[test]
    fn test_semantic_index_remove_fact() {
        let mut index = SemanticIndex::new();
        index.add_fact(1, "Rust programming");
        index.add_fact(2, "Python programming");

        assert!(index.remove_fact(1));
        let results = index.search("programming", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 2);

        // Removing again is a no-op.
        assert!(!index.remove_fact(1));
    }

    #[test]
    fn test_semantic_index_update_fact() {
        let mut index = SemanticIndex::new();
        index.add_fact(1, "Rust programming");
        index.add_fact(1, "Cooking Italian recipes");

        let results = index.search("programming", 10);
        assert!(results.is_empty(), "Old text should no longer match");

        let results = index.search("cooking", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 1);
    }

    #[test]
    fn test_build_index() {
        let facts = vec![
            (1i64, "Rust programming language".to_string()),
            (2i64, "Python data science".to_string()),
        ];
        let index = build_index(&facts);
        assert_eq!(index.len(), 2);

        let results = index.search("programming", 1);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 1);
    }
}
