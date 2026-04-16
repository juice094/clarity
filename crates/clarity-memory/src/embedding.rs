//! Vector search and embedding utilities
//!
//! Provides TF-IDF based vectorization for semantic similarity search
//! without requiring external ML models or APIs.
//!
//! # Example
//! ```
//! use clarity_memory::embedding::{TfidfVectorizer, CosineIndex};
//!
//! let documents = vec![
//!     "Rust is a systems programming language",
//!     "Python is great for data science",
//!     "JavaScript runs in the browser",
//! ];
//!
//! let mut vectorizer = TfidfVectorizer::new();
//! vectorizer.fit(&documents);
//!
//! let index = CosineIndex::new(&vectorizer, &documents);
//! let results = index.search("programming language", 2);
//! ```

use regex::Regex;
use std::collections::{HashMap, HashSet};

/// TF-IDF Vectorizer
///
/// Converts text documents into TF-IDF vectors for similarity comparison.
/// This is a lightweight alternative to neural embeddings.
#[derive(Debug, Clone)]
pub struct TfidfVectorizer {
    /// Document frequency: term -> number of documents containing term
    doc_freq: HashMap<String, u32>,
    /// Total number of documents fitted
    total_docs: u32,
    /// Vocabulary: term -> index
    vocabulary: HashMap<String, usize>,
    /// Stop words to ignore
    stop_words: HashSet<String>,
    /// Regex for tokenization
    tokenizer: Regex,
    /// Minimum document frequency for a term to be included
    min_df: u32,
    /// Maximum document frequency (as ratio) for a term to be included
    max_df_ratio: f32,
}

impl Default for TfidfVectorizer {
    fn default() -> Self {
        Self::new()
    }
}

impl TfidfVectorizer {
    /// Create a new TfidfVectorizer with default English stop words
    pub fn new() -> Self {
        let stop_words: HashSet<String> = [
            "the",
            "a",
            "an",
            "is",
            "are",
            "was",
            "were",
            "be",
            "been",
            "being",
            "have",
            "has",
            "had",
            "do",
            "does",
            "did",
            "will",
            "would",
            "could",
            "should",
            "may",
            "might",
            "must",
            "shall",
            "can",
            "need",
            "dare",
            "ought",
            "used",
            "to",
            "of",
            "in",
            "for",
            "on",
            "with",
            "at",
            "by",
            "from",
            "as",
            "into",
            "through",
            "during",
            "before",
            "after",
            "above",
            "below",
            "between",
            "under",
            "again",
            "further",
            "then",
            "once",
            "here",
            "there",
            "when",
            "where",
            "why",
            "how",
            "all",
            "each",
            "few",
            "more",
            "most",
            "other",
            "some",
            "such",
            "no",
            "nor",
            "not",
            "only",
            "own",
            "same",
            "so",
            "than",
            "too",
            "very",
            "just",
            "and",
            "but",
            "if",
            "or",
            "because",
            "until",
            "while",
            "this",
            "that",
            "these",
            "those",
            "i",
            "me",
            "my",
            "myself",
            "we",
            "our",
            "ours",
            "ourselves",
            "you",
            "your",
            "yours",
            "yourself",
            "yourselves",
            "he",
            "him",
            "his",
            "himself",
            "she",
            "her",
            "hers",
            "herself",
            "it",
            "its",
            "itself",
            "they",
            "them",
            "their",
            "theirs",
            "themselves",
            "what",
            "which",
            "who",
            "whom",
            "whose",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();

        Self {
            doc_freq: HashMap::new(),
            total_docs: 0,
            vocabulary: HashMap::new(),
            stop_words,
            tokenizer: Regex::new(r"[a-zA-Z0-9]+").unwrap(),
            min_df: 1,
            max_df_ratio: 0.95,
        }
    }

    /// Create vectorizer with custom stop words
    pub fn with_stop_words(stop_words: HashSet<String>) -> Self {
        let mut v = Self::new();
        v.stop_words = stop_words;
        v
    }

    /// Set minimum document frequency
    pub fn with_min_df(mut self, min_df: u32) -> Self {
        self.min_df = min_df;
        self
    }

    /// Set maximum document frequency ratio
    pub fn with_max_df_ratio(mut self, max_df_ratio: f32) -> Self {
        self.max_df_ratio = max_df_ratio;
        self
    }

    /// Tokenize text into terms
    fn tokenize(&self, text: &str) -> Vec<String> {
        self.tokenizer
            .find_iter(text.to_lowercase().as_str())
            .map(|m| m.as_str().to_string())
            .filter(|t| !self.stop_words.contains(t) && t.len() > 1)
            .collect()
    }

    /// Fit the vectorizer on a corpus of documents
    ///
    /// This builds the vocabulary and document frequency statistics.
    pub fn fit(&mut self, documents: &[impl AsRef<str>]) {
        self.total_docs = documents.len() as u32;
        self.doc_freq.clear();

        // Count document frequencies
        for doc in documents {
            let terms: HashSet<String> = self.tokenize(doc.as_ref()).into_iter().collect();
            for term in terms {
                *self.doc_freq.entry(term).or_insert(0) += 1;
            }
        }

        // Build vocabulary with filtered terms
        self.vocabulary.clear();
        let max_df = ((self.total_docs as f32 * self.max_df_ratio) as u32).max(1);

        for (term, df) in &self.doc_freq {
            if *df >= self.min_df && *df <= max_df {
                let idx = self.vocabulary.len();
                self.vocabulary.insert(term.clone(), idx);
            }
        }

        tracing::debug!(
            "Fitted TF-IDF on {} documents, vocabulary size: {}",
            self.total_docs,
            self.vocabulary.len()
        );
    }

    /// Calculate IDF for a term
    fn idf(&self, term: &str) -> f32 {
        let df = self.doc_freq.get(term).copied().unwrap_or(1).max(1);
        ((self.total_docs as f32) / (df as f32)).ln() + 1.0
    }

    /// Transform a document into a TF-IDF vector
    pub fn transform(&self, document: impl AsRef<str>) -> SparseVector {
        let terms = self.tokenize(document.as_ref());
        let term_counts: HashMap<String, u32> = terms.iter().fold(HashMap::new(), |mut acc, t| {
            *acc.entry(t.clone()).or_insert(0) += 1;
            acc
        });

        let total_terms = terms.len() as f32;
        let mut vector: HashMap<usize, f32> = HashMap::new();

        for (term, count) in term_counts {
            if let Some(&idx) = self.vocabulary.get(&term) {
                let tf = count as f32 / total_terms;
                let idf = self.idf(&term);
                vector.insert(idx, tf * idf);
            }
        }

        // L2 normalize
        let norm: f32 = vector.values().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in vector.values_mut() {
                *v /= norm;
            }
        }

        SparseVector { data: vector }
    }

