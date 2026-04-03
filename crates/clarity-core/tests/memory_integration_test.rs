//! Core-Memory Integration Tests
//!
//! These tests verify the integration between clarity-core and clarity-memory crates.

use std::sync::Arc;
use clarity_core::memory::{Memory, MemoryStore, InMemoryStore, PersistentMemoryStore, MemoryConfig, MemoryTicker};
use clarity_core::agent::{Agent, AgentConfig, MockLlm};
use clarity_core::registry::ToolRegistry;

// ==================== Memory Store Implementations ====================

#[tokio::test]
async fn test_in_memory_store_basic_operations() {
    let config = MemoryConfig {
        max_memories: 100,
        importance_threshold: 0.3,
        memory_format: String::from("[{timestamp}] {content}"),
    };

    let store = InMemoryStore::with_config(config);

    // Store memories
    store.store(Memory::new("Memory 1").with_importance(0.8)).await.unwrap();
    store.store(Memory::new("Memory 2").with_importance(0.4)).await.unwrap();
    store.store(Memory::new("Memory 3").with_importance(0.9)).await.unwrap();

    // Count
    assert_eq!(store.count().await.unwrap(), 3);

    // Get all
    let all = store.get_all().await.unwrap();
    assert_eq!(all.len(), 3);

    // Retrieve with filter
    let important = store.retrieve(0.7).await.unwrap();
    assert_eq!(important.len(), 2);
}

#[tokio::test]
async fn test_in_memory_store_max_limit() {
    let config = MemoryConfig {
        max_memories: 3,
        ..Default::default()
    };

    let store = InMemoryStore::with_config(config);

    // Store more than max
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
async fn test_in_memory_store_search() {
    let store = InMemoryStore::new();

    store.store(Memory::new("Rust is a great programming language")).await.unwrap();
    store.store(Memory::new("Python is also popular")).await.unwrap();
    store.store(Memory::new("I love Rust programming")).await.unwrap();

    // Search
    let results = store.search("rust", 10).await.unwrap();
    assert_eq!(results.len(), 2);

    // Search with limit
    let results = store.search("programming", 1).await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_in_memory_store_summarize() {
    let store = InMemoryStore::new();

    store.store(Memory::new("First memory")).await.unwrap();
    store.store(Memory::new("Second memory")).await.unwrap();

    let summary = store.summarize(10).await.unwrap();
    assert!(summary.contains("First"));
    assert!(summary.contains("Second"));
}

#[tokio::test]
async fn test_in_memory_store_clear() {
    let store = InMemoryStore::new();

    store.store(Memory::new("Test")).await.unwrap();
    assert_eq!(store.count().await.unwrap(), 1);

    store.clear().await.unwrap();
    assert_eq!(store.count().await.unwrap(), 0);
}

// ==================== Persistent Memory Store ====================

#[tokio::test]
async fn test_persistent_memory_store_basic() {
    let store = PersistentMemoryStore::new_in_memory().unwrap();

    // Store
    store.store(Memory::new("Test memory").with_tags(vec!["test".to_string()])).await.unwrap();

    // Count
    assert_eq!(store.count().await.unwrap(), 1);

    // Retrieve
    let all = store.get_all().await.unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].content, "Test memory");
}

#[tokio::test]
async fn test_persistent_memory_store_search() {
    let store = PersistentMemoryStore::new_in_memory().unwrap();

    store.store(Memory::new("User likes Rust programming").with_tags(vec!["tech".to_string()])).await.unwrap();
    store.store(Memory::new("User prefers tea").with_tags(vec!["preference".to_string()])).await.unwrap();
    store.store(Memory::new("Learning async Rust").with_tags(vec!["learning".to_string()])).await.unwrap();

    // Full-text search
    let results = store.search("Rust", 10).await.unwrap();
    assert_eq!(results.len(), 2);

    let results = store.search("programming", 10).await.unwrap();
    assert_eq!(results.len(), 1);
}

#[tokio::test]
async fn test_persistent_memory_store_with_config() {
    let config = MemoryConfig {
        max_memories: 50,
        importance_threshold: 0.5,
        memory_format: String::from("{content}"),
    };

    let store = PersistentMemoryStore::with_config(
        std::path::Path::new(":memory:"),
        config,
    )
    .unwrap();

    // Verify config is stored
    assert_eq!(store.config().max_memories, 50);
}

// ==================== Memory Ticker ====================

