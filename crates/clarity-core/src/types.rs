//! Core shared types for clarity-core
//!
//! Types in this module are used across multiple layers (agent, llm, approval, tools)
//! and are kept here to avoid circular dependencies.

use serde::{Deserialize, Serialize};

// P2: Re-export contract types so existing `use clarity_core::types::{ToolCall, FunctionCall}`
// continue to work.  Downstream crates may eventually migrate to `clarity_contract` directly.
pub use clarity_contract::{FunctionCall, ToolCall};

// P1-2: Re-export subagent contract types that were previously defined here.
pub use clarity_contract::subagent::{AgentTypeDefinition, LaborMarket};

/// A single step inside an execution plan.
/// B3: Moved from `agent/plan.rs` to `types.rs` to reduce `agent` module coupling.
/// Downstream crates should import from `clarity_core::types` or via the re-export
/// in `clarity_core::agent` (kept for backwards compatibility).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanStep {
    /// Human-readable identifier (e.g. "1", "2a")
    pub id: String,
    /// What this step does in plain language.
    pub description: String,
    /// The tool to invoke (must exist in the registry).
    pub tool_name: String,
    /// JSON payload for the tool call.
    #[serde(default)]
    pub tool_params: serde_json::Value,
}

/// B3: A structured execution plan produced by `Agent::plan()`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Plan {
    /// Short title summarising the plan.
    pub title: String,
    /// Ordered steps to execute.
    pub steps: Vec<PlanStep>,
}

/// B3: Result of executing a single plan step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanResult {
    /// The step that was executed.
    pub step_id: String,
    /// Whether the tool call succeeded.
    pub success: bool,
    /// Stringified tool output (or error message).
    pub output: String,
}

/// Runtime execution status of a single plan step.
/// Distinct from the LLM-generated `PlanStep` — this tracks mutable runtime state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanStepExecutionStatus {
    /// Task is pending.
    Pending,
    /// Task is running.
    Running,
    /// Successful execution.
    Success,
    /// Task failed.
    Failed,
    /// Step was skipped.
    Skipped,
}

/// Runtime state for one step of a plan execution.
#[derive(Debug, Clone)]
pub struct PlanExecutionState {
    /// Step identifier.
    pub step_id: String,
    /// Tool status.
    pub status: PlanStepExecutionStatus,
    /// Serialized tool result.
    pub result: Option<PlanResult>,
}
