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

use serde::{Deserialize, Serialize};

// ============================================================================
// Modules
// ============================================================================

pub mod capability;
pub mod error;
pub mod federation;
pub mod llm;
pub mod tool;

// ============================================================================
// Phase 1: Core interchange types
// ============================================================================

/// A tool call from the LLM.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// Function call details.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // JSON string
}

// ============================================================================
// Phase 2: Message types
// ============================================================================

/// LLM message role.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    Tool,
}

/// A message in a conversation.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
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
    pub content: Option<String>,
    pub tool_calls: Vec<ToolCall>,
}

impl StreamDelta {
    /// Check if this delta contains any meaningful data.
    pub fn is_empty(&self) -> bool {
        self.content.is_none() && self.tool_calls.is_empty()
    }
}

// ============================================================================
// Re-exports from submodules
// ============================================================================

pub use capability::{CapabilityToken, TokenError};
pub use error::{AgentError, ContractError, ContractResult, ToolError, ToolResult, sanitize_path_str};
pub use federation::{
    Capability, Fact, FederationMessage, FederationNode, FederationResponse, NodeStatus,
    TaskSpec, ToolSpec,
};
pub use llm::{LlmProvider, LlmResponse};
pub use tool::{ApprovalMode, BoxedTool, IntoSharedTool, SharedTool, Tool, ToolContext};

#[cfg(test)]
mod tests;
