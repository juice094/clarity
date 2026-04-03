//! Enhanced Memory System Features
//!
//! Provides advanced memory capabilities:
//! - File-based storage backend
//! - TF-IDF vector search
//! - Automatic importance scoring
//! - Memory consolidation

use super::{Memory, MemoryConfig, MemoryStore};
use async_trait::async_trait;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::debug;

/// TF-IDF based vector search for memories
pub struct TfidfSearch {
    /// Document frequency: term -> number of documents containing term
    doc_frequency: HashMap<String, usize>,
    /// Total number of documents
    total_docs: usize,
    /// Document vectors: memory_id -> term frequencies
    doc_vectors: HashMap<String, HashMap<String, f32>>,
}

impl TfidfSearch {
    /// Create new TF-IDF search index
    pub fn new() -> Self {
        Self {
            doc_frequency: HashMap::new(),
            total_docs: 0,
            doc_vectors: HashMap::new(),
        }
    }

    /// Tokenize text into terms
    fn tokenize(text: &str) -> Vec<String> {
        text.to_lowercase()
            .split(|c: char| !c.is_alphanumeric())
            .filter(|s| !s.is_empty() && s.len() > 2)
            .map(|s| s.to_string())
            .collect()
    }

    /// Add a document to the index
    pub fn add_document(&mut self, doc_id: &str, content: &str) {
        let terms = Self::tokenize(content);
        let mut term_counts: HashMap<String, usize> = HashMap::new();

        // Count term frequencies
        for term in &terms {
            *term_counts.entry(term.clone()).or_insert(0) += 1;
        }

        // Update document frequency
        let unique_terms: HashSet<_> = terms.iter().cloned().collect();
        for term in unique_terms {
            *self.doc_frequency.entry(term).or_insert(0) += 1;
        }

        // Create TF vector
        let max_count = term_counts.values().max().copied().unwrap_or(1) as f32;
        let tf_vector: HashMap<String, f32> = term_counts
            .iter()
            .map(|(term, count)| (term.clone(), *count as f32 / max_count))
            .collect();

        self.doc_vectors.insert(doc_id.to_string(), tf_vector);
        self.total_docs += 1;
    }

    /// Remove a document from the index
    pub fn remove_document(&mut self, doc_id: &str) {
        if let Some(vector) = self.doc_vectors.remove(doc_id) {
            for term in vector.keys() {
                if let Some(count) = self.doc_frequency.get_mut(term) {
                    *count = count.saturating_sub(1);
                }
            }
            self.total_docs = self.total_docs.saturating_sub(1);
        }
    }

    /// Calculate TF-IDF score for a query against a document
    fn calculate_score(&self, query_terms: &[String], doc_id: &str) -> f32 {
        let doc_vector = match self.doc_vectors.get(doc_id) {
            Some(v) => v,
            None => return 0.0,
        };

        let mut score = 0.0;
        for term in query_terms {
            let tf = doc_vector.get(term).copied().unwrap_or(0.0);
            let idf = self.calculate_idf(term);
            score += tf * idf;
        }

        score
    }

    /// Calculate IDF for a term
    fn calculate_idf(&self, term: &str) -> f32 {
        let doc_count = self.doc_frequency.get(term).copied().unwrap_or(1) as f32;
        ((self.total_docs as f32 + 1.0) / (doc_count + 1.0)).ln() + 1.0
    }

    /// Search for documents similar to query
    pub fn search(&self, query: &str, top_k: usize) -> Vec<(String, f32)> {
        let query_terms = Self::tokenize(query);
        
        if query_terms.is_empty() || self.total_docs == 0 {
            return Vec::new();
        }

        let mut scores: Vec<(String, f32)> = self
            .doc_vectors
            .keys()
            .map(|doc_id| {
                let score = self.calculate_score(&query_terms, doc_id);
                (doc_id.clone(), score)
            })
            .filter(|(_, score)| *score > 0.0)
            .collect();

        // Sort by score descending
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        scores.truncate(top_k);

        scores
    }

