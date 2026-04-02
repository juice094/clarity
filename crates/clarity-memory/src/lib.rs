//! # Clarity Memory
//! 
//! An OpenHanako-style memory system for Clarity.
//! 
//! This crate provides a multi-level memory system for AI assistants:
//! 
//! - **Storage Layer**: SQLite + FTS5 for efficient fact storage and retrieval
//! - **Session Store**: JSONL-based conversation storage
//! - **Compilation Pipeline**: Four-level memory compilation (today, week, long-term, facts)
//! - **Turn-based Trigger**: Automatic compilation after N conversation turns
//! - **Fact Extraction**: LLM-powered meta-fact extraction from conversations
//! 
//! ## Quick Start
//! 
//! ```rust,no_run
//! use clarity_memory::{MemoryStore, SessionStore, MemoryCompiler, MemoryTicker, CompileConfig, LlmClient};
//! use std::sync::Arc;
//! 
//! # async fn example() -> anyhow::Result<()> {
//! // Initialize stores
//! let memory_store = MemoryStore::new("memory.db".as_ref())?;
//! let session_store = SessionStore::new("sessions")?;
//! 
//! // Create compiler
//! # let llm_client: Arc<dyn LlmClient> = unimplemented!();
//! let config = CompileConfig::default();
//! let compiler = MemoryCompiler::new(memory_store, session_store, llm_client, config);
//! 
//! // Create turn-based ticker
//! let mut ticker = MemoryTicker::new("memory_output", Some(6));
//! 
//! // Set up compile callback
//! ticker.set_compile_callback(move || {
//!     Box::pin(async move {
//!         // Run compilation here
//!         Ok(std::collections::HashMap::new())
//!     })
//! });
//! 
//! // Notify on each turn
//! if let Some(future) = ticker.notify_turn("session-1") {
//!     let results = future.await?;
//!     println!("Compilation results: {:?}", results);
//! }
//! # Ok(())
//! # }
//! ```

pub mod compiler;
pub mod extractor;
pub mod session_store;
pub mod store;
pub mod ticker;
pub mod types;

// Re-export commonly used types
pub use compiler::MemoryCompiler;
pub use extractor::{FactExtractor, LlmClient};
pub use session_store::SessionStore;
pub use store::MemoryStore;
pub use ticker::{MemoryTicker, SharedMemoryTicker, DEFAULT_TURNS_PER_SUMMARY};
pub use types::{CompileConfig, CompileStatus, Fact, MemoryError, Message, MetaFact, Result};

#[cfg(test)]
mod integration_tests {
    use super::*;
    use extractor::MockLlmClient;
    use tempfile::TempDir;
    use std::sync::Arc;

    #[tokio::test]
    async fn test_full_workflow() {
        let temp_dir = TempDir::new().unwrap();
        
        // Initialize stores
        let _memory_store = MemoryStore::new_in_memory().unwrap();
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
            config
        );
        
        // Add some conversation
        session_store.append_message("session-1", "user", "I love Rust programming!").unwrap();
        session_store.append_message("session-1", "assistant", "That's great! Rust is a powerful language.").unwrap();
        session_store.append_message("session-1", "user", "I'm learning async programming in Rust.").unwrap();
        
        // Run fact compilation
        let _facts_path = temp_dir.path().join("facts.md");
        
        // Verify session store has messages
        let messages = session_store.get_messages("session-1").unwrap();
        assert_eq!(messages.len(), 3);
        
        // Test fact extraction separately
        let extractor = FactExtractor::new(llm_client, "gpt-4");
        let facts = extractor.extract_facts("User likes Rust programming").await.unwrap();
        assert!(!facts.is_empty());
        assert!(facts[0].fact.contains("Rust"));
    }

    #[test]
    fn test_error_handling() {
        // Test invalid database path
        let result = MemoryStore::new("/invalid/path/that/does/not/exist.db".as_ref());
        assert!(result.is_err());
        
        // Test empty tags search
        let store = MemoryStore::new_in_memory().unwrap();
        let result = store.search_by_tags(&[], 10).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_ticker_integration() {
        let temp_dir = TempDir::new().unwrap();
        
        // Create ticker with low threshold
        let output_dir = temp_dir.path().join("memory");
        let mut ticker = MemoryTicker::new(&output_dir, Some(2));
        
        // Set up a simple callback that doesn't require Send
        ticker.set_compile_callback(|| Box::pin(async { 
            Ok(std::collections::HashMap::new()) 
        }));
        
        // First turn - no trigger
        assert!(ticker.notify_turn("session-1").is_none());
        
        // Second turn - should trigger
        assert!(ticker.notify_turn("session-1").is_some());
    }
}
