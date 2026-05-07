//! Core shared types for clarity-core
//!
//! Types in this module are used across multiple layers (agent, llm, approval, tools)
//! and are kept here to avoid circular dependencies.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// P2: Re-export contract types so existing `use clarity_core::types::{ToolCall, FunctionCall}`
// continue to work.  Downstream crates may eventually migrate to `clarity_contract` directly.
pub use clarity_contract::{FunctionCall, ToolCall};

/// Normalize a path by resolving `.` and `..` components.
/// Does not require the path to exist and does not add UNC prefixes.
pub fn normalize_path(path: &std::path::Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(p) => result.push(p.as_os_str()),
            std::path::Component::RootDir => result.push(component),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                result.pop();
            }
            std::path::Component::Normal(name) => {
                result.push(name);
            }
        }
    }
    result
}

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
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

/// Runtime state for one step of a plan execution.
#[derive(Debug, Clone)]
pub struct PlanExecutionState {
    pub step_id: String,
    pub status: PlanStepExecutionStatus,
    pub result: Option<PlanResult>,
}

/// Definition of a subagent type.
/// P1-1: Moved from `subagents/registry.rs` to `types.rs` to break the
/// `background↔subagents` circular dependency.
///
/// Risk: `types.rs` now hosts a small registry type (`LaborMarket`).
/// If registry logic grows significantly, consider extracting to a dedicated
/// `agent-types` crate instead of bloating `types.rs`.
#[derive(Debug, Clone)]
pub struct AgentTypeDefinition {
    pub name: String,
    pub description: String,
    pub system_prompt: String,
    pub allowed_tools: Option<Vec<String>>,
    pub max_iterations: usize,
}

/// Registry for subagent types (LaborMarket).
/// P1-1: Moved from `subagents/registry.rs` to `types.rs` to break the
/// `background↔subagents` circular dependency.
#[derive(Clone)]
pub struct LaborMarket {
    types: std::collections::HashMap<String, AgentTypeDefinition>,
}

impl Default for LaborMarket {
    fn default() -> Self {
        Self::new()
    }
}

impl LaborMarket {
    pub fn new() -> Self {
        let mut market = Self {
            types: std::collections::HashMap::new(),
        };
        market.register_builtin_types();
        market
    }

    fn register_builtin_types(&mut self) {
        self.register(AgentTypeDefinition {
            name: "coder".to_string(),
            description: "Code engineering tasks - implementation, refactoring, debugging"
                .to_string(),
            system_prompt: CODER_SYSTEM_PROMPT.to_string(),
            allowed_tools: None,
            max_iterations: 20,
        });
        self.register(AgentTypeDefinition {
            name: "explore".to_string(),
            description: "Codebase exploration and research".to_string(),
            system_prompt: EXPLORE_SYSTEM_PROMPT.to_string(),
            allowed_tools: Some(vec![
                "file_read".to_string(),
                "glob".to_string(),
                "grep".to_string(),
            ]),
            max_iterations: 10,
        });
        self.register(AgentTypeDefinition {
            name: "plan".to_string(),
            description: "Implementation planning and design".to_string(),
            system_prompt: PLAN_SYSTEM_PROMPT.to_string(),
            allowed_tools: Some(vec![
                "file_read".to_string(),
                "glob".to_string(),
                "grep".to_string(),
                "file_write".to_string(),
            ]),
            max_iterations: 5,
        });
    }

    pub fn register(&mut self, type_def: AgentTypeDefinition) {
        self.types.insert(type_def.name.clone(), type_def);
    }

    pub fn get(&self, name: &str) -> Option<&AgentTypeDefinition> {
        self.types.get(name)
    }

    pub fn require(&self, name: &str) -> &AgentTypeDefinition {
        self.get(name)
            .unwrap_or_else(|| panic!("Unknown agent type: {}", name))
    }

    pub fn list(&self) -> Vec<&AgentTypeDefinition> {
        self.types.values().collect()
    }
}

const CODER_SYSTEM_PROMPT: &str = r#"You are a code engineering assistant.
Your task is to implement, refactor, or debug code.

Guidelines:
- Write clean, idiomatic code
- Add appropriate error handling
- Follow existing code style
- Test your changes when possible
"#;

const EXPLORE_SYSTEM_PROMPT: &str = r#"You are a codebase exploration assistant.
Your task is to understand and explain code structure.

Guidelines:
- Use file_read, glob, grep to explore
- Provide clear summaries
- Ask questions if unclear
- Focus on understanding, not changing
"#;

const PLAN_SYSTEM_PROMPT: &str = r#"You are an implementation planning assistant.
Your task is to design solutions before implementation.

Guidelines:
- Explore first to understand context
- Design approach before coding
- Write plan to file if needed
- Get user approval before implementation
"#;
