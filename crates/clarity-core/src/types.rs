//! Core shared types for clarity-core
//!
//! Types in this module are used across multiple layers (agent, llm, approval, tools)
//! and are kept here to avoid circular dependencies.

use serde::{Deserialize, Serialize};

/// A tool call from the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// Function call details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // JSON string
}