    /// Clear all documents
    pub fn clear(&mut self) {
        self.doc_frequency.clear();
        self.doc_vectors.clear();
        self.total_docs = 0;
    }
}

impl Default for TfidfSearch {
    fn default() -> Self {
        Self::new()
    }
}

/// File-based memory store with TF-IDF search
pub struct FileMemoryStore {
    storage_dir: PathBuf,
    memories: Arc<RwLock<HashMap<String, Memory>>>,
    tfidf: Arc<RwLock<TfidfSearch>>,
    config: MemoryConfig,
}

impl FileMemoryStore {
    /// Create new file-based memory store
    pub fn new(storage_dir: impl Into<PathBuf>) -> anyhow::Result<Self> {
        let dir = storage_dir.into();
        std::fs::create_dir_all(&dir)?;

        let store = Self {
            storage_dir: dir.clone(),
            memories: Arc::new(RwLock::new(HashMap::new())),
            tfidf: Arc::new(RwLock::new(TfidfSearch::new())),
            config: MemoryConfig::default(),
        };

        // Load existing memories
        store.load_all()?;

        Ok(store)
    }

    /// Create with custom config
    pub fn with_config(
        storage_dir: impl Into<PathBuf>,
        config: MemoryConfig,
    ) -> anyhow::Result<Self> {
        let mut store = Self::new(storage_dir)?;
        store.config = config;
        Ok(store)
    }

    /// Load all memories from disk
    fn load_all(&self) -> anyhow::Result<()> {
        let entries = std::fs::read_dir(&self.storage_dir)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if path.extension().map(|e| e == "json").unwrap_or(false) {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(memory) = serde_json::from_str::<Memory>(&content) {
                        let _rt = tokio::runtime::Handle::try_current();
                        let mem_id = memory.id.clone();
                        let mem_content = memory.content.clone();

                        // Use blocking_write directly
                        let mut memories = self.memories.blocking_write();
                        let mut tfidf = self.tfidf.blocking_write();
                        memories.insert(mem_id.clone(), memory);
                        tfidf.add_document(&mem_id, &mem_content);
                    }
                }
            }
        }

        Ok(())
    }

    /// Save memory to disk
    async fn save_to_disk(&self, memory: &Memory) -> anyhow::Result<()> {
        let path = self.storage_dir.join(format!("{}.json", memory.id));
        let content = serde_json::to_string_pretty(memory)?;
        tokio::fs::write(&path, content).await?;
        debug!("Saved memory {} to disk", memory.id);
        Ok(())
    }

    /// Delete memory from disk
    async fn delete_from_disk(&self, memory_id: &str) -> anyhow::Result<()> {
        let path = self.storage_dir.join(format!("{}.json", memory_id));
        if path.exists() {
            tokio::fs::remove_file(&path).await?;
            debug!("Deleted memory {} from disk", memory_id);
        }
        Ok(())
    }

    /// Get default storage directory
    pub fn default_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clarity")
            .join("memories")
    }
}

#[async_trait]
impl MemoryStore for FileMemoryStore {
    async fn store(&self, memory: Memory) -> anyhow::Result<()> {
        // Add to TF-IDF index
        let mut tfidf = self.tfidf.write().await;
        tfidf.add_document(&memory.id, &memory.content);
        drop(tfidf);

        // Save to memory map
        let mut memories = self.memories.write().await;
        memories.insert(memory.id.clone(), memory.clone());
        drop(memories);

        // Persist to disk
        self.save_to_disk(&memory).await?;

        Ok(())
    }

    async fn retrieve(&self, min_importance: f32) -> anyhow::Result<Vec<Memory>> {
        let memories = self.memories.read().await;
        let mut result: Vec<_> = memories
            .values()
            .filter(|m| m.importance >= min_importance)
            .cloned()
            .collect();

        // Sort by timestamp (most recent first)
        result.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        // Limit to max_memories
        result.truncate(self.config.max_memories);

        Ok(result)
    }

