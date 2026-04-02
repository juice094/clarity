//! Memory Store Implementations
//!
//! Provides storage backends for memories.

use super::{Memory, MemoryConfig};
use async_trait::async_trait;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Trait for memory storage backends
#[async_trait]
pub trait MemoryStore: Send + Sync {
    /// Store a new memory
    async fn store(&self, memory: Memory) -> anyhow::Result<()>;
    
    /// Retrieve memories, optionally filtered by importance
    async fn retrieve(&self, min_importance: f32) -> anyhow::Result<Vec<Memory>>;
    
    /// Get all memories
    async fn get_all(&self) -> anyhow::Result<Vec<Memory>>;
    
    /// Clear all memories
    async fn clear(&self) -> anyhow::Result<()>;
    
    /// Get memory count
    async fn count(&self) -> anyhow::Result<usize>;
    
    /// Summarize memories into a formatted string
    async fn summarize(&self, limit: usize) -> anyhow::Result<String> {
        let memories = self.retrieve(0.0).await?;
        let summary: Vec<String> = memories
            .into_iter()
            .take(limit)
            .map(|m| format!("- {}", m.content))
            .collect();
        Ok(summary.join("\n"))
    }
}

/// In-memory storage backend
#[derive(Debug, Clone)]
pub struct InMemoryStore {
    config: MemoryConfig,
    memories: Arc<RwLock<VecDeque<Memory>>>,
}

impl InMemoryStore {
    /// Create a new in-memory store with default config
    pub fn new() -> Self {
        Self::with_config(MemoryConfig::default())
    }

    /// Create a new in-memory store with custom config
    pub fn with_config(config: MemoryConfig) -> Self {
        Self {
            config,
            memories: Arc::new(RwLock::new(VecDeque::new())),
        }
    }

    /// Get config reference
    pub fn config(&self) -> &MemoryConfig {
        &self.config
    }
}

impl Default for InMemoryStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl MemoryStore for InMemoryStore {
    async fn store(&self, memory: Memory) -> anyhow::Result<()> {
        let mut memories = self.memories.write().await;
        
        // Add new memory
        memories.push_back(memory);
        
        // Enforce max_memories limit
        while memories.len() > self.config.max_memories {
            memories.pop_front();
        }
        
        Ok(())
    }

    async fn retrieve(&self, min_importance: f32) -> anyhow::Result<Vec<Memory>> {
        let memories = self.memories.read().await;
        
        let filtered: Vec<Memory> = memories
            .iter()
            .filter(|m| m.importance >= min_importance)
            .cloned()
            .collect();
        
        Ok(filtered)
    }

    async fn get_all(&self) -> anyhow::Result<Vec<Memory>> {
        let memories = self.memories.read().await;
        Ok(memories.iter().cloned().collect())
    }

    async fn clear(&self) -> anyhow::Result<()> {
        let mut memories = self.memories.write().await;
        memories.clear();
        Ok(())
    }

    async fn count(&self) -> anyhow::Result<usize> {
        let memories = self.memories.read().await;
        Ok(memories.len())
    }

    async fn summarize(&self, limit: usize) -> anyhow::Result<String> {
        let memories = self.memories.read().await;
        
        let summary: Vec<String> = memories
            .iter()
            .rev() // Most recent first
            .take(limit)
            .map(|m| format!("- {}", m.content))
            .collect();
        
        if summary.is_empty() {
            Ok("(No memories yet)".to_string())
        } else {
            Ok(summary.join("\n"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_memory_store() {
        let store = InMemoryStore::new();
        
        // Store memories
        store.store(Memory::new("Memory 1").with_importance(0.8)).await.unwrap();
        store.store(Memory::new("Memory 2").with_importance(0.4)).await.unwrap();
        
        // Count
        assert_eq!(store.count().await.unwrap(), 2);
        
        // Retrieve all
        let all = store.get_all().await.unwrap();
        assert_eq!(all.len(), 2);
        
        // Retrieve filtered
        let important = store.retrieve(0.5).await.unwrap();
        assert_eq!(important.len(), 1);
        assert_eq!(important[0].content, "Memory 1");
    }

    #[tokio::test]
    async fn test_max_memories_limit() {
        let config = MemoryConfig {
            max_memories: 3,
            ..Default::default()
        };
        let store = InMemoryStore::with_config(config);
        
        // Store 5 memories
        for i in 0..5 {
            store.store(Memory::new(format!("Memory {}", i))).await.unwrap();
        }
        
        // Should only keep 3 most recent
        assert_eq!(store.count().await.unwrap(), 3);
        
        let memories = store.get_all().await.unwrap();
        assert_eq!(memories[0].content, "Memory 2");
        assert_eq!(memories[2].content, "Memory 4");
    }

    #[tokio::test]
    async fn test_summarize() {
        let store = InMemoryStore::new();
        
        store.store(Memory::new("First")).await.unwrap();
        store.store(Memory::new("Second")).await.unwrap();
        
        let summary = store.summarize(10).await.unwrap();
        assert!(summary.contains("Second"));
        assert!(summary.contains("First"));
    }

    #[tokio::test]
    async fn test_clear() {
        let store = InMemoryStore::new();
        
        store.store(Memory::new("Test")).await.unwrap();
        assert_eq!(store.count().await.unwrap(), 1);
        
        store.clear().await.unwrap();
        assert_eq!(store.count().await.unwrap(), 0);
    }
}