    /// Fit and transform documents
    pub fn fit_transform(&mut self, documents: &[impl AsRef<str>]) -> Vec<SparseVector> {
        self.fit(documents);
        documents.iter().map(|d| self.transform(d)).collect()
    }

    /// Get vocabulary size
    pub fn vocab_size(&self) -> usize {
        self.vocabulary.len()
    }

    /// Check if a term is in the vocabulary
    pub fn has_term(&self, term: &str) -> bool {
        self.vocabulary.contains_key(term)
    }
}

/// Sparse vector representation
#[derive(Debug, Clone)]
pub struct SparseVector {
    data: HashMap<usize, f32>,
}

impl SparseVector {
    /// Create empty vector
    pub fn empty() -> Self {
        Self {
            data: HashMap::new(),
        }
    }

    /// Calculate cosine similarity with another vector
    pub fn cosine_similarity(&self, other: &SparseVector) -> f32 {
        let mut dot_product = 0.0;
        let mut self_norm_sq = 0.0;
        let mut other_norm_sq = 0.0;

        for (idx, val) in &self.data {
            self_norm_sq += val * val;
            if let Some(&other_val) = other.data.get(idx) {
                dot_product += val * other_val;
            }
        }

        for val in other.data.values() {
            other_norm_sq += val * val;
        }

        let norm = (self_norm_sq * other_norm_sq).sqrt();
        if norm > 0.0 {
            dot_product / norm
        } else {
            0.0
        }
    }

    /// Get non-zero dimensions
    pub fn non_zero_dims(&self) -> usize {
        self.data.len()
    }
}

/// Cosine similarity index for fast nearest neighbor search
#[derive(Debug)]
pub struct CosineIndex {
    vectors: Vec<(String, SparseVector)>,
    vectorizer: TfidfVectorizer,
}

impl CosineIndex {
    /// Create a new index from documents
    pub fn new(vectorizer: &TfidfVectorizer, documents: &[impl AsRef<str>]) -> Self {
        let vectors: Vec<(String, SparseVector)> = documents
            .iter()
            .map(|d| {
                let s = d.as_ref().to_string();
                let v = vectorizer.transform(&s);
                (s, v)
            })
            .collect();

        Self {
            vectors,
            vectorizer: vectorizer.clone(),
        }
    }

    /// Search for most similar documents
    ///
    /// Returns list of (document, similarity_score) pairs, sorted by similarity
    pub fn search(&self, query: impl AsRef<str>, top_k: usize) -> Vec<(String, f32)> {
        let query_vec = self.vectorizer.transform(query);

        let mut scores: Vec<(String, f32)> = self
            .vectors
            .iter()
            .map(|(doc, vec)| (doc.clone(), query_vec.cosine_similarity(vec)))
            .collect();

        // Sort by similarity (descending)
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scores.truncate(top_k);

        scores
    }

    /// Add a new document to the index
    pub fn add(&mut self, document: impl AsRef<str>) {
        let s = document.as_ref().to_string();
        let v = self.vectorizer.transform(&s);
        self.vectors.push((s, v));
    }

    /// Get index size
    pub fn len(&self) -> usize {
        self.vectors.len()
    }