    async fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<Memory>> {
        // First try TF-IDF search
        let tfidf = self.tfidf.read().await;
        let search_results = tfidf.search(query, limit);
        drop(tfidf);

        let memories = self.memories.read().await;
        let mut results = Vec::new();

        for (doc_id, score) in search_results {
            if let Some(memory) = memories.get(&doc_id) {
                let mut memory = memory.clone();
                // Boost importance based on search score
                memory.importance = (memory.importance + score.min(1.0)) / 2.0;
                results.push(memory);
            }
        }

        // If no TF-IDF results, fall back to simple text search
        if results.is_empty() {
            let query_lower = query.to_lowercase();
            results = memories
                .values()
                .filter(|m| m.content.to_lowercase().contains(&query_lower))
                .cloned()
                .collect();
            results.truncate(limit);
        }

        Ok(results)
    }

    async fn get_all(&self) -> anyhow::Result<Vec<Memory>> {
        let memories = self.memories.read().await;
        let mut result: Vec<_> = memories.values().cloned().collect();
        result.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));
        Ok(result)
    }

    async fn clear(&self) -> anyhow::Result<()> {
        let mut memories = self.memories.write().await;
        let mut tfidf = self.tfidf.write().await;

        // Delete all files
        for id in memories.keys() {
            let _ = self.delete_from_disk(id).await;
        }

        memories.clear();
        tfidf.clear();

        Ok(())
    }

    async fn count(&self) -> anyhow::Result<usize> {
        let memories = self.memories.read().await;
        Ok(memories.len())
    }
}

/// Automatic importance scorer for memories
pub struct ImportanceScorer;

impl ImportanceScorer {
    /// Score the importance of a memory based on content analysis
    pub fn score(content: &str) -> f32 {
        let mut score: f32 = 0.5; // Base score

        // Length factor (longer memories might be more important)
        let word_count = content.split_whitespace().count();
        if word_count > 50 {
            score += 0.1;
        }

        // Keyword-based scoring
        let important_keywords = [
            "important",
            "critical",
            "essential",
            "key",
            "main",
            "primary",
            "crucial",
            "vital",
            "remember",
            "note",
            "decision",
            "conclusion",
            "summary",
        ];

        let content_lower = content.to_lowercase();
        for keyword in &important_keywords {
            if content_lower.contains(keyword) {
                score += 0.05;
            }
        }

        // Question-based memories (user queries)
        if content_lower.starts_with("how") || content_lower.starts_with("what")
            || content_lower.starts_with("why") || content_lower.starts_with("when")
            || content_lower.starts_with("where") || content_lower.starts_with("who")
        {
            score += 0.05;
        }

        // Action items
        let action_keywords = ["todo", "task", "action", "fix", "implement", "create"];
        for keyword in &action_keywords {
            if content_lower.contains(keyword) {
                score += 0.1;
            }
        }

        // Code-related memories
        if content.contains("```") || content.contains("fn ") || content.contains("class ") {
            score += 0.1;
        }

        score.clamp(0.0, 1.0)
    }

    /// Score and update memory importance
    pub fn score_memory(mut memory: Memory) -> Memory {
        let score = Self::score(&memory.content);
        memory.importance = score;
        memory
    }
}

/// Memory consolidator for merging similar memories
pub struct MemoryConsolidator;

