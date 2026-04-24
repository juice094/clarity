//! Memory System for Clarity
//!
//! Manages conversation memory storage and retrieval.
//! Provides ticker-based memory updates during agent execution.
//!
//! Enhanced features:
//! - File-based storage backend
//! - TF-IDF vector search
//! - Automatic importance scoring
//! - Memory consolidation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

mod llm_bridge;
mod store;

pub use clarity_memory::chunking::{Chunk, ChunkConfig, Chunker};
pub use llm_bridge::LlmProviderBridge;
pub use store::{InMemoryStore, MemoryStore};

/// A single memory entry
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Memory {
    /// Unique identifier for this memory
    pub id: String,
    /// Timestamp when the memory was created
    pub timestamp: chrono::DateTime<chrono::Utc>,
    /// Content of the memory
    pub content: String,
    /// Importance score (0.0 - 1.0)
    pub importance: f32,
    /// Optional tags for categorization
    pub tags: Vec<String>,
}

impl Memory {
    /// Create a new memory entry
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            id: uuid(),
            timestamp: chrono::Utc::now(),
            content: content.into(),
            importance: 0.5,
            tags: Vec::new(),
        }
    }

    /// Set importance score
    pub fn with_importance(mut self, importance: f32) -> Self {
        self.importance = importance.clamp(0.0, 1.0);
        self
    }

    /// Add tags
    pub fn with_tags(mut self, tags: Vec<String>) -> Self {
        self.tags = tags;
        self
    }
}

/// Memory configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryConfig {
    /// Maximum number of memories to retain
    pub max_memories: usize,
    /// Threshold for including memory in system prompt
    pub importance_threshold: f32,
    /// Format string for memory display
    pub memory_format: String,
}

impl Default for MemoryConfig {
    fn default() -> Self {
        Self {
            max_memories: 100,
            importance_threshold: 0.3,
            memory_format: String::from("[{timestamp}] {content}"),
        }
    }
}

// Re-export clarity-memory's full-featured ticker implementations.
// The legacy simplified MemoryTicker (message counter only, no callback)
// has been removed. Use SharedMemoryTicker for async-safe cross-boundary
// usage, or MemoryTicker directly for single-threaded scenarios.
pub use clarity_memory::{MemoryTicker, SharedMemoryTicker};

pub mod extraction;
pub use extraction::TurnMemoryExtractor;

/// Persistent memory store backed by `clarity-memory`
#[derive(Debug)]
pub struct PersistentMemoryStore {
    inner: clarity_memory::MemoryStore,
    config: MemoryConfig,
    importance_scores: Arc<RwLock<HashMap<i64, f32>>>,
}