    /// Check if index is empty
    pub fn is_empty(&self) -> bool {
        self.vectors.is_empty()
    }
}

/// Simple in-memory vector store for facts
#[derive(Debug)]
pub struct VectorStore {
    vectorizer: TfidfVectorizer,
    facts: Vec<(i64, String, SparseVector)>,
}

impl VectorStore {
    /// Create new vector store
    pub fn new() -> Self {
        Self {
            vectorizer: TfidfVectorizer::new(),
            facts: Vec::new(),
        }
    }

    /// Add facts and build index
    pub fn index_facts(&mut self, facts: &[(i64, String)]) {
        let texts: Vec<&str> = facts.iter().map(|(_, t)| t.as_str()).collect();
        self.vectorizer.fit(&texts);

        self.facts = facts
            .iter()
            .map(|(id, text)| (*id, text.clone(), self.vectorizer.transform(text)))
            .collect();
    }

    /// Search for similar facts
    pub fn search(&self, query: &str, top_k: usize) -> Vec<(i64, String, f32)> {
        let query_vec = self.vectorizer.transform(query);

        let mut results: Vec<(i64, String, f32)> = self
            .facts
            .iter()
            .map(|(id, text, vec)| (*id, text.clone(), query_vec.cosine_similarity(vec)))
            .filter(|(_, _, score)| *score > 0.0)
            .collect();

        results.sort_by(|a, b| b.2.partial_cmp(&a.2).unwrap());
        results.truncate(top_k);

        results
    }

    /// Get store size
    pub fn len(&self) -> usize {
        self.facts.len()
    }

    pub fn is_empty(&self) -> bool {
        self.facts.is_empty()
    }
}

impl Default for VectorStore {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tfidf_vectorizer() {
        let docs = vec![
            "Rust is a systems programming language",
            "Python is great for data science",
            "JavaScript runs in the browser",
        ];

        let mut vectorizer = TfidfVectorizer::new();
        let vectors = vectorizer.fit_transform(&docs);

        assert_eq!(vectors.len(), 3);
        assert!(vectorizer.vocab_size() > 0);
    }

    #[test]
    fn test_cosine_similarity() {
        let docs = vec![
            "programming in rust",
            "programming in python",
            "cooking recipes",
        ];

        let mut vectorizer = TfidfVectorizer::new();
        vectorizer.fit(&docs);

        let v1 = vectorizer.transform("programming in rust");
        let v2 = vectorizer.transform("programming in python");
        let v3 = vectorizer.transform("cooking recipes");

        let sim1 = v1.cosine_similarity(&v2);
        let sim2 = v1.cosine_similarity(&v3);

        assert!(sim1 > sim2, "Programming docs should be more similar");
    }

    #[test]
    fn test_cosine_index() {
        let docs = vec![
            "Rust programming language",
            "Python for data science",
            "JavaScript web development",
            "Cooking Italian recipes",
        ];

        let mut vectorizer = TfidfVectorizer::new();
        vectorizer.fit(&docs);

        let index = CosineIndex::new(&vectorizer, &docs);
        let results = index.search("programming", 2);

        assert_eq!(results.len(), 2);
        assert!(results[0].0.contains("programming") || results[0].0.contains("Python"));
    }

    #[test]
    fn test_vector_store() {
        let facts = vec![
            (1, "User likes Rust programming language".to_string()),
            (2, "User enjoys Python programming".to_string()),
            (3, "User has a dog named Max".to_string()),
        ];

        let mut store = VectorStore::new();
        store.index_facts(&facts);

        // Search for "programming" - should match facts 1 and 2
        let results = store.search("programming", 2);
        assert_eq!(results.len(), 2, "Should find 2 results for 'programming'");
        assert!(
            results[0].2 >= results[1].2,
            "Results should be sorted by score"
        );

        // First result should be about Rust or Python
        let first_text = &results[0].1;
        assert!(
            first_text.contains("Rust") || first_text.contains("Python"),
            "First result should be about programming"
        );
    }

    #[test]
    fn test_empty_query() {
        let docs = vec!["Rust programming", "Python scripting"];

        let mut vectorizer = TfidfVectorizer::new();
        vectorizer.fit(&docs);

        let empty = vectorizer.transform("");
        assert_eq!(empty.non_zero_dims(), 0);
    }

    #[test]
    fn test_stop_words() {
        let docs = vec!["the and of rust programming"];

        let mut vectorizer = TfidfVectorizer::new();
        vectorizer.fit(&docs);

        // Stop words should be filtered
        assert!(
            !vectorizer.has_term("the"),
            "'the' should be filtered as stop word"
        );
        assert!(
            !vectorizer.has_term("and"),
            "'and' should be filtered as stop word"
        );
        assert!(
            !vectorizer.has_term("of"),
            "'of' should be filtered as stop word"
        );
        assert!(
            vectorizer.has_term("rust"),
            "'rust' should be in vocabulary"
        );
        assert!(
            vectorizer.has_term("programming"),
            "'programming' should be in vocabulary"
        );
    }
}
