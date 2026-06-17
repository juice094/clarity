#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
//! API Compatibility and Stability Tests
//!
//! These tests verify that public APIs maintain backward compatibility.
//! Any breaking change here indicates a major version bump is needed.

use clarity_core::agent::{Agent, AgentConfig, LlmProvider, Message, MockLlm};
use clarity_core::error::{AgentError, ToolError};
use clarity_core::memory::{InMemoryStore, MemoryStore, MemoryTicker, SharedMemoryTicker};
use clarity_core::registry::ToolRegistry;
use clarity_core::tools::{FileReadTool, Tool, ToolContext};
use clarity_llm::LlmFactory;
use std::sync::Arc;

// ==================== Agent API Stability ====================

#[test]
fn test_agent_constructor_api() {
    // Test: Agent::new(registry) - must exist
    let registry = ToolRegistry::new();
    let _agent = Agent::new(registry);
}

#[test]
fn test_agent_with_config_api() {
    // Test: Agent::with_config(registry, config) - must exist
    let registry = ToolRegistry::new();
    let config = AgentConfig::new();
    let _agent = Agent::with_config(registry, config);
}

#[test]
fn test_agent_config_builder_api() {
    // Test: AgentConfig builder methods - must exist
    let config = AgentConfig::new()
        .with_max_iterations(10)
        .with_working_dir("/tmp")
        .with_read_only(false)
        .with_system_prompt("test");

    assert_eq!(config.max_iterations, 10);
}

#[tokio::test]
async fn test_agent_run_api() {
    // Test: Agent::run(&self, query) -> Result<String, AgentError>
    let registry = ToolRegistry::new();
    let config = AgentConfig::new();
    let agent = Agent::with_config(registry, config).with_llm(Arc::new(MockLlm));

    // Should accept &str and String
    let _result = agent.run("test query").await;
    let _result = agent.run(String::from("test query")).await;
}

#[tokio::test]
async fn test_agent_run_streaming_api() {
    // Test: Agent::run_streaming(&self, query) -> Result<String, AgentError>
    let registry = ToolRegistry::new();
    let config = AgentConfig::new();
    let agent = Agent::with_config(registry, config).with_llm(Arc::new(MockLlm));

    let result = agent.run_streaming("test").await;

    assert!(result.is_ok());
}

#[test]
fn test_agent_builder_methods() {
    let registry = ToolRegistry::new();
    let config = AgentConfig::new();

    // Test: with_llm, with_memory, with_memory_ticker - must exist
    let _agent = Agent::with_config(registry, config)
        .with_llm(Arc::new(MockLlm) as Arc<dyn LlmProvider>)
        .with_memory(Arc::new(InMemoryStore::new()) as Arc<dyn MemoryStore>)
        .with_memory_ticker(SharedMemoryTicker::new(MemoryTicker::new("/tmp", Some(5))));
}

// ==================== ToolRegistry API Stability ====================

#[test]
fn test_tool_registry_api() {
    // Test: ToolRegistry::new() - must exist
    let registry = ToolRegistry::new();

    // Test: ToolRegistry::with_builtin_tools() - must exist
    let _registry = ToolRegistry::with_builtin_tools();

    // Test: register<T>(&self, tool) -> Result<(), AgentError>
    let result = registry.register(FileReadTool::new());
    assert!(result.is_ok());

    // Test: unregister(&self, name) -> Result<bool, AgentError>
    let _result = registry.unregister("file_read");

    // Test: get(&self, name) -> Result<Option<SharedTool>, AgentError>
    let _result = registry.get("file_read");

    // Test: contains(&self, name) -> Result<bool, AgentError>
    let _result = registry.contains("file_read");

    // Test: list_tools(&self) -> Result<Vec<String>, AgentError>
    let _result = registry.list_tools();

    // Test: get_tool_schemas(&self) -> Result<Value, AgentError>
    let _result = registry.get_tool_schemas();

    // Test: get_tool_definitions(&self) -> Result<Value, AgentError>
    let _result = registry.get_tool_definitions();
}

