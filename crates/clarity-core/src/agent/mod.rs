//! Agent Loop - Core orchestration component
//!
//! The `Agent` manages the interaction loop between the LLM and tools.
//! It handles:
//! - Tool discovery for LLM
//! - Request routing to appropriate tools
//! - Context management
//! - Iteration limits and safety
//! - Error recovery with retry
//! - Execution tracing
//! - State persistence
//! - Parallel tool execution

pub mod compaction_service;
pub mod config;
pub mod controller;
pub mod enhanced;
pub mod ops;

mod construct;
mod execution;
mod prompt;
mod run;

#[cfg(test)]
mod tests;

use crate::agent::compaction_service::CompactionService;
use crate::approval::{ApprovalMode, ApprovalRuntime};
use crate::compaction::CompactionConfig;
use crate::error::AgentError;
use crate::memory::{MemoryStore, MemoryTicker};
use crate::registry::ToolRegistry;
use crate::skills::SkillRegistry;
use clarity_wire::Wire;

use std::sync::{Arc, Mutex};
use tokio_util::sync::CancellationToken;

// Re-export enhanced features
pub use controller::{AgentController, ControllerEvent};
pub use enhanced::{
    ConversationState, ErrorRecovery, ErrorRecoveryConfig, ExecutionStep, ExecutionSummary,
    ExecutionTracer, ParallelToolExecutor, RecoveryStrategy, StatePersistence, StepType,
    TokenUsage,
};
pub use ops::Op;

// Re-export core API types from their canonical locations for backwards compatibility.
// New code should import directly from `crate::types` or `crate::llm::api`.
pub use crate::llm::api::{LlmProvider, LlmResponse, Message, MessageRole, StreamDelta};
pub use crate::types::{FunctionCall, ToolCall};

// Re-export config
pub use config::AgentConfig;

/// Simple mock LLM for testing
pub struct MockLlm;

#[async_trait::async_trait]
impl LlmProvider for MockLlm {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &serde_json::Value,
    ) -> Result<LlmResponse, crate::error::AgentError> {
        Ok(LlmResponse {
            content: "This is a mock response".to_string(),
            tool_calls: vec![],
            is_complete: true,
        })
    }

    fn stream(
        &self,
        _messages: &[Message],
        _tools: &serde_json::Value,
    ) -> Result<
        tokio::sync::mpsc::Receiver<Result<StreamDelta, crate::error::AgentError>>,
        crate::error::AgentError,
    > {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        tokio::spawn(async move {
            let _ = tx
                .send(Ok(StreamDelta {
                    content: Some("This is a mock response".to_string()),
                    tool_calls: vec![],
                }))
                .await;
        });
        Ok(rx)
    }

    fn set_prompt_cache_key(&mut self, _key: &str) {}
}

/// The main Agent struct
///
/// Manages the interaction between user, LLM, and tools.
///
/// # Example
///
/// ```rust,no_run
/// use clarity_core::{Agent, ToolRegistry};
/// use clarity_core::agent::AgentConfig;
///
/// #[tokio::main]
/// async fn main() -> anyhow::Result<()> {
///     let registry = ToolRegistry::with_builtin_tools();
///
///     let config = AgentConfig::new()
///         .with_max_iterations(10)
///         .with_read_only(false);
///
///     let agent = Agent::with_config(registry, config);
///
///     // This would need an actual LLM provider
///     // let response = agent.run("List all Rust files").await?;
///
///     Ok(())
/// }
/// ```
#[derive(Clone)]
pub struct Agent {
    registry: ToolRegistry,
    config: AgentConfig,
    llm: Arc<std::sync::RwLock<Option<Arc<dyn LlmProvider>>>>,
    memory_store: Option<Arc<dyn MemoryStore>>,
    memory_ticker: Option<MemoryTicker>,
    /// Optional wire for UI communication
    wire: Option<Arc<Wire>>,
    /// Approval runtime for tool execution control
    approval_runtime: Option<Arc<dyn ApprovalRuntime>>,
    /// Approval mode (Interactive, Yolo, Plan)
    approval_mode: ApprovalMode,
    /// Compaction configuration for context management
    compaction_config: CompactionConfig,
    /// Maximum context tokens before compaction
    max_context_tokens: usize,
    /// Optional compaction service for proactive history compression
    compaction_service: Option<CompactionService>,
    /// Cancellation token for interrupting in-flight runs
    cancel_token: CancellationToken,
    /// Session token usage accumulator
    session_usage: Arc<Mutex<TokenUsage>>,
    /// Optional skill registry for orchestration
    skill_registry: Option<SkillRegistry>,
    /// Currently active skill id (shared across clones)
    active_skill: Arc<std::sync::RwLock<Option<String>>>,
    /// Cached file prompt to avoid repeated disk I/O in build_system_prompt
    file_prompt_cache: Arc<std::sync::RwLock<Option<String>>>,
}