impl PersistentMemoryStore {
    /// Create a new persistent memory store
    pub async fn new(db_path: &std::path::Path) -> anyhow::Result<Self> {
        let inner = clarity_memory::MemoryStore::new(db_path).await?;
        Ok(Self {
            inner,
            config: MemoryConfig::default(),
            importance_scores: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create an in-memory persistent store for testing
    pub fn new_in_memory() -> anyhow::Result<Self> {
        let inner = clarity_memory::MemoryStore::new_in_memory()?;
        Ok(Self {
            inner,
            config: MemoryConfig::default(),
            importance_scores: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    /// Create with custom config
    pub async fn with_config(
        db_path: &std::path::Path,
        config: MemoryConfig,
    ) -> anyhow::Result<Self> {
        let mut store = Self::new(db_path).await?;
        store.config = config;
        Ok(store)
    }

    /// Get config reference
    pub fn config(&self) -> &MemoryConfig {
        &self.config
    }

    /// Access the underlying `clarity_memory::MemoryStore`.
    ///
    /// Used by `MemoryCompiler` which needs the concrete store type.
    pub fn inner(&self) -> &clarity_memory::MemoryStore {
        &self.inner
    }
}

fn fact_to_memory(fact: clarity_memory::Fact, scores: &HashMap<i64, f32>) -> Memory {
    Memory {
        id: fact.id.to_string(),
        timestamp: fact.created_at,
        content: fact.fact,
        importance: scores.get(&fact.id).copied().unwrap_or(0.5),
        tags: fact.tags,
    }
}

#[async_trait::async_trait]
impl MemoryStore for PersistentMemoryStore {
    async fn store(&self, memory: Memory) -> anyhow::Result<()> {
        let id = self
            .inner
            .save_fact(&memory.content, &memory.tags, None, None)
            .await?;
        let mut scores = self.importance_scores.write().await;
        scores.insert(id, memory.importance);
        Ok(())
    }

    async fn retrieve(&self, min_importance: f32) -> anyhow::Result<Vec<Memory>> {
        let all = self.get_all().await?;
        Ok(all
            .into_iter()
            .filter(|m| m.importance >= min_importance)
            .collect())
    }

    async fn search(&self, query: &str, limit: usize) -> anyhow::Result<Vec<Memory>> {
        let facts = self.inner.search_fulltext(query, limit).await?;
        let scores = self.importance_scores.read().await;
        Ok(facts
            .into_iter()
            .map(|f| fact_to_memory(f, &scores))
            .collect())
    }

    async fn search_similar(
        &self,
        query: &str,
        limit: usize,
    ) -> anyhow::Result<Vec<(Memory, f32)>> {
        let results = self.inner.search_hybrid(query, limit).await?;
        let scores = self.importance_scores.read().await;
        Ok(results
            .into_iter()
            .map(|(fact, score)| (fact_to_memory(fact, &scores), score))
            .collect())
    }

    async fn get_all(&self) -> anyhow::Result<Vec<Memory>> {
        let count = self.inner.count_facts().await? as usize;
        if count == 0 {
            return Ok(Vec::new());
        }
        let facts = self.inner.get_recent_facts(count).await?;
        let scores = self.importance_scores.read().await;
        Ok(facts
            .into_iter()
            .map(|f| fact_to_memory(f, &scores))
            .collect())
    }

    async fn clear(&self) -> anyhow::Result<()> {
        self.inner.clear_all().await?;
        let mut scores = self.importance_scores.write().await;
        scores.clear();
        Ok(())
    }

    async fn count(&self) -> anyhow::Result<usize> {
        Ok(self.inner.count_facts().await? as usize)
    }
}

/// Generate a simple UUID
fn uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    format!("{:x}", timestamp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_creation() {
        let memory = Memory::new("Test content").with_importance(0.8);

        assert_eq!(memory.content, "Test content");
        assert!((memory.importance - 0.8).abs() < 0.001);
        assert!(!memory.id.is_empty());
    }

    #[tokio::test]
    async fn test_memory_ticker() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut ticker = MemoryTicker::new(temp_dir.path(), Some(3));
        ticker.set_compile_callback(|| async { Ok(std::collections::HashMap::new()) });
        let shared = SharedMemoryTicker::new(ticker);

        // First 2 turns should not trigger
        assert!(shared.notify_turn("session-1").await.is_none());
        assert!(shared.notify_turn("session-1").await.is_none());

        // 3rd turn should trigger
        assert!(shared.notify_turn("session-1").await.is_some());

        // Reset and verify
        shared.reset_turn_count("session-1").await;
        assert_eq!(shared.get_turn_count("session-1").await, 0);
    }

    #[tokio::test]
    async fn test_memory_ticker_cycles() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut ticker = MemoryTicker::new(temp_dir.path(), Some(2));
        ticker.set_compile_callback(|| async { Ok(std::collections::HashMap::new()) });
        let shared = SharedMemoryTicker::new(ticker);

        // Cycle through turns
        assert!(shared.notify_turn("session-1").await.is_none()); // 1
        // Trigger and wait to reset the compiling flag
        assert!(shared.notify_turn_and_wait("session-1").await.is_some()); // 2 - trigger
        assert!(shared.notify_turn("session-1").await.is_none()); // 3
        assert!(shared.notify_turn_and_wait("session-1").await.is_some()); // 4 - trigger
    }

    #[tokio::test]
    async fn test_persistent_memory_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = PersistentMemoryStore::new(temp_dir.path().join("memory.db").as_path())
            .await
            .unwrap();

        store
            .store(Memory::new("Rust is great").with_tags(vec!["tech".to_string()]))
            .await
            .unwrap();
        store
            .store(Memory::new("I love pizza").with_tags(vec!["food".to_string()]))
            .await
            .unwrap();

        let all = store.get_all().await.unwrap();
        assert_eq!(all.len(), 2);

        let search = store.search("Rust", 10).await.unwrap();
        assert_eq!(search.len(), 1);
        assert_eq!(search[0].content, "Rust is great");

        let count = store.count().await.unwrap();
        assert_eq!(count, 2);
    }
}
