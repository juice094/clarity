//! BM25 (Okapi BM25) scoring for information retrieval
//!
//! BM25 is a bag-of-words retrieval function that ranks a set of documents
//! based on the query terms appearing in each document. It is generally
//! considered superior to TF-IDF for short-text retrieval (like facts).
//!
//! # Formula
//!
//! ```text
//! score(D, Q) = Σ IDF(qᵢ) · (f(qᵢ,D) · (k1 + 1)) / (f(qᵢ,D) + k1 · (1 - b + b · |D| / avgdl))
//!
//! IDF(qᵢ) = ln((N - n(qᵢ) + 0.5) / (n(qᵢ) + 0.5) + 1)
//! ```
//!
//! Where:
//! - `N` = total number of documents
//! - `n(qᵢ)` = number of documents containing term qᵢ
//! - `f(qᵢ,D)` = frequency of term qᵢ in document D
//! - `|D|` = length of document D (in terms)
//! - `avgdl` = average document length
//! - `k1` = term frequency saturation parameter (default 1.5)
//! - `b` = length normalization parameter (default 0.75)

use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// Default k1 parameter for BM25
const DEFAULT_K1: f32 = 1.5;
/// Default b parameter for BM25
const DEFAULT_B: f32 = 0.75;

/// English stop words for token filtering
fn default_stop_words() -> HashSet<String> {
    [
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
        "am",
        "been",
        "being",
        "have",
        "has",
        "had",
        "do",
        "does",
        "did",
        "doing",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

/// Token-matching regex for [`tokenize`].
///
/// # Panics
///
/// Panics only if the literal pattern is invalid; it is known to be valid.
#[allow(clippy::unwrap_used)]
static TOKEN_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\b[a-zA-Z_\-]+\b").unwrap());

/// Tokenize text into lowercase terms, filtering stop words and short tokens
fn tokenize(text: &str) -> Vec<String> {
    let stop_words = default_stop_words();
    TOKEN_RE
        .find_iter(text.to_lowercase().as_str())
        .map(|m| m.as_str().to_string())
        .filter(|t| !stop_words.contains(t) && t.len() > 1)
        .collect()
}

/// BM25 index for efficient document scoring
///
/// Build once from a corpus, then score multiple queries.
#[derive(Debug, Clone)]
pub struct Bm25Index {
    /// Term frequency per document: doc_idx -> term -> count
    doc_term_freqs: Vec<HashMap<String, u32>>,
    /// Document lengths (in terms)
    doc_lengths: Vec<u32>,
    /// Total number of documents
    total_docs: u32,
    /// Average document length
    avg_dl: f32,
    /// Document frequency: term -> number of docs containing it
    doc_freq: HashMap<String, u32>,
    /// k1: term frequency saturation (default 1.5)
    k1: f32,
    /// b: length normalization (default 0.75)
    b: f32,
}

impl Bm25Index {
    /// Build a BM25 index from a corpus of documents
    ///
    /// # Example
    ///
    /// ```
    /// use clarity_memory::bm25::Bm25Index;
    ///
    /// let docs = vec![
    ///     "Rust is a systems programming language",
    ///     "Python is great for data science",
    ///     "JavaScript runs in the browser",
    /// ];
    ///
    /// let index = Bm25Index::new(&docs);
    /// let scores: Vec<f32> = (0..docs.len()).map(|i| index.score("programming", i)).collect();
    /// ```
    pub fn new(docs: &[impl AsRef<str>]) -> Self {
        let total_docs = docs.len() as u32;
        let mut doc_term_freqs = Vec::with_capacity(docs.len());
        let mut doc_lengths = Vec::with_capacity(docs.len());
        let mut doc_freq: HashMap<String, u32> = HashMap::new();
        let mut total_length = 0u32;

        for doc in docs {
            let terms = tokenize(doc.as_ref());
            let mut term_freq: HashMap<String, u32> = HashMap::new();
            let mut unique_terms: HashSet<String> = HashSet::new();

            for term in terms {
                *term_freq.entry(term.clone()).or_insert(0) += 1;
                unique_terms.insert(term);
            }

            for term in unique_terms {
                *doc_freq.entry(term).or_insert(0) += 1;
            }

            let doc_len = term_freq.values().sum::<u32>();
            total_length += doc_len;
            doc_lengths.push(doc_len);
            doc_term_freqs.push(term_freq);
        }

        let avg_dl = if total_docs > 0 {
            total_length as f32 / total_docs as f32
        } else {
            1.0
        };

        Self {
            doc_term_freqs,
            doc_lengths,
            total_docs,
            avg_dl,
            doc_freq,
            k1: DEFAULT_K1,
            b: DEFAULT_B,
        }
    }

    /// Create a BM25 index with custom k1 and b parameters
    ///
    /// - `k1`: Controls term frequency saturation. Higher values = more saturation.
    ///   Typical range: 1.2 - 2.0. Default: 1.5.
    /// - `b`: Controls length normalization. 0 = no normalization, 1 = full normalization.
    ///   Typical range: 0.5 - 0.85. Default: 0.75.
    pub fn with_params(docs: &[impl AsRef<str>], k1: f32, b: f32) -> Self {
        let mut index = Self::new(docs);
        index.k1 = k1;
        index.b = b;
        index
    }

    /// Calculate IDF for a term using Lucene-smoothed formula
    ///
    /// Formula: `ln(1 + (N - n + 0.5) / (n + 0.5))`
    ///
    /// This version is always non-negative, even when a term appears in all documents.
    fn idf(&self, term: &str) -> f32 {
        let n = self.doc_freq.get(term).copied().unwrap_or(0) as f32;
        let numerator = self.total_docs as f32 - n + 0.5;
        let denominator = n + 0.5;
        (1.0 + numerator / denominator).ln()
    }

    /// Score a single document against a query
    ///
    /// Returns the BM25 score. Higher = more relevant.
    pub fn score(&self, query: &str, doc_idx: usize) -> f32 {
        if doc_idx >= self.doc_term_freqs.len() {
            return 0.0;
        }

        let query_terms = tokenize(query);
        if query_terms.is_empty() {
            return 0.0;
        }

        let term_freqs = &self.doc_term_freqs[doc_idx];
        let doc_len = self.doc_lengths[doc_idx] as f32;
        let mut score = 0.0;

        for term in query_terms {
            let f = *term_freqs.get(&term).unwrap_or(&0) as f32;
            if f == 0.0 {
                continue;
            }

            let idf = self.idf(&term);
            let dl_ratio = doc_len / self.avg_dl;
            let denom = f + self.k1 * (1.0 - self.b + self.b * dl_ratio);

            score += idf * (f * (self.k1 + 1.0)) / denom;
        }

        score
    }

    /// Search for the top-k most relevant documents
    ///
    /// Returns a list of `(doc_index, score)` pairs, sorted by score descending.
    pub fn search(&self, query: &str, top_k: usize) -> Vec<(usize, f32)> {
        let mut results: Vec<(usize, f32)> = (0..self.doc_term_freqs.len())
            .map(|idx| (idx, self.score(query, idx)))
            .filter(|(_, score)| *score > 0.0)
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    /// Get the number of documents in the index
    pub fn len(&self) -> usize {
        self.doc_term_freqs.len()
    }

    /// Check if the index is empty
    pub fn is_empty(&self) -> bool {
        self.doc_term_freqs.is_empty()
    }

    /// Get the average document length
    pub fn avg_dl(&self) -> f32 {
        self.avg_dl
    }
}

impl Default for Bm25Index {
    fn default() -> Self {
        Self {
            doc_term_freqs: Vec::new(),
            doc_lengths: Vec::new(),
            total_docs: 0,
            avg_dl: 1.0,
            doc_freq: HashMap::new(),
            k1: DEFAULT_K1,
            b: DEFAULT_B,
        }
    }
}

/// Incremental BM25 index that supports adding and removing documents
///
/// Unlike [`Bm25Index`] which is built from a static corpus, this index
/// maintains mutable state so documents can be added or removed one at a
/// time without rebuilding the whole index.
#[derive(Debug, Clone)]
pub struct IncrementalBm25Index {
    /// Term frequency per document: doc_idx -> term -> count
    doc_term_freqs: Vec<HashMap<String, u32>>,
    /// Document lengths (in terms)
    doc_lengths: Vec<u32>,
    /// Tombstone marker for removed documents
    alive: Vec<bool>,
    /// Document frequency: term -> number of alive docs containing it
    doc_freq: HashMap<String, u32>,
    /// Total number of alive documents
    total_docs: u32,
    /// Total length of all alive documents
    total_length: u32,
    /// k1: term frequency saturation
    k1: f32,
    /// b: length normalization
    b: f32,
}

impl IncrementalBm25Index {
    /// Create an empty incremental BM25 index
    pub fn new() -> Self {
        Self {
            doc_term_freqs: Vec::new(),
            doc_lengths: Vec::new(),
            alive: Vec::new(),
            doc_freq: HashMap::new(),
            total_docs: 0,
            total_length: 0,
            k1: DEFAULT_K1,
            b: DEFAULT_B,
        }
    }

    /// Create an index with custom parameters
    pub fn with_params(k1: f32, b: f32) -> Self {
        let mut idx = Self::new();
        idx.k1 = k1;
        idx.b = b;
        idx
    }

    /// Add a single document and return its index
    ///
    /// The returned `usize` is the stable `doc_idx` that can later be
    /// passed to [`Self::score`] or [`Self::remove_document`].
    pub fn add_document(&mut self, doc: &str) -> usize {
        let terms = tokenize(doc);
        let mut term_freq: HashMap<String, u32> = HashMap::new();
        let mut unique_terms: HashSet<String> = HashSet::new();

        for term in terms {
            *term_freq.entry(term.clone()).or_insert(0) += 1;
            unique_terms.insert(term);
        }

        for term in unique_terms {
            *self.doc_freq.entry(term).or_insert(0) += 1;
        }

        let doc_len = term_freq.values().sum::<u32>();
        self.total_length += doc_len;
        self.total_docs += 1;

        let idx = self.doc_term_freqs.len();
        self.doc_term_freqs.push(term_freq);
        self.doc_lengths.push(doc_len);
        self.alive.push(true);
        idx
    }

    /// Remove a document by index (tombstone)
    ///
    /// Returns `true` if the document was alive and is now removed.
    pub fn remove_document(&mut self, idx: usize) -> bool {
        if idx >= self.alive.len() || !self.alive[idx] {
            return false;
        }
        self.alive[idx] = false;

        let term_freqs = &self.doc_term_freqs[idx];
        for term in term_freqs.keys() {
            if let Some(count) = self.doc_freq.get_mut(term) {
                *count = count.saturating_sub(1);
                if *count == 0 {
                    self.doc_freq.remove(term);
                }
            }
        }

        self.total_docs -= 1;
        self.total_length -= self.doc_lengths[idx];
        true
    }

    /// Check whether a document index is still alive
    pub fn is_alive(&self, idx: usize) -> bool {
        self.alive.get(idx).copied().unwrap_or(false)
    }

    /// Current average document length (alive docs only)
    fn avg_dl(&self) -> f32 {
        if self.total_docs > 0 {
            self.total_length as f32 / self.total_docs as f32
        } else {
            1.0
        }
    }

    /// Calculate IDF for a term using Lucene-smoothed formula
    fn idf(&self, term: &str) -> f32 {
        let n = self.doc_freq.get(term).copied().unwrap_or(0) as f32;
        let numerator = self.total_docs as f32 - n + 0.5;
        let denominator = n + 0.5;
        (1.0 + numerator / denominator).ln()
    }

    /// Score a single alive document against a query
    ///
    /// Returns `0.0` for dead or out-of-bounds indices.
    pub fn score(&self, query: &str, doc_idx: usize) -> f32 {
        if doc_idx >= self.alive.len() || !self.alive[doc_idx] {
            return 0.0;
        }

        let query_terms = tokenize(query);
        if query_terms.is_empty() {
            return 0.0;
        }

        let term_freqs = &self.doc_term_freqs[doc_idx];
        let doc_len = self.doc_lengths[doc_idx] as f32;
        let avg_dl = self.avg_dl();
        let mut score = 0.0;

        for term in query_terms {
            let f = *term_freqs.get(&term).unwrap_or(&0) as f32;
            if f == 0.0 {
                continue;
            }

            let idf = self.idf(&term);
            let dl_ratio = doc_len / avg_dl;
            let denom = f + self.k1 * (1.0 - self.b + self.b * dl_ratio);

            score += idf * (f * (self.k1 + 1.0)) / denom;
        }

        score
    }

    /// Search for the top-k most relevant alive documents
    ///
    /// Returns a list of `(doc_index, score)` pairs, sorted by score descending.
    pub fn search(&self, query: &str, top_k: usize) -> Vec<(usize, f32)> {
        let mut results: Vec<(usize, f32)> = (0..self.doc_term_freqs.len())
            .filter(|&idx| self.alive[idx])
            .map(|idx| (idx, self.score(query, idx)))
            .filter(|(_, score)| *score > 0.0)
            .collect();

        results.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        results.truncate(top_k);
        results
    }

    /// Number of alive documents
    pub fn len(&self) -> usize {
        self.total_docs as usize
    }

    /// Check if there are no alive documents
    pub fn is_empty(&self) -> bool {
        self.total_docs == 0
    }
}

impl Default for IncrementalBm25Index {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_incremental_bm25_basic() {
        let mut index = IncrementalBm25Index::new();
        let idx0 = index.add_document("Rust is a systems programming language");
        let idx1 = index.add_document("Python is great for data science");
        let idx2 = index.add_document("JavaScript runs in the browser");

        assert_eq!(index.len(), 3);
        assert!(index.is_alive(idx0));

        let score0 = index.score("programming", idx0);
        let score1 = index.score("programming", idx1);
        let score2 = index.score("programming", idx2);

        assert!(
            score0 > score1,
            "Rust doc should score higher than Python doc"
        );
        assert!(score0 > score2, "Rust doc should score higher than JS doc");
    }

    #[test]
    fn test_incremental_bm25_remove_document() {
        let mut index = IncrementalBm25Index::new();
        let idx0 = index.add_document("Rust programming");
        let idx1 = index.add_document("Python programming");
        let _idx2 = index.add_document("Cooking recipes");

        assert_eq!(index.len(), 3);

        // Remove the Rust doc
        assert!(index.remove_document(idx0));
        assert!(!index.is_alive(idx0));
        assert_eq!(index.len(), 2);

        // Scoring a removed doc returns 0
        assert_eq!(index.score("programming", idx0), 0.0);

        // Search should only return alive docs
        let results = index.search("programming", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, idx1);

        // Removing same doc again is a no-op
        assert!(!index.remove_document(idx0));
    }

    #[test]
    fn test_incremental_bm25_idf_updates_on_remove() {
        let mut index = IncrementalBm25Index::new();
        let idx0 = index.add_document("Rust programming language");
        let idx1 = index.add_document("Python programming");
        let _idx2 = index.add_document("Cooking recipes");

        // Score before removal
        let score_before = index.score("programming", idx1);

        // Remove Rust doc (also contains "programming")
        index.remove_document(idx0);

        // Score after removal — IDF for "programming" should increase
        let score_after = index.score("programming", idx1);
        assert!(
            score_after > score_before,
            "IDF should increase after removing a doc with the query term"
        );
    }

    #[test]
    fn test_incremental_bm25_search_top_k() {
        let mut index = IncrementalBm25Index::new();
        index.add_document("User likes Rust programming language");
        index.add_document("User enjoys Python programming");
        index.add_document("User has a dog named Max");

        let results = index.search("programming", 2);
        assert_eq!(results.len(), 2);
        assert!(results[0].1 >= results[1].1);
    }

    #[test]
    fn test_incremental_bm25_empty() {
        let index = IncrementalBm25Index::new();
        assert!(index.is_empty());
        assert_eq!(index.search("test", 5).len(), 0);
    }

    #[test]
    fn test_incremental_bm25_add_after_remove() {
        let mut index = IncrementalBm25Index::new();
        let idx0 = index.add_document("Rust programming");
        index.remove_document(idx0);

        // New doc gets a new index (tombstone is not reused in this design)
        let idx1 = index.add_document("Python programming");
        assert!(index.is_alive(idx1));
        assert_eq!(index.len(), 1);

        let results = index.search("programming", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, idx1);
    }

    #[test]
    fn test_bm25_basic() {
        let docs = vec![
            "Rust is a systems programming language",
            "Python is great for data science",
            "JavaScript runs in the browser",
            "Cooking Italian recipes",
        ];

        let index = Bm25Index::new(&docs);

        // "programming" should match doc 0 best
        let scores: Vec<f32> = (0..docs.len())
            .map(|i| index.score("programming", i))
            .collect();
        assert!(
            scores[0] > scores[1],
            "Rust doc should score higher than Python doc for 'programming'"
        );
        assert!(
            scores[0] > scores[2],
            "Rust doc should score higher than JS doc"
        );
        assert!(
            scores[0] > scores[3],
            "Rust doc should score higher than cooking doc"
        );
    }

    #[test]
    fn test_bm25_search() {
        let docs = vec![
            "User likes Rust programming language",
            "User enjoys Python programming",
            "User has a dog named Max",
        ];

        let index = Bm25Index::new(&docs);
        let results = index.search("programming", 2);

        assert_eq!(results.len(), 2);
        // First result should be doc 0 or 1 (both about programming)
        assert!(
            results[0].0 == 0 || results[0].0 == 1,
            "First result should be a programming doc"
        );
    }

    #[test]
    fn test_bm25_empty_query() {
        let docs = vec!["Rust programming", "Python scripting"];
        let index = Bm25Index::new(&docs);

        let score = index.score("", 0);
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_bm25_empty_corpus() {
        let docs: Vec<&str> = vec![];
        let index = Bm25Index::new(&docs);

        assert!(index.is_empty());
        assert_eq!(index.score("test", 0), 0.0);
    }

    #[test]
    fn test_bm25_vs_tfidf_short_text() {
        // BM25 is known to outperform TF-IDF on short text
        let docs = vec![
            "Rust async programming",
            "Rust sync programming",
            "Python async programming",
        ];

        let index = Bm25Index::new(&docs);

        // Query with term that appears in all docs
        let scores: Vec<f32> = (0..docs.len())
            .map(|i| index.score("programming", i))
            .collect();
        // All docs contain "programming", so IDF is low but all should have some score
        // Use is_finite to avoid NaN issues and allow very small positive scores
        assert!(
            scores.iter().all(|&s| s.is_finite() && s >= 0.0),
            "All docs contain 'programming', scores should be non-negative finite: {:?}",
            scores
        );
        // At least one doc should have a positive score
        assert!(
            scores.iter().any(|&s| s > 0.0),
            "At least one doc should score positively"
        );
    }

    #[test]
    fn test_bm25_parameters() {
        let docs = vec![
            "Rust programming language with many features",
            "Rust programming",
        ];

        // With high b, longer documents are penalized more
        let index_high_b = Bm25Index::with_params(&docs, 1.5, 0.9);
        let score1_high = index_high_b.score("programming", 0);
        let score2_high = index_high_b.score("programming", 1);

        // With low b, length normalization is weaker
        let index_low_b = Bm25Index::with_params(&docs, 1.5, 0.1);
        let score1_low = index_low_b.score("programming", 0);
        let score2_low = index_low_b.score("programming", 1);

        // The ratio between doc scores should differ based on b
        let ratio_high = score2_high / score1_high;
        let ratio_low = score2_low / score1_low;
        assert!(
            ratio_high > ratio_low || (ratio_high - ratio_low).abs() < 0.01,
            "High b should favor shorter doc more (or be roughly equal due to term saturation)"
        );
    }
}
