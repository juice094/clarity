#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
use clarity_core::agent::{Agent, AgentConfig, MockLlm};
use clarity_core::memory::{Memory, MemoryStore, PersistentMemoryStore};
use clarity_core::registry::ToolRegistry;
use std::sync::Arc;
use tempfile::TempDir;

/// Scenario C — Memory persistence through Core.
/// Create a temporary DB, run an Agent with a PersistentMemoryStore,
/// then reopen the store and verify the data survived.
#[tokio::test]
async fn test_memory_persistence_through_core() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("memory.db");

    // First agent + store instance
    let store = Arc::new(
        PersistentMemoryStore::new(&db_path)
            .await
            .expect("Failed to create PersistentMemoryStore"),
    );

    let registry = ToolRegistry::with_builtin_tools();
    let config = AgentConfig::new().with_max_iterations(2);

    let agent = Agent::with_config(registry, config)
        .with_llm(Arc::new(MockLlm))
        .with_memory(store.clone());

    let result = agent.run("Persist this thought").await;
    assert!(result.is_ok(), "Agent run failed: {:?}", result);

    // Also store a direct fact so we have something deterministic to search for
    store
        .store(Memory::new("Rust is awesome").with_tags(vec!["tech".to_string()]))
        .await
        .expect("Direct store failed");

    // Drop the first store by letting it go out of scope
    drop(store);
    drop(agent);

    // Re-create a new store pointing at the same file
    let store2 = PersistentMemoryStore::new(&db_path)
        .await
        .expect("Failed to reopen PersistentMemoryStore");

    // Verify the directly saved fact is retrievable
    let all = store2.get_all().await.expect("get_all failed");
    assert!(
        all.iter().any(|m| m.content.contains("Rust is awesome")),
        "Expected 'Rust is awesome' in persisted memories: {:?}",
        all
    );

    // Verify search works across the persisted data
    let search = store2
        .search("Persist this thought", 10)
        .await
        .expect("search failed");
    assert!(
        !search.is_empty(),
        "Expected to find the conversation memory after reopening"
    );
}
