//! LLM API types — shared contracts for LLM communication
//!
//! These types define the interface between the Agent and LLM providers.
//! They are kept separate from `agent/mod.rs` to avoid circular dependencies
//! (e.g., `llm/`, `compaction/`, and `subagents/` should not depend on `agent/`).
//!
//! ## Type origin
//!
//! All types in this module are defined in `clarity-contract` and re-exported
//! here for backward compatibility. New code should import directly from
//! `clarity_contract`.

// Re-export all contract types so existing imports continue to work.
pub use clarity_contract::{
    LlmProvider, LlmResponse, Message, MessageRole, StreamDelta, ToolCall,
};
pub use clarity_contract::llm::{Pricing, ProviderCapabilities};