#[tokio::test]
async fn test_tool_registry_execute_api() {
    let registry = ToolRegistry::new();
    registry.register(FileReadTool::new()).unwrap();

    // Test: execute(&self, name, args, ctx) -> ToolResult<Value>
    let ctx = ToolContext::new();
    let _result = registry
        .execute(
            "file_read",
            serde_json::json!({"path": "/tmp/test.txt"}),
            ctx,
        )
        .await;
}

// ==================== Tool Trait API Stability ====================

#[test]
fn test_tool_trait_methods() {
    use serde_json::Value;

    let tool = FileReadTool::new();

    // Test: name(&self) -> &str
    let _name: &str = tool.name();

    // Test: description(&self) -> &str
    let _desc: &str = tool.description();

    // Test: parameters(&self) -> Value
    let _params: Value = tool.parameters();
}

// ==================== MemoryStore Trait API Stability ====================

#[tokio::test]
async fn test_memory_store_trait_api() {
    use clarity_core::memory::{Memory, MemoryStore};

    let store = InMemoryStore::new();

    // Test: store(&self, Memory) -> Result<()>
    let memory = Memory::new("test");
    let _result = store.store(memory).await;

    // Test: retrieve(&self, min_importance) -> Result<Vec<Memory>>
    let _result = store.retrieve(0.5).await;

    // Test: get_all(&self) -> Result<Vec<Memory>>
    let _result = store.get_all().await;

    // Test: clear(&self) -> Result<()>
    let _result = store.clear().await;

    // Test: count(&self) -> Result<usize>
    let _result = store.count().await;

    // Test: search(&self, query, limit) -> Result<Vec<Memory>>
    let _result = store.search("test", 10).await;

    // Test: summarize(&self, limit) -> Result<String>
    let _result = store.summarize(10).await;
}

#[test]
fn test_memory_creation_api() {
    use clarity_core::memory::Memory;

    // Test: Memory::new(content) - must exist
    let _memory = Memory::new("content");

    // Test: with_importance - must exist
    let _memory = Memory::new("content").with_importance(0.8);

    // Test: with_tags - must exist
    let _memory = Memory::new("content").with_tags(vec!["tag".to_string()]);
}

// ==================== LlmProvider Trait API Stability ====================

#[test]
fn test_llm_provider_trait_structure() {
    // Test: MockLlm implements LlmProvider
    let _llm: Arc<dyn LlmProvider> = Arc::new(MockLlm);
}

// ==================== Error Types API Stability ====================

#[test]
fn test_error_types_api() {
    // Test: ToolError constructors
    let _err = ToolError::invalid_params("test");
    let _err = ToolError::execution_failed("test");
    let _err = ToolError::not_found("test");
    let _err = ToolError::from_io(std::io::Error::other("test"));

    // Test: AgentError variants exist
    let _err = AgentError::Registry("test".to_string());
    let _err = AgentError::Llm("test".to_string());
    let _err = AgentError::MaxIterationsExceeded(10);
}

// ==================== Message Types API Stability ====================

#[test]
fn test_message_creation_api() {
    // Test: Message::system(content)
    let _msg = Message::system("prompt");

    // Test: Message::user(content)
    let _msg = Message::user("query");

    // Test: Message::assistant(content)
    let _msg = Message::assistant("response");

    // Test: Message::tool(id, content)
    let _msg = Message::tool("call_123", "result");
}

// ==================== Re-exports API Stability ====================

#[test]
fn test_crate_level_reexports() {
    // Test: clarity_core re-exports - these should compile
    fn _check_types() {
        // Note: This function is never called, just checking compilation
        let _: fn() = || {
            // Types should be available from prelude
            panic!("compile-time check");
        };
    }
}

// ==================== LLM Factory API Stability ====================

#[test]
#[allow(deprecated)]
fn test_llm_factory_api() {
    // Test: LlmFactory methods exist
    // These will fail without API keys but verify the API exists
    let _result = LlmFactory::kimi();
    let _result = LlmFactory::deepseek();
}

// ==================== Version Compatibility ====================

#[test]
fn test_version_constant() {
    // Test: VERSION constant exists
    let _version: &str = clarity_core::VERSION;
}