impl MemoryConsolidator {
    /// Find similar memories based on content overlap
    pub fn find_similar(memories: &[Memory], threshold: f32) -> Vec<Vec<String>> {
        let mut groups: Vec<Vec<String>> = Vec::new();
        let mut used: HashSet<String> = HashSet::new();

        for i in 0..memories.len() {
            let mem_i = &memories[i];
            if used.contains(&mem_i.id) {
                continue;
            }

            let mut group = vec![mem_i.id.clone()];
            used.insert(mem_i.id.clone());

            for j in (i + 1)..memories.len() {
                let mem_j = &memories[j];
                if used.contains(&mem_j.id) {
                    continue;
                }

                let similarity = Self::calculate_similarity(&mem_i.content, &mem_j.content);
                if similarity >= threshold {
                    group.push(mem_j.id.clone());
                    used.insert(mem_j.id.clone());
                }
            }

            if group.len() > 1 {
                groups.push(group);
            }
        }

        groups
    }

    /// Calculate simple Jaccard similarity between two texts
    fn calculate_similarity(a: &str, b: &str) -> f32 {
        let a_lower = a.to_lowercase();
        let b_lower = b.to_lowercase();
        let tokens_a: HashSet<_> = a_lower.split_whitespace().collect();
        let tokens_b: HashSet<_> = b_lower.split_whitespace().collect();

        if tokens_a.is_empty() || tokens_b.is_empty() {
            return 0.0;
        }

        let intersection: HashSet<_> = tokens_a.intersection(&tokens_b).collect();
        let union: HashSet<_> = tokens_a.union(&tokens_b).collect();

        intersection.len() as f32 / union.len() as f32
    }

    /// Consolidate a group of similar memories into one
    pub fn consolidate_group(group: &[Memory]) -> Memory {
        // Use the most recent and highest importance memory as base
        let base = group
            .iter()
            .max_by(|a, b| {
                a.timestamp
                    .cmp(&b.timestamp)
                    .then_with(|| a.importance.partial_cmp(&b.importance).unwrap())
            })
            .cloned()
            .unwrap_or_else(|| Memory::new(""));

        // Merge tags from all memories
        let all_tags: HashSet<_> = group.iter().flat_map(|m| m.tags.clone()).collect();

        Memory {
            id: base.id,
            timestamp: base.timestamp,
            content: base.content,
            importance: (base.importance + 0.1).min(1.0), // Boost importance slightly
            tags: all_tags.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tfidf_tokenize() {
        let text = "Hello world! This is a test.";
        let tokens = TfidfSearch::tokenize(text);
        assert!(tokens.contains(&"hello".to_string()));
        assert!(tokens.contains(&"world".to_string()));
        assert!(tokens.contains(&"test".to_string()));
    }

    #[test]
    fn test_tfidf_search() {
        let mut tfidf = TfidfSearch::new();
        tfidf.add_document("1", "Rust is a systems programming language");
        tfidf.add_document("2", "Python is great for data science");
        tfidf.add_document("3", "Rust has excellent performance");

        let results = tfidf.search("Rust programming", 2);
        assert!(!results.is_empty());
        assert_eq!(results[0].0, "1"); // Most relevant
    }

    #[test]
    fn test_importance_scorer() {
        let low = ImportanceScorer::score("Hi there");
        let high = ImportanceScorer::score("This is a critical decision that we must remember");

        assert!(high > low);
        assert!(low >= 0.0 && low <= 1.0);
        assert!(high >= 0.0 && high <= 1.0);
    }

    #[test]
    fn test_memory_consolidator() {
        let memories = vec![
            Memory::new("Rust is fast"),
            Memory::new("Rust is safe and fast"),
            Memory::new("Python is easy"),
        ];

        let similar = MemoryConsolidator::find_similar(&memories, 0.5);
        assert!(!similar.is_empty());
    }

    #[tokio::test]
    async fn test_file_memory_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = FileMemoryStore::new(temp_dir.path()).unwrap();

        store.store(Memory::new("Test memory")).await.unwrap();

        let all = store.get_all().await.unwrap();
        assert_eq!(all.len(), 1);

        let search = store.search("Test", 10).await.unwrap();
        assert_eq!(search.len(), 1);
    }
}
