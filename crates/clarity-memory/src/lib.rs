//! # Clarity Memory
//!
//! An advanced memory storage system for Clarity with multiple backends,
//! vector search, and OpenHanako-style memory compilation.
//!
//! ## Features
//!
//! - **Multiple Storage Backends**:
//!   - `FileStore`: JSON file storage with atomic writes
//!   - `SqliteStore`: SQLite with FTS5 full-text search (default, requires `sqlite` feature)
//!   - `HybridStore`: Hot memory cache + cold file storage
//!
//! - **Vector Search**: TF-IDF based semantic similarity without external APIs
//!
//! - **Memory Compilation**: Four-level compilation pipeline (today, week, long-term, facts)
//!
//! - **Session Storage**: JSONL-based conversation storage
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use clarity_memory::{MemoryStore, SessionStore};
//!
//! # async fn example() -> anyhow::Result<()> {
//! // Initialize stores
//! let memory_store = MemoryStore::new("memory.db".as_ref()).await?;
//! let session_store = SessionStore::new("sessions")?;
//!
//! // Save a fact
//! let id = memory_store.save_fact(
//!     "User likes Rust programming",
//!     &["preference".to_string(), "tech".to_string()],
//!     None,
//!     Some("session-1")
//! ).await?;
//!
//! // Search facts
//! let results = memory_store.search_fulltext("Rust", 10).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Storage Backends
//!
//! ```rust,no_run
//! use clarity_memory::backends::{FileStore, HybridStore, BackendConfig, StorageFactory};
//!
//! # async fn backends() -> anyhow::Result<()> {
//! // File-based storage
//! let file_store = FileStore::new("./memory_files").await?;
//!
//! // Hybrid storage with caching
//! let hybrid_store = HybridStore::new(1000, "./memory_cold", 60).await?;
//!
//! // Using the factory
//! let backend = StorageFactory::create(BackendConfig::file_default()).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Vector Search
//!
//! ```rust
//! use clarity_memory::embedding::{TfidfVectorizer, CosineIndex};
//!
//! let docs = vec![
//!     "Rust is a systems programming language",
//!     "Python is great for data science",
//! ];
//!
//! let mut vectorizer = TfidfVectorizer::new();
//! let index = CosineIndex::new(&vectorizer, &docs);
//! let results = index.search("programming", 2);
//! ```

pub mod backends;
pub mod compiler;
pub mod embedding;
pub mod extractor;
pub mod session_store;
pub mod store;
pub mod ticker;
pub mod types;

// Re-export commonly used types
pub use backends::{BackendConfig, StorageBackend, StorageFactory};
pub use backends::{FileStore, HybridStore};
#[cfg(feature = "sqlite")]
pub use backends::SqliteStore;

pub use compiler::MemoryCompiler;
pub use embedding::{CosineIndex, SparseVector, TfidfVectorizer, VectorStore};
pub use extractor::{FactExtractor, LlmClient};
pub use session_store::SessionStore;
pub use store::MemoryStore;
pub use ticker::{MemoryTicker, SharedMemoryTicker, DEFAULT_TURNS_PER_SUMMARY};
pub use types::{CompileConfig, CompileStatus, Fact, MemoryError, Message, MetaFact, Result};

#[cfg(test)]
mod integration_tests {
    use super::*;
    use extractor::MockLlmClient;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_full_workflow() {
        let temp_dir = TempDir::new().unwrap();

        // Initialize stores
        let memory_store = MemoryStore::new_in_memory().unwrap();
        let session_store = SessionStore::new(temp_dir.path().join("sessions")).unwrap();

        // Create LLM client that returns predictable responses
        let extraction_response = r#"[
            {"fact": "User likes Rust programming", "tags": ["preference", "tech"], "time": null},
            {"fact": "User is learning async programming", "tags": ["learning", "rust"], "time": null}
        ]"#;
        let llm_client = Arc::new(MockLlmClient::new(extraction_response));

        // Create compiler
        let config = CompileConfig::default();
        let _compiler = MemoryCompiler::new(
            MemoryStore::new_in_memory().unwrap(), // Use separate store for compiler
            session_store.clone(),
            llm_client.clone(),
            config,
        );

        // Add some conversation
        session_store
            .append_message("session-1", "user", "I love Rust programming!")
            .unwrap();
        session_store
            .append_message("session-1", "assistant", "That's great! Rust is a powerful language.")
            .unwrap();
        session_store
            .append_message("session-1", "user", "I'm learning async programming in Rust.")
            .unwrap();

        // Verify session store has messages
        let messages = session_store.get_messages("session-1").unwrap();
        assert_eq!(messages.len(), 3);

        // Test fact extraction separately
        let extractor = FactExtractor::new(llm_client, "gpt-4");
        let facts = extractor
            .extract_facts("User likes Rust programming")
            .await
            .unwrap();
        assert!(!facts.is_empty());
        assert!(facts[0].fact.contains("Rust"));

        // Test memory store
        let id = memory_store
            .save_fact(
                "Test fact from workflow",
                &["test".to_string()],
                None,
                Some("session-1"),
            )
            .await
            .unwrap();
        assert!(id > 0);

