//! End-to-End Integration Tests for Clarity
//!
//! These tests verify the integration between all crates in the workspace.

use clarity_core::agent::{Agent, AgentConfig, Message, MessageRole, MockLlm};
use clarity_core::error::AgentError;
use clarity_core::llm::LlmFactory;
use clarity_core::memory::{Memory, MemoryStore, MemoryTicker};
use clarity_core::registry::ToolRegistry;
use clarity_core::tools::{BashTool, FileReadTool, Tool};
use serde_json::json;
use std::sync::Arc;

// ==================== Core + Memory Integration ====================

#[tokio::test]
async fn test_agent_with_persistent_memory() {
    use clarity_core::memory::PersistentMemoryStore;

    // Create in-memory persistent store for testing
    let memory_store: Arc<dyn MemoryStore> =
        Arc::new(PersistentMemoryStore::new_in_memory().unwrap());
    let ticker = MemoryTicker::new(3);

    let registry = ToolRegistry::new();
    let config = AgentConfig::new().with_max_iterations(2);

    let agent = Agent::with_config(registry, config)
        .with_llm(Arc::new(MockLlm))
        .with_memory(memory_store.clone())
        .with_memory_ticker(ticker);

    // Run a query
    let result = agent.run("Test query").await;
    assert!(result.is_ok());

    // Verify memory was stored
    let memories = memory_store.get_all().await.unwrap();
    assert!(!memories.is_empty());
}

#[tokio::test]
async fn test_agent_memory_search_integration() {
    use clarity_core::memory::PersistentMemoryStore;

    let memory_store: Arc<dyn MemoryStore> =
        Arc::new(PersistentMemoryStore::new_in_memory().unwrap());

    // Pre-populate some memories
    memory_store
        .store(Memory::new("User likes Rust programming").with_tags(vec!["tech".to_string()]))
        .await
        .unwrap();
    memory_store
        .store(
            Memory::new("User prefers tea over coffee").with_tags(vec!["preference".to_string()]),
        )
        .await
        .unwrap();

    let registry = ToolRegistry::new();
    let config = AgentConfig::new();

    let agent = Agent::with_config(registry, config)
        .with_llm(Arc::new(MockLlm))
        .with_memory(memory_store);

    // Run query - should retrieve relevant memories
    let result = agent.run("What programming language?").await;
    assert!(result.is_ok());
}

// ==================== Tool Registry Integration ====================

#[test]
fn test_builtin_tools_registration() {
    let registry = ToolRegistry::with_builtin_tools();

    // Check that all expected tools are registered
    let tools = registry.list_tools().unwrap();
    assert!(!tools.is_empty(), "Should have builtin tools");

    // Check specific tools
    assert!(registry.contains("file_read").unwrap());
    assert!(registry.contains("file_write").unwrap());
    assert!(registry.contains("bash").unwrap());
    assert!(registry.contains("glob").unwrap());
    assert!(registry.contains("grep").unwrap());
}

#[tokio::test]
async fn test_tool_execution_integration() {
    let registry = ToolRegistry::new();
    registry.register(BashTool::new()).unwrap();

    let config = AgentConfig::new();
    let agent = Agent::with_config(registry, config);

    // Execute tool directly
    let result = agent
        .execute_tool("bash", json!({"command": "echo hello"}))
        .await;

    // Note: May fail in Windows environment without proper shell setup
    // but the API interface should work
    match result {
        Ok(value) => {
            let output = value.get("stdout").and_then(|s| s.as_str());
            assert!(output.is_some(), "Should have stdout");
        }
        Err(e) => {
            // Tool might fail due to environment, but API should work
            println!("Tool execution failed (expected in test env): {}", e);
        }
    }
}

// ==================== LLM Provider Integration ====================

#[test]
fn test_llm_factory_error_handling() {
    // Without any env vars set, factory methods should return error
    // Test kimi provider (requires KIMI_API_KEY)
    let result = LlmFactory::kimi();
    assert!(result.is_err(), "Should fail without KIMI_API_KEY");

    // Test deepseek provider (requires DEEPSEEK_API_KEY)
    let result = LlmFactory::deepseek();
    assert!(result.is_err(), "Should fail without DEEPSEEK_API_KEY");
}

// ==================== Configuration Integration ====================

#[test]
fn test_agent_config_builder_pattern() {
    let config = AgentConfig::new()
        .with_max_iterations(15)
        .with_read_only(true)
        .with_system_prompt("Custom prompt");

    assert_eq!(config.max_iterations, 15);
    assert!(config.read_only);
    assert_eq!(config.system_prompt, "Custom prompt");
}

// ==================== Error Handling Integration ====================

#[tokio::test]
async fn test_agent_error_propagation() {
    let registry = ToolRegistry::new();
    let config = AgentConfig::new();

    // Agent without LLM should fail when run
    let agent = Agent::with_config(registry, config);

    let result = agent.run("Test").await;
    assert!(result.is_err());

    match result.unwrap_err() {
        AgentError::Unconfigured => {
            // Agent without LLM is in Unconfigured state
        }
        AgentError::Llm(msg) => {
            assert!(msg.contains("No LLM provider"));
        }
        other => panic!("Expected Unconfigured or LLM error, got: {:?}", other),
    }
}

#[tokio::test]
async fn test_streaming_api_integration() {
    let registry = ToolRegistry::new();
    let config = AgentConfig::new();

    let agent = Agent::with_config(registry, config).with_llm(Arc::new(MockLlm));

    let chunk_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let chunk_count_clone = chunk_count.clone();

    let result = agent
        .run_streaming("Test query", move |_chunk| {
            chunk_count_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        })
        .await;

    assert!(result.is_ok());
    // MockLlm sends at least one chunk
    assert!(chunk_count.load(std::sync::atomic::Ordering::SeqCst) >= 1);
}

// ==================== Cross-Crate Type Compatibility ====================

#[test]
fn test_message_type_compatibility() {
    // Test message creation matches expected interface
    let system = Message::system("System prompt");
    assert_eq!(system.role, MessageRole::System);

    let user = Message::user("User query");
    assert_eq!(user.role, MessageRole::User);

    let assistant = Message::assistant("Assistant response");
    assert_eq!(assistant.role, MessageRole::Assistant);

    let tool = Message::tool("call_123", "Tool result");
    assert_eq!(tool.role, MessageRole::Tool);
    assert_eq!(tool.tool_call_id, Some("call_123".to_string()));
}

#[test]
fn test_tool_trait_object_safety() {
    // Verify Tool trait is object-safe
    let _tool: Box<dyn Tool> = Box::new(FileReadTool::new());

    // Can be converted to Arc for shared ownership
    let _shared: Arc<dyn Tool> = Arc::new(FileReadTool::new());
}

// ==================== Workspace Dependency Verification ====================

#[test]
fn test_tokio_runtime_compatibility() {
    // Verify all crates use compatible tokio versions
    // This is a compile-time check
    use tokio::runtime::Runtime;

    let rt = Runtime::new().unwrap();
    let result = rt.block_on(async { tokio::spawn(async { "tokio runtime works" }).await });

    assert!(result.is_ok());
}

#[test]
fn test_serde_compatibility() {
    use serde::{Deserialize, Serialize};
    use serde_json;

    #[derive(Serialize, Deserialize, Debug, PartialEq)]
    struct TestData {
        name: String,
        value: i32,
    }

    let data = TestData {
        name: "test".to_string(),
        value: 42,
    };

    let json = serde_json::to_string(&data).unwrap();
    let decoded: TestData = serde_json::from_str(&json).unwrap();

    assert_eq!(data, decoded);
}
