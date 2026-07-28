//! Subagent orchestration tools: Agent, AgentSwarm
//!
//! Exposes the existing `clarity-subagents` runtime (via the contract-level
//! [`SubagentOrchestrator`] trait) to the LLM as built-in tools:
//! - `agent`: run a single subagent synchronously and return its summary.
//! - `agent_swarm`: fan one prompt template (with a `{{item}}` placeholder)
//!   out over many items in parallel and return an aggregated summary.

use async_trait::async_trait;
use serde_json::{Value, json};
use std::sync::Arc;

use crate::error::ToolError;
use crate::tools::helpers;
use crate::tools::{Tool, ToolContext, ToolResult};
use clarity_contract::subagent::{ParallelConfig, RunSpec, SubagentOrchestrator};

/// Default timeout for subagent tool runs (30 minutes, aligned with the
/// subagent execution budget used elsewhere in the runtime).
const DEFAULT_TIMEOUT_SECS: u64 = 1800;

/// Shared orchestrator reference used by the subagent tools.
fn require_orchestrator(
    orchestrator: &Option<Arc<dyn SubagentOrchestrator>>,
) -> ToolResult<Arc<dyn SubagentOrchestrator>> {
    orchestrator.clone().ok_or_else(|| {
        ToolError::execution_failed(
            "Subagent orchestrator not configured for agent tool. \
             Please ensure the application injected a SubagentOrchestrator \
             (agent.with_orchestrator()) before invoking agent tools."
                .to_string(),
        )
    })
}

/// Build a [`RunSpec`] from shared tool arguments.
fn build_spec(description: &str, prompt: String, args: &Value) -> RunSpec {
    let agent_type = helpers::optional_str(args, "subagent_type").unwrap_or("coder");
    let mut spec = RunSpec::new(description, prompt).with_type(agent_type);
    if let Some(model) = helpers::optional_str(args, "model") {
        spec = spec.with_model(model);
    }
    if let Some(resume) = helpers::optional_str(args, "resume") {
        spec = spec.with_resume(resume);
    }
    if let Some(max) = args.get("max_iterations").and_then(|v| v.as_u64()) {
        spec = spec.with_max_iterations(max as usize);
    }
    spec
}

/// Tool for running a single subagent synchronously.
///
/// The tool blocks until the subagent finishes and returns its result
/// summary, so the parent agent sees the outcome directly in the tool
/// result of the current turn.
pub struct AgentTool {
    orchestrator: Option<Arc<dyn SubagentOrchestrator>>,
}

impl AgentTool {
    /// Create a new tool instance without an orchestrator reference.
    ///
    /// The tool will return an error at execution time unless an
    /// orchestrator is provided via [`Self::with_orchestrator`].
    pub fn new() -> Self {
        Self { orchestrator: None }
    }

    /// Create a tool bound to a specific [`SubagentOrchestrator`].
    pub fn with_orchestrator(orchestrator: Arc<dyn SubagentOrchestrator>) -> Self {
        Self {
            orchestrator: Some(orchestrator),
        }
    }
}

impl Default for AgentTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for AgentTool {
    fn check_readiness(&self) -> Option<String> {
        if self.orchestrator.is_none() {
            Some(
                "Subagent orchestrator not configured. Call agent.with_orchestrator() first."
                    .to_string(),
            )
        } else {
            None
        }
    }

    fn name(&self) -> &str {
        "agent"
    }

    fn description(&self) -> &str {
        "Spawn a single subagent to handle a task and wait for its result. \
         Use this to delegate focused subtasks (exploration, coding, planning) \
         to a specialized agent. Returns the subagent's result summary."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The task or instruction for the subagent to execute"
                },
                "description": {
                    "type": "string",
                    "description": "Optional short human-readable description of the task"
                },
                "subagent_type": {
                    "type": "string",
                    "description": "Subagent type to use: coder, explore, plan, or another registered type (default: coder)"
                },
                "resume": {
                    "type": "string",
                    "description": "Optional agent ID of a previous subagent run to resume"
                },
                "model": {
                    "type": "string",
                    "description": "Optional model override"
                },
                "max_iterations": {
                    "type": "integer",
                    "description": "Optional maximum number of agent iterations"
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": "Timeout in seconds (default: 1800)"
                }
            },
            "required": ["prompt"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let prompt = helpers::required_str(&args, "prompt")?;
        let orchestrator = require_orchestrator(&self.orchestrator)?;

        let description = helpers::optional_str(&args, "description")
            .map(str::to_string)
            .unwrap_or_else(|| prompt.chars().take(50).collect());
        let spec = build_spec(&description, prompt.to_string(), &args);

        let timeout = args
            .get("timeout_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_SECS);
        let config = ParallelConfig::new()
            .with_max_concurrency(1)
            .with_timeout(timeout);

        let result = orchestrator
            .run_parallel(vec![spec], config, None)
            .await
            .map_err(|e| ToolError::execution_failed(format!("Subagent run failed: {}", e)))?;

        if let Some((desc, err)) = result.failures.into_iter().next() {
            return Err(ToolError::execution_failed(format!(
                "Subagent '{}' failed: {}",
                desc, err
            )));
        }

        let run = result.results.into_iter().next().ok_or_else(|| {
            ToolError::execution_failed("Subagent run returned no result".to_string())
        })?;

        Ok(json!({
            "agent_id": run.agent_id,
            "agent_type": run.agent_type,
            "status": run.status.to_string(),
            "summary": run.summary,
            "resumed": run.resumed,
            "steps_taken": run.steps_taken,
            "elapsed_ms": run.elapsed_ms,
        }))
    }
}