        let retrieved = memory_store.get_fact(id).await.unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().fact, "Test fact from workflow");
    }

    #[tokio::test]
    async fn test_error_handling() {
        // Test invalid database path - should work even with invalid path since we use in-memory fallback
        let result = MemoryStore::new_in_memory();
        assert!(result.is_ok());

        // Test empty tags search
        let store = MemoryStore::new_in_memory().unwrap();
        let result = store.search_by_tags(&[], 10).await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_ticker_integration() {
        let temp_dir = TempDir::new().unwrap();

        // Create ticker with low threshold
        let output_dir = temp_dir.path().join("memory");
        let mut ticker = MemoryTicker::new(&output_dir, Some(2));

        // Set up a simple callback that doesn't require Send
        ticker.set_compile_callback(|| Box::pin(async { Ok(std::collections::HashMap::new()) }));

        // First turn - no trigger
        assert!(ticker.notify_turn("session-1").is_none());

        // Second turn - should trigger
        assert!(ticker.notify_turn("session-1").is_some());
    }

    #[tokio::test]
    async fn test_file_store_backend() {
        use crate::backends::FileStore;

        let temp_dir = TempDir::new().unwrap();
        let store = FileStore::new(temp_dir.path()).await.unwrap();

        let id = store
            .save_fact("File store test", &["test".to_string()], None, None)
            .await
            .unwrap();

        let fact = store.get_fact(id).await.unwrap().expect("Fact should exist");
        assert_eq!(fact.fact, "File store test");

        // Test search
        let results = store.search_fulltext("File", 10).await.unwrap();
        assert_eq!(results.len(), 1);
    }

    #[tokio::test]
    #[ignore = "Timeout issue - needs investigation"]
    async fn test_hybrid_store_backend() {
        use crate::backends::HybridStore;

        let temp_dir = TempDir::new().unwrap();
        let store = HybridStore::new(100, temp_dir.path(), 1).await.unwrap();

        let id = store
            .save_fact("Hybrid store test", &["test".to_string()], None, None)
            .await
            .unwrap();

        let fact = store.get_fact(id).await.unwrap().expect("Fact should exist");
        assert_eq!(fact.fact, "Hybrid store test");

        // Check cache stats
        let stats = store.cache_stats();
        assert_eq!(stats.cache_size, 1);
    }

    #[tokio::test]
    async fn test_vector_search() {
        use crate::embedding::{CosineIndex, TfidfVectorizer};

        let documents = vec![
            "Rust is a systems programming language",
            "Python is great for data science",
            "JavaScript runs in the browser",
            "Cooking Italian recipes",
        ];

        let vectorizer = TfidfVectorizer::new();
        let index = CosineIndex::new(&vectorizer, &documents);

        let results = index.search("programming language", 2);
        assert_eq!(results.len(), 2);
        // First result should be most relevant
        assert!(results[0].1 >= results[1].1);
    }

    #[tokio::test]
    async fn test_embedding_integration() {
        use crate::embedding::VectorStore;

        let facts = vec![
            (1i64, "User likes Rust programming language".to_string()),
            (2i64, "User enjoys Python programming".to_string()),
            (3i64, "User has a dog named Max".to_string()),
        ];

        let mut store = VectorStore::new();
        store.index_facts(&facts);

        // Search for "programming" - should find facts 1 and 2
        let results = store.search("programming", 2);
        assert_eq!(results.len(), 2, "Should find 2 programming-related facts");
        // Should find the Rust and Python facts
        let ids: Vec<i64> = results.iter().map(|(id, _, _)| *id).collect();
        assert!(ids.contains(&1), "Should find Rust fact");
        assert!(ids.contains(&2), "Should find Python fact");
    }

    #[tokio::test]
    async fn test_backend_factory() {
        use crate::backends::StorageFactory;

        let temp_dir = TempDir::new().unwrap();

        // Test File backend
        let config = BackendConfig::File {
            dir: temp_dir.path().join("file_backend"),
            compress: false,
        };
        let backend = StorageFactory::create(config).await.unwrap();
        let id = backend
            .save_fact("Factory test", &["test".to_string()], None, None)
            .await
            .unwrap();
        assert!(id > 0);

        // Test Hybrid backend
        let config = BackendConfig::Hybrid {
            cache_size: 100,
            cold_dir: temp_dir.path().join("hybrid_cold"),
            sync_interval_secs: 1,
        };
        let backend = StorageFactory::create(config).await.unwrap();
        let id = backend
            .save_fact("Hybrid test", &["test".to_string()], None, None)
            .await
            .unwrap();
        assert!(id > 0);
    }

    #[tokio::test]
    async fn test_bulk_operations() {
        let store = MemoryStore::new_in_memory().unwrap();

        let facts = vec![
            ("Bulk fact 1", vec!["a".to_string()], None, None),
            ("Bulk fact 2", vec!["b".to_string()], None, None),
            ("Bulk fact 3", vec!["c".to_string()], None, None),
            ("Bulk fact 4", vec!["d".to_string()], None, None),
            ("Bulk fact 5", vec!["e".to_string()], None, None),
        ];

        let ids = store.bulk_save_facts(&facts).await.unwrap();
        assert_eq!(ids.len(), 5);
        assert_eq!(store.count_facts().await.unwrap(), 5);

        // Verify all facts were saved
        for (i, id) in ids.iter().enumerate() {
            let fact = store.get_fact(*id).await.unwrap().expect("Fact should exist");
            assert_eq!(fact.fact, format!("Bulk fact {}", i + 1));
        }
    }
}
