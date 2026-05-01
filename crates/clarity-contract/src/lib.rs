//! P2: `clarity-contract` — Core contract types shared across the Clarity ecosystem.
//!
//! This crate exists to break the monolithic dependency on `clarity-core`.
//! Downstream crates (egui, tui, gateway, headless) should import fundamental
//! types from here instead of pulling in the entire core runtime.
//!
//! PoC scope (Week 4):
//! - `ToolCall` + `FunctionCall` (the LLM↔Agent interchange types)
//!
//! Risk: Full migration touches every `use clarity_core::types::...` and
//! `use clarity_core::llm::api::...` statement across ~6 crates.  The PoC
//! keeps `clarity-core` re-exporting these types so downstream code does
//! not break yet.  A coordinated migration sprint is needed before the
//! re-exports can be removed.
//!
//! Future expansions:
//! - `Message`, `MessageRole`, `StreamDelta`
//! - `AgentError`, `ToolError`
//! - `LlmProvider` trait

use serde::{Deserialize, Serialize};

/// A tool call from the LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// Function call details.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // JSON string
}