/// Tool for running many subagents in parallel from one prompt template.
///
/// The template must contain a `{{item}}` placeholder; one subagent is
/// spawned per item with the placeholder replaced. Results are aggregated
/// into a single summary.
pub struct AgentSwarmTool {
    orchestrator: Option<Arc<dyn SubagentOrchestrator>>,
}

impl AgentSwarmTool {
    /// Create a new tool instance without an orchestrator reference.
    pub fn new() -> Self {
        Self { orchestrator: None }
    }

    /// Create a tool bound to a specific [`SubagentOrchestrator`].
    pub fn with_orchestrator(orchestrator: Arc<dyn SubagentOrchestrator>) -> Self {
        Self {
            orchestrator: Some(orchestrator),
        }
    }
}

impl Default for AgentSwarmTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for AgentSwarmTool {
    fn check_readiness(&self) -> Option<String> {
        if self.orchestrator.is_none() {
            Some(
                "Subagent orchestrator not configured. Call agent.with_orchestrator() first."
                    .to_string(),
            )
        } else {
            None
        }
    }

    fn name(&self) -> &str {
        "agent_swarm"
    }

    fn description(&self) -> &str {
        "Run multiple subagents in parallel: provide a prompt_template containing \
         a {{item}} placeholder and an items array; one subagent is spawned per item \
         with the placeholder replaced. Returns an aggregated summary of all results."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "prompt_template": {
                    "type": "string",
                    "description": "Prompt template containing a {{item}} placeholder, e.g. 'Review {{item}} for regressions'"
                },
                "items": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "List of items; one subagent is spawned per item"
                },
                "subagent_type": {
                    "type": "string",
                    "description": "Subagent type for all spawned agents (default: coder)"
                },
                "max_concurrency": {
                    "type": "integer",
                    "description": "Maximum number of concurrent subagents (default: 3)"
                },
                "timeout_seconds": {
                    "type": "integer",
                    "description": "Timeout in seconds for the whole batch (default: 1800)"
                },
                "cancel_on_error": {
                    "type": "boolean",
                    "description": "Cancel remaining subagents when one fails (default: false)"
                }
            },
            "required": ["prompt_template", "items"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> ToolResult<Value> {
        let template = helpers::required_str(&args, "prompt_template")?;
        if !template.contains("{{item}}") {
            return Err(ToolError::execution_failed(
                "prompt_template must contain a {{item}} placeholder".to_string(),
            ));
        }

        let items: Vec<String> = args
            .get("items")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if items.is_empty() {
            return Err(ToolError::execution_failed(
                "items must be a non-empty array of strings".to_string(),
            ));
        }

        let orchestrator = require_orchestrator(&self.orchestrator)?;

        let specs: Vec<RunSpec> = items
            .iter()
            .map(|item| build_spec(item, template.replace("{{item}}", item), &args))
            .collect();

        let max_concurrency = args
            .get("max_concurrency")
            .and_then(|v| v.as_u64())
            .map(|v| v as usize)
            .unwrap_or(3);
        let timeout = args
            .get("timeout_seconds")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_SECS);
        let mut config = ParallelConfig::new()
            .with_max_concurrency(max_concurrency)
            .with_timeout(timeout);
        if args.get("cancel_on_error").and_then(|v| v.as_bool()) == Some(true) {
            config = config.cancel_on_error();
        }

        let result = orchestrator
            .run_parallel(specs, config, None)
            .await
            .map_err(|e| ToolError::execution_failed(format!("Agent swarm failed: {}", e)))?;

        let summary = result
            .aggregated_summary
            .clone()
            .unwrap_or_else(|| result.merged_output());
        let failures: Vec<Value> = result
            .failures
            .iter()
            .map(|(desc, err)| json!({ "description": desc, "error": err }))
            .collect();

        Ok(json!({
            "succeeded": result.results.len(),
            "failed": result.failures.len(),
            "total_elapsed_ms": result.total_elapsed_ms,
            "summary": summary,
            "failures": failures,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::subagent::{
        AgentTeam, BatchProgressHandle, ExecutionStatus, ParallelResult, SubagentError,
        SubagentResult, TeamResult,
    };

    struct MockOrchestrator;

    fn mock_run(agent_id: &str) -> SubagentResult {
        SubagentResult {
            agent_id: agent_id.to_string(),
            agent_type: "coder".to_string(),
            status: ExecutionStatus::Success,
            summary: format!("summary of {}", agent_id),
            full_output: String::new(),
            resumed: false,
            steps_taken: 1,
            elapsed_ms: 5,
            started_at: 0,
            completed_at: 5,
            monitoring_enabled: false,
        }
    }

    #[async_trait]
    impl SubagentOrchestrator for MockOrchestrator {
        async fn run_parallel(
            &self,
            specs: Vec<RunSpec>,
            _config: ParallelConfig,
            _progress: Option<BatchProgressHandle>,
        ) -> Result<ParallelResult, SubagentError> {
            Ok(ParallelResult {
                results: specs
                    .iter()
                    .enumerate()
                    .map(|(i, _)| mock_run(&format!("a{}", i)))
                    .collect(),
                failures: vec![],
                total_elapsed_ms: 10,
                actual_concurrency: 1,
                aggregated_summary: Some("aggregated".to_string()),
            })
        }

        async fn run_team(&self, _team: AgentTeam) -> Result<TeamResult, SubagentError> {
            Err(SubagentError::Cancelled)
        }
    }

    fn orchestrator() -> Arc<dyn SubagentOrchestrator> {
        Arc::new(MockOrchestrator)
    }

    #[test]
    fn agent_tool_metadata() {
        let tool = AgentTool::new();
        assert_eq!(tool.name(), "agent");
        assert!(tool.check_readiness().is_some());
        let params = tool.parameters();
        assert_eq!(params["required"], json!(["prompt"]));
        assert!(params["properties"].get("subagent_type").is_some());
        assert!(params["properties"].get("resume").is_some());
    }

    #[test]
    fn agent_swarm_tool_metadata() {
        let tool = AgentSwarmTool::new();
        assert_eq!(tool.name(), "agent_swarm");
        assert!(tool.check_readiness().is_some());
        let params = tool.parameters();
        assert_eq!(params["required"], json!(["prompt_template", "items"]));
    }

    #[tokio::test]
    async fn agent_tool_returns_summary() {
        let tool = AgentTool::with_orchestrator(orchestrator());
        assert!(tool.check_readiness().is_none());

        let result = tool
            .execute(json!({"prompt": "fix the bug"}), ToolContext::new())
            .await
            .unwrap();
        assert_eq!(result["status"], "completed");
        assert_eq!(result["summary"], "summary of a0");
    }

    #[tokio::test]
    async fn agent_tool_errors_without_orchestrator() {
        let tool = AgentTool::new();
        let result = tool
            .execute(json!({"prompt": "x"}), ToolContext::new())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn agent_swarm_requires_placeholder_and_items() {
        let tool = AgentSwarmTool::with_orchestrator(orchestrator());

        let missing_placeholder = tool
            .execute(
                json!({"prompt_template": "no placeholder", "items": ["a"]}),
                ToolContext::new(),
            )
            .await;
        assert!(missing_placeholder.is_err());

        let empty_items = tool
            .execute(
                json!({"prompt_template": "do {{item}}", "items": []}),
                ToolContext::new(),
            )
            .await;
        assert!(empty_items.is_err());
    }

    #[tokio::test]
    async fn agent_swarm_expands_items_and_aggregates() {
        let tool = AgentSwarmTool::with_orchestrator(orchestrator());

        let result = tool
            .execute(
                json!({
                    "prompt_template": "review {{item}}",
                    "items": ["src/a.rs", "src/b.rs"],
                }),
                ToolContext::new(),
            )
            .await
            .unwrap();
        assert_eq!(result["succeeded"], 2);
        assert_eq!(result["failed"], 0);
        assert_eq!(result["summary"], "aggregated");
    }
}
