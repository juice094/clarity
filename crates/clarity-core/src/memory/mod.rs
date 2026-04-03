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
use std::sync::Arc;
use tokio::sync::RwLock;

mod enhanced;
mod store;

pub use enhanced::{
    FileMemoryStore, ImportanceScorer, MemoryConsolidator, TfidfSearch,
};
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

/// Memory ticker for triggering memory operations
#[derive(Debug, Clone)]
pub struct MemoryTicker {
    /// How often to trigger (in number of messages)
    pub tick_interval: usize,
    /// Message counter
    message_count: Arc<RwLock<usize>>,
}

impl MemoryTicker {
    /// Create a new ticker with the specified interval
    pub fn new(tick_interval: usize) -> Self {
        Self {
            tick_interval,
            message_count: Arc::new(RwLock::new(0)),
        }
    }

    /// Tick the counter and check if it's time to trigger
    pub async fn tick(&self) -> bool {
        let mut count = self.message_count.write().await;
        *count += 1;
        *count % self.tick_interval == 0
    }

    /// Reset the counter
    pub async fn reset(&self) {
        let mut count = self.message_count.write().await;
        *count = 0;
    }

    /// Get current count
    pub async fn count(&self) -> usize {
        *self.message_count.read().await
    }
}

impl Default for MemoryTicker {
    fn default() -> Self {
        Self::new(5) // Default: trigger every 5 messages
    }
}

/// Placeholder for PersistentMemoryStore (uses clarity-memory when enabled)
#[derive(Debug, Clone)]
pub struct PersistentMemoryStore;

impl PersistentMemoryStore {
    /// Create a new persistent memory store
    pub fn new(_db_path: &std::path::Path) -> anyhow::Result<Self> {
        Ok(Self)
    }

    /// Create an in-memory persistent store for testing
    pub fn new_in_memory() -> anyhow::Result<Self> {
        Ok(Self)
    }

    /// Create with custom config
    pub fn with_config(_db_path: &std::path::Path, _config: MemoryConfig) -> anyhow::Result<Self> {
        Ok(Self)
    }

    /// Get config reference
    pub fn config(&self) -> &MemoryConfig {
        // Return static default since this is a placeholder
        static DEFAULT_CONFIG: std::sync::OnceLock<MemoryConfig> = std::sync::OnceLock::new();
        DEFAULT_CONFIG.get_or_init(MemoryConfig::default)
    }
}

#[async_trait::async_trait]
impl MemoryStore for PersistentMemoryStore {
    async fn store(&self, _memory: Memory) -> anyhow::Result<()> {
        Ok(())
    }

    async fn retrieve(&self, _min_importance: f32) -> anyhow::Result<Vec<Memory>> {
        Ok(Vec::new())
    }

    async fn search(&self, _query: &str, _limit: usize) -> anyhow::Result<Vec<Memory>> {
        Ok(Vec::new())
    }

    async fn get_all(&self) -> anyhow::Result<Vec<Memory>> {
        Ok(Vec::new())
    }

    async fn clear(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn count(&self) -> anyhow::Result<usize> {
        Ok(0)
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
        let ticker = MemoryTicker::new(3);
        
        // First 2 ticks should return false
        assert!(!ticker.tick().await);
        assert!(!ticker.tick().await);
        
        // 3rd tick should return true
        assert!(ticker.tick().await);
        
        // Reset and verify
        ticker.reset().await;
        assert_eq!(ticker.count().await, 0);
    }

    #[tokio::test]
    async fn test_memory_ticker_cycles() {
        let ticker = MemoryTicker::new(2);
        
        // Cycle through ticks
        assert!(!ticker.tick().await); // 1
        assert!(ticker.tick().await);  // 2 - trigger
        assert!(!ticker.tick().await); // 3
        assert!(ticker.tick().await);  // 4 - trigger
    }

    #[tokio::test]
    async fn test_file_memory_store() {
        let temp_dir = tempfile::tempdir().unwrap();
        let store = FileMemoryStore::new(temp_dir.path()).unwrap();

        store.store(Memory::new("Rust is great").with_tags(vec!["tech".to_string()])).await.unwrap();
        store.store(Memory::new("I love pizza").with_tags(vec!["food".to_string()])).await.unwrap();
        
        let all = store.get_all().await.unwrap();
        assert_eq!(all.len(), 2);
        
        let search = store.search("Rust", 10).await.unwrap();
        assert_eq!(search.len(), 1);
        assert_eq!(search[0].content, "Rust is great");
        
        let count = store.count().await.unwrap();
        assert_eq!(count, 2);
    }
}
