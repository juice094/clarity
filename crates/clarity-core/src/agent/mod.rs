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

use std::sync::Arc;
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

/// Lifecycle state of an Agent instance.
#[derive(Debug, Clone)]
pub enum AgentState {
    /// No LLM configured. `run()` is illegal.
    Unconfigured,
    /// Ready to accept a turn. `cancel_token` is guaranteed fresh.
    Idle,
    /// A turn is currently in progress on this (or a cloned) Agent.
    Running {
        /// Snapshot of the turn's token, used by `cancel()`.
        cancel_token: CancellationToken,
    },
    /// Previous turn was cancelled or the inner task panicked.
    /// Requires explicit reset (or implicit reset on next `run()` attempt).
    Stalled,
}

/// Shared mutable runtime state of an Agent.
struct AgentInner {
    state: AgentState,
    llm: Option<Arc<dyn LlmProvider>>,
    session_usage: TokenUsage,
    active_skill: Option<String>,
    /// Snapshotted at turn start so that mid-turn set_active_skill() calls
    /// do not affect the in-flight turn.
    snapshotted_skill: Option<String>,
    file_prompt_cache: Option<String>,
}

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
    /// Optional skill registry for orchestration
    skill_registry: Option<SkillRegistry>,
    /// Shared mutable runtime state
    inner: Arc<std::sync::RwLock<AgentInner>>,
}