#[tokio::test]
async fn test_memory_ticker_basic() {
    let ticker = MemoryTicker::new(3);

    // First 2 ticks should return false
    assert!(!ticker.tick().await);
    assert!(!ticker.tick().await);

    // 3rd tick should return true
    assert!(ticker.tick().await);

    // Reset
    ticker.reset().await;
    assert_eq!(ticker.count().await, 0);
}

#[tokio::test]
async fn test_memory_ticker_cycles() {
    let ticker = MemoryTicker::new(2);

    // Cycle through
    assert!(!ticker.tick().await); // 1
    assert!(ticker.tick().await);  // 2 - trigger
    assert!(!ticker.tick().await); // 3
    assert!(ticker.tick().await);  // 4 - trigger
    assert!(!ticker.tick().await); // 5
}

#[tokio::test]
async fn test_memory_ticker_default() {
    let ticker: MemoryTicker = Default::default();

    // Default interval is 5
    for _ in 0..4 {
        assert!(!ticker.tick().await);
    }
    assert!(ticker.tick().await);
}

// ==================== Memory Struct ====================

#[test]
fn test_memory_creation() {
    let memory = Memory::new("Test content");

    assert_eq!(memory.content, "Test content");
    assert!(!memory.id.is_empty());
    assert_eq!(memory.importance, 0.5); // Default
    assert!(memory.tags.is_empty());
}

#[test]
fn test_memory_with_importance() {
    let memory = Memory::new("Test").with_importance(0.8);
    assert!((memory.importance - 0.8).abs() < 0.001);

    // Test clamping
    let memory = Memory::new("Test").with_importance(1.5);
    assert!((memory.importance - 1.0).abs() < 0.001);

    let memory = Memory::new("Test").with_importance(-0.5);
    assert!((memory.importance - 0.0).abs() < 0.001);
}

#[test]
fn test_memory_with_tags() {
    let memory = Memory::new("Test").with_tags(vec![
        "tag1".to_string(),
        "tag2".to_string(),
    ]);

    assert_eq!(memory.tags.len(), 2);
    assert!(memory.tags.contains(&"tag1".to_string()));
}

// ==================== Cross-Module Integration ====================

#[tokio::test]
async fn test_memory_store_trait_object() {
    // Both implementations should work as trait objects
    let stores: Vec<Box<dyn MemoryStore>> = vec![
        Box::new(InMemoryStore::new()),
        Box::new(PersistentMemoryStore::new_in_memory().unwrap()),
    ];

    for store in stores {
        store.store(Memory::new("Test")).await.unwrap();
        let count = store.count().await.unwrap();
        assert_eq!(count, 1);
    }
}

#[tokio::test]
async fn test_memory_with_agent_integration() {
    let memory_store: Arc<dyn MemoryStore> = Arc::new(InMemoryStore::new());

    // Pre-populate memory
    memory_store.store(Memory::new("User preference: dark mode")).await.unwrap();

    let registry = ToolRegistry::new();
    let config = AgentConfig::new();

    let agent = Agent::with_config(registry, config)
        .with_llm(Arc::new(MockLlm))
        .with_memory(memory_store.clone());

    // Run agent
    let result = agent.run("What are my preferences?").await;
    assert!(result.is_ok());

    // Memory should now contain the conversation
    let memories = memory_store.get_all().await.unwrap();
    assert!(memories.len() >= 1);
}

#[tokio::test]
async fn test_memory_thread_safety() {
    let store = Arc::new(InMemoryStore::new());

    let mut handles = vec![];

    // Spawn multiple tasks that access the store concurrently
    for i in 0..10 {
        let store_clone = store.clone();
        let handle = tokio::spawn(async move {
            store_clone
                .store(Memory::new(format!("Memory {}", i)))
                .await
                .unwrap();
        });
        handles.push(handle);
    }

    // Wait for all tasks
    for handle in handles {
        handle.await.unwrap();
    }

    // Verify all memories were stored
    assert_eq!(store.count().await.unwrap(), 10);
}

// ==================== Error Handling ====================

#[tokio::test]
async fn test_memory_store_error_handling() {
    let store = InMemoryStore::new();

    // Clear empty store should succeed
    store.clear().await.unwrap();

    // Count on empty store should return 0
    assert_eq!(store.count().await.unwrap(), 0);

    // Search on empty store should return empty
    let results = store.search("anything", 10).await.unwrap();
    assert!(results.is_empty());
}
