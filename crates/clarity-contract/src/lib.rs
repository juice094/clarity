#![cfg_attr(test, allow(clippy::unwrap_used, clippy::expect_used, clippy::panic))]
//! `clarity-contract` — Core contract types shared across the Clarity ecosystem.
//!
//! This crate exists to break the monolithic dependency on `clarity-core`.
//! Downstream crates (egui, tui, gateway, headless, future plugin SDK) should import
//! fundamental types from here instead of pulling in the entire core runtime.
//!
//! ## Status: Phase 3 — Federation Contract Layer (May 2026)
//!
//! ### Extracted types (done)
//!
//! | Type | Origin | Consumer |
//! |------|--------|---------|
//! | `ToolCall`, `FunctionCall` | `core::types` | Agent, LLM, Tools |
//! | `Message`, `MessageRole` | `core::llm::api` | Agent, LLM, Compaction |
//! | `StreamDelta` | `core::llm::api` | LLM streaming |
//! | `ToolError` | `core::error` | Tool execution |
//! | `ContractError` | **new** | Cross-crate federation |
//! | `LlmProvider` (trait) | `core::llm::api` | All LLM consumers |
//! | `LlmResponse` | `core::llm::api` | LLM consumers |
//! | `Tool` (trait) | `core::tools` | All tool consumers |
//! | `ToolContext` | `core::tools` | Tool execution |
//! | `ApprovalMode` | `core::approval` | Tool execution |
//! | `FederationNode` (trait) | **new** | Claw runtime |
//! | `FederationMessage` | **new** | Claw runtime |
//! | `Capability` | **new** | Claw runtime |
//!
//! ### Migration strategy
//!
//! 1. Define type in this crate
//! 2. `clarity-core` re-exports via `pub use clarity_contract::*` in the original module
//! 3. All existing `use clarity_core::...::TypeName` continue to work
//! 4. Downstream crates migrate to `use clarity_contract::TypeName` at their own pace
//! 5. Re-exports are removed only after all downstream crates have migrated

// 临时豁免：从 clarity-core 迁出的大量类型/字段尚未补全文档，后续统一补充。
#![allow(missing_docs)]

use serde::{Deserialize, Serialize};

// ============================================================================
// Modules
// ============================================================================

pub mod capability;
pub mod error;
pub mod federation;
pub mod llm;
pub mod reliable_provider;
pub mod rollout;
pub mod subagent;
pub mod thread;
pub mod tool;

// ============================================================================
// Phase 1: Core interchange types
// ============================================================================

/// A tool call from the LLM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    /// Unique identifier for this tool call.
    pub id: String,
    /// Tool call type (typically `"function"`).
    #[serde(rename = "type")]
    pub call_type: String,
    /// Function call details.
    pub function: FunctionCall,
}

/// Function call details.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunctionCall {
    /// Name of the function to invoke.
    pub name: String,
    /// JSON-encoded arguments for the function.
    pub arguments: String, // JSON string
}

// ============================================================================
// Phase 2: Message types
// ============================================================================

/// LLM message role.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System prompt message.
    System,
    /// User input message.
    User,
    /// Assistant-generated message.
    Assistant,
    /// Tool response message.
    Tool,
}

/// A message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    /// Role of the message sender.
    pub role: MessageRole,
    /// Text content of the message.
    pub content: String,
    /// Tool calls requested by the assistant.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Identifier of the tool call this message is responding to.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

impl Message {
    /// Create a system message.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::System,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a user message.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::User,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create an assistant message.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Assistant,
            content: content.into(),
            tool_calls: None,
            tool_call_id: None,
        }
    }

    /// Create a tool response message.
    pub fn tool(tool_call_id: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            role: MessageRole::Tool,
            content: content.into(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id.into()),
        }
    }
}

/// Delta emitted by a streaming LLM response.
#[derive(Debug, Clone, Default)]
pub struct StreamDelta {
    /// Text chunk emitted by the model, if any.
    pub content: Option<String>,
    /// Reasoning / thinking chunk emitted by the model, if any.
    /// Providers that do not expose reasoning (e.g. OpenAI gpt-4o) leave this as `None`.
    pub reasoning_content: Option<String>,
    /// Tool calls parsed from the stream so far.
    pub tool_calls: Vec<ToolCall>,
}

impl StreamDelta {
    /// Convenience constructor for a plain text delta.
    pub fn content(text: impl Into<String>) -> Self {
        Self {
            content: Some(text.into()),
            ..Default::default()
        }
    }

    /// Convenience constructor for a reasoning/thinking delta.
    pub fn reasoning(text: impl Into<String>) -> Self {
        Self {
            reasoning_content: Some(text.into()),
            ..Default::default()
        }
    }

    /// Check if this delta contains any meaningful data.
    pub fn is_empty(&self) -> bool {
        self.content.is_none() && self.reasoning_content.is_none() && self.tool_calls.is_empty()
    }
}

// ============================================================================
// Re-exports from submodules
// ============================================================================

pub use error::{
    AgentError, ContractError, ContractResult, ToolError, ToolResult, sanitize_path_str,
};
pub use federation::{
    Capability, Fact, FederationMessage, FederationNode, FederationResponse, NodeStatus, TaskSpec,
    ToolSpec,
};
pub use llm::{LlmProvider, LlmProviderFactory, LlmResponse, Pricing, ProviderCapabilities};
pub use reliable_provider::ReliableProvider;
pub use rollout::{
    CompactedItem, CreateRolloutParams, GitInfo, ResumeRolloutParams, RolloutEventMsg, RolloutItem,
    RolloutLine, RolloutResponseItem, SessionMeta, SessionMetaLine, SessionSource, ThreadSource,
    TurnContextItem,
};
pub use subagent::{
    AgentTeam, AgentTypeDefinition, BatchProgress, BatchProgressHandle, BatchStatus,
    CapabilityToken, ExecutionStatus, GitContext, LaborMarket, Mailbox, MailboxError,
    MailboxMessage, MessagePayload, ParallelConfig, ParallelResult, RunSpec, SubagentError,
    SubagentOrchestrator, SubagentProgressEvent, SubagentResult, SubagentState, SubagentStatus,
    TeamResult, TokenError, collect_git_context,
};
pub use thread::{SessionId, ThreadId};
pub use tool::{ApprovalMode, BoxedTool, IntoSharedTool, SharedTool, Tool, ToolContext};

#[cfg(test)]
mod tests;
