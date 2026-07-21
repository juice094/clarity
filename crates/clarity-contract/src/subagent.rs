//! Subagent data types shared across the Clarity ecosystem.
//!
//! These types describe subagent configurations, execution results, and
//! coordination primitives without pulling in the full core runtime.

use crate::{AgentError, ToolError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::path::{Path, PathBuf};

// Re-export capability types already defined in capability.rs
pub use crate::capability::{CapabilityToken, TokenError};

// ============================================================================
// runner.rs types
// ============================================================================

/// Errors that can occur during subagent execution.
#[derive(Debug, Clone)]
pub enum SubagentError {
    /// Failed to build the subagent.
    BuildFailed(String),
    /// Subagent execution failed.
    ExecutionFailed {
        /// Detailed error message.
        message: String,
        /// Short error category.
        brief: String,
    },
    /// Maximum allowed steps were reached.
    MaxStepsReached {
        /// Number of steps taken.
        steps: usize,
        /// Phase where the limit was reached.
        phase: String,
    },
    /// LLM provider error.
    LlmError(String),
    /// Execution was cancelled.
    Cancelled,
    /// Memory store error.
    StoreError(String),
    /// Invalid response from the subagent.
    InvalidResponse(String),
    /// Unknown agent type requested.
    UnknownAgentType(String),
    /// Failed to resume a persisted subagent.
    ResumeFailed(String),
}

impl fmt::Display for SubagentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SubagentError::BuildFailed(msg) => write!(f, "Build failed: {}", msg),
            SubagentError::ExecutionFailed { message, brief } => {
                write!(f, "Execution failed ({}): {}", brief, message)
            }
            SubagentError::MaxStepsReached { steps, phase } => {
                write!(f, "Max steps ({}) reached during {}", steps, phase)
            }
            SubagentError::LlmError(msg) => write!(f, "LLM error: {}", msg),
            SubagentError::Cancelled => write!(f, "Subagent execution was cancelled"),
            SubagentError::StoreError(msg) => write!(f, "Store error: {}", msg),
            SubagentError::InvalidResponse(msg) => write!(f, "Invalid response: {}", msg),
            SubagentError::UnknownAgentType(name) => write!(f, "Unknown agent type: {}", name),
            SubagentError::ResumeFailed(msg) => write!(f, "Resume failed: {}", msg),
        }
    }
}

impl std::error::Error for SubagentError {}

impl From<AgentError> for SubagentError {
    fn from(err: AgentError) -> Self {
        match err {
            AgentError::MaxIterationsExceeded(n) => SubagentError::MaxStepsReached {
                steps: n,
                phase: "execution".into(),
            },
            AgentError::Llm(msg) => SubagentError::LlmError(msg),
            _ => SubagentError::ExecutionFailed {
                message: err.to_string(),
                brief: "agent error".into(),
            },
        }
    }
}

impl From<ToolError> for SubagentError {
    fn from(err: ToolError) -> Self {
        SubagentError::ExecutionFailed {
            message: err.to_string(),
            brief: "tool error".into(),
        }
    }
}

impl From<anyhow::Error> for SubagentError {
    fn from(err: anyhow::Error) -> Self {
        SubagentError::BuildFailed(err.to_string())
    }
}

/// Real-time progress events emitted by a single subagent run.
#[derive(Debug, Clone)]
pub enum SubagentProgressEvent {
    /// A new execution stage was reached.
    Stage {
        /// Agent identifier.
        agent_id: String,
        /// Name of the stage.
        name: String,
    },
    /// New output text was appended.
    Output {
        /// Agent identifier.
        agent_id: String,
        /// Output text.
        text: String,
    },
    /// The agent status changed.
    StatusChange {
        /// Agent identifier.
        agent_id: String,
        /// Agent type.
        agent_type: String,
        /// New status.
        status: SubagentStatus,
    },
    /// Budget progress update (steps taken / max steps).
    Progress {
        /// Agent identifier.
        agent_id: String,
        /// Steps taken so far.
        steps: usize,
        /// Maximum allowed steps.
        max_steps: usize,
    },
}

/// 子代理执行结果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentResult {
    /// 代理 ID
    pub agent_id: String,
    /// 代理类型
    pub agent_type: String,
    /// 执行状态
    pub status: ExecutionStatus,
    /// 输出摘要
    pub summary: String,
    /// 完整输出
    pub full_output: String,
    /// 是否从恢复执行
    pub resumed: bool,
    /// 执行步数
    pub steps_taken: usize,
    /// 耗时（毫秒）
    pub elapsed_ms: u64,
    /// 开始时间
    pub started_at: u64,
    /// 结束时间
    pub completed_at: u64,
    /// 是否通过 monitoring 模式执行
    #[serde(default)]
    pub monitoring_enabled: bool,
}

/// Final status of a subagent execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    /// Execution completed successfully.
    Success,
    /// Execution failed.
    Failed,
    /// Execution was cancelled.
    Cancelled,
    /// Maximum allowed steps were reached.
    MaxStepsReached,
}

impl fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExecutionStatus::Success => write!(f, "completed"),
            ExecutionStatus::Failed => write!(f, "failed"),
            ExecutionStatus::Cancelled => write!(f, "cancelled"),
            ExecutionStatus::MaxStepsReached => write!(f, "max_steps_reached"),
        }
    }
}

/// Completion notification for a background (spawned) subagent run.
///
/// Emitted by `SubagentHandle` (in `clarity-subagents`) when a non-blocking
/// subagent run finishes. [`SubagentCompletion::to_system_message`] renders
/// the notification as system-message text that the parent agent can inject
/// into its conversation context.
#[derive(Debug, Clone)]
pub struct SubagentCompletion {
    /// Human-readable task description from the originating [`RunSpec`].
    pub description: String,
    /// Final outcome of the run.
    pub result: Result<SubagentResult, SubagentError>,
}

impl SubagentCompletion {
    /// Render the completion as system-message text suitable for injection
    /// into the parent agent's conversation context.
    pub fn to_system_message(&self) -> String {
        match &self.result {
            Ok(r) => format!(
                "[subagent completed] {} (agent {}, type {}): {}",
                self.description, r.agent_id, r.agent_type, r.summary
            ),
            Err(e) => format!("[subagent failed] {}: {}", self.description, e),
        }
    }
}

/// Specification for a single subagent run.
#[derive(Debug, Clone)]
pub struct RunSpec {
    /// Human-readable task description.
    pub description: String,
    /// Prompt or instructions for the subagent.
    pub prompt: String,
    /// Requested agent type (e.g., "coder", "explore").
    pub requested_type: String,
    /// Optional model override.
    pub model_override: Option<String>,
    /// Optional agent instance identifier to resume.
    pub resume: Option<String>,
    /// Optional maximum iteration override.
    pub max_iterations: Option<usize>,
    /// Whether to collect Git context.
    pub git_context: bool,
    /// Optional capability token for permission isolation.
    pub capability_token: Option<CapabilityToken>,
    /// Goal tags for routing decisions.
    pub goal_tags: Vec<String>,
    /// Optional JSON Schema for structured output enforcement.
    ///
    /// When set, the subagent's LLM provider will be configured with
    /// `response_format: { type: "json_schema", json_schema: { ... } }`.
    /// The subagent's final response is validated against this schema
    /// (best-effort; unsupported providers fall back to unconstrained text).
    pub output_schema: Option<serde_json::Value>,
    /// Capability tags requested for this run.
    pub capabilities: Vec<String>,
    /// Force read-only mode regardless of the agent type definition.
    pub read_only: bool,
}

impl RunSpec {
    /// 创建新的运行规格
    pub fn new(description: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            description: description.into(),
            prompt: prompt.into(),
            requested_type: "coder".into(),
            model_override: None,
            resume: None,
            max_iterations: None,
            git_context: true,
            capability_token: None,
            goal_tags: Vec::new(),
            output_schema: None,
            capabilities: Vec::new(),
            read_only: false,
        }
    }

    /// 设置代理类型
    pub fn with_type(mut self, agent_type: impl Into<String>) -> Self {
        self.requested_type = agent_type.into();
        self
    }

    /// 设置模型覆盖
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model_override = Some(model.into());
        self
    }

    /// 设置恢复实例
    pub fn with_resume(mut self, agent_id: impl Into<String>) -> Self {
        self.resume = Some(agent_id.into());
        self
    }

    /// 设置最大迭代次数
    pub fn with_max_iterations(mut self, max: usize) -> Self {
        self.max_iterations = Some(max);
        self
    }

    /// 设置结构化输出 JSON Schema。
    ///
    /// 当设置时，子代理的 LLM provider 将被配置为结构化输出模式，
    /// 最终响应将根据此 schema 进行验证（尽力而为；不支持的 provider 退回非结构化文本）。
    pub fn with_output_schema(mut self, schema: serde_json::Value) -> Self {
        self.output_schema = Some(schema);
        self
    }

    /// 禁用 Git 上下文
    pub fn without_git_context(mut self) -> Self {
        self.git_context = false;
        self
    }

    /// 设置能力令牌
    pub fn with_capability_token(mut self, token: CapabilityToken) -> Self {
        self.capability_token = Some(token);
        self
    }

    /// 设置目标标签（Jumpy Predictor 路由决策用）
    pub fn with_goal_tags(mut self, tags: Vec<String>) -> Self {
        self.goal_tags = tags;
        self
    }

    /// 设置能力标签。
    pub fn with_capabilities(mut self, capabilities: Vec<String>) -> Self {
        self.capabilities = capabilities;
        self
    }

    /// 强制以只读模式运行子代理。
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
        self
    }
}

// ============================================================================
// parallel.rs types
// ============================================================================

/// Configuration for parallel subagent execution.
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// Maximum number of concurrent subagents.
    pub max_concurrency: usize,
    /// Optional timeout in seconds.
    pub timeout_secs: Option<u64>,
    /// Whether to cancel remaining tasks when one fails.
    pub cancel_on_error: bool,
    /// Whether to aggregate individual results into a summary.
    pub enable_aggregation: bool,
}

impl Default for ParallelConfig {
    fn default() -> Self {
        Self {
            max_concurrency: 3,
            timeout_secs: Some(300),
            cancel_on_error: false,
            enable_aggregation: true,
        }
    }
}

impl ParallelConfig {
    /// 创建默认配置
    pub fn new() -> Self {
        Self::default()
    }

    /// 设置最大并发数
    pub fn with_max_concurrency(mut self, max: usize) -> Self {
        self.max_concurrency = max.max(1);
        self
    }

    /// 设置超时时间
    pub fn with_timeout(mut self, secs: u64) -> Self {
        self.timeout_secs = Some(secs);
        self
    }

    /// 设置错误时取消
    pub fn cancel_on_error(mut self) -> Self {
        self.cancel_on_error = true;
        self
    }

    /// 禁用结果聚合
    pub fn without_aggregation(mut self) -> Self {
        self.enable_aggregation = false;
        self
    }
}

/// Result of a parallel subagent execution.
#[derive(Debug, Clone)]
pub struct ParallelResult {
    /// Successful subagent results.
    pub results: Vec<SubagentResult>,
    /// Failed subagents, as (description, error message) pairs.
    pub failures: Vec<(String, String)>,
    /// Total elapsed time in milliseconds.
    pub total_elapsed_ms: u64,
    /// Actual concurrency level reached.
    pub actual_concurrency: usize,
    /// Aggregated summary, if aggregation was enabled.
    pub aggregated_summary: Option<String>,
}

impl ParallelResult {
    /// 检查是否全部成功
    pub fn all_succeeded(&self) -> bool {
        self.failures.is_empty()
    }

    /// 获取成功率
    pub fn success_rate(&self) -> f64 {
        let total = self.results.len() + self.failures.len();
        if total == 0 {
            0.0
        } else {
            self.results.len() as f64 / total as f64
        }
    }

    /// 合并所有输出
    pub fn merged_output(&self) -> String {
        let mut outputs = Vec::new();

        for result in &self.results {
            outputs.push(format!(
                "## {} ({}): {}\n{}\n",
                result.agent_id, result.agent_type, result.status, result.summary
            ));
        }

        for (id, err) in &self.failures {
            outputs.push(format!("## {}: FAILED\n{}\n", id, err));
        }

        outputs.join("\n---\n")
    }
}

/// Progress state of an in-flight parallel batch.
#[derive(Debug, Clone)]
pub struct BatchProgress {
    /// Unique batch identifier.
    pub batch_id: String,
    /// Total number of subagents in this batch.
    pub total: usize,
    /// Number of subagents that have completed.
    pub completed: usize,
    /// Number of subagents that have failed.
    pub failed: usize,
    /// Agent IDs currently executing.
    pub running: Vec<String>,
    /// Current status of the batch.
    pub status: BatchStatus,
    /// Unix timestamp (seconds) when execution started.
    pub started_at: u64,
    /// Elapsed milliseconds so far.
    pub elapsed_ms: u64,
    /// Subagent results collected so far.
    pub results: Vec<SubagentResult>,
    /// Failures collected so far.
    pub failures: Vec<(String, String)>,
}

impl BatchProgress {
    /// Create a new batch progress tracker.
    pub fn new(batch_id: String, specs: &[RunSpec]) -> Self {
        let total = specs.len();
        let started_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        Self {
            batch_id,
            total,
            completed: 0,
            failed: 0,
            running: specs.iter().map(|s| s.description.clone()).collect(),
            status: BatchStatus::Running,
            started_at,
            elapsed_ms: 0,
            results: Vec::new(),
            failures: Vec::new(),
        }
    }
}

/// Status of a parallel batch.
#[derive(Debug, Clone, PartialEq)]
pub enum BatchStatus {
    /// Batch is currently running.
    Running,
    /// Batch completed successfully.
    Completed,
    /// Batch was cancelled.
    Cancelled,
    /// Batch failed with an error message.
    Failed(String),
}

// ============================================================================
// team.rs types
// ============================================================================

/// A message sent between team members via the shared Mailbox.
#[derive(Debug, Clone)]
pub struct MailboxMessage {
    /// Agent ID of the sender.
    pub from: String,
    /// Message payload.
    pub payload: MessagePayload,
    /// Unix timestamp (seconds).
    pub timestamp: u64,
}

/// Payload variants for MailboxMessage.
#[derive(Debug, Clone)]
pub enum MessagePayload {
    /// Free-form text broadcast.
    Text(String),
    /// Status update (started, completed, failed, etc.).
    StatusUpdate(SubagentStatus),
    /// Intermediate result that other members may consume.
    IntermediateResult(String),
}

/// Shared message bus for an AgentTeam.
#[derive(Clone)]
pub struct Mailbox {
    tx: tokio::sync::broadcast::Sender<MailboxMessage>,
}

impl Default for Mailbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Mailbox {
    /// Create a new mailbox with capacity for 256 in-flight messages.
    pub fn new() -> Self {
        let (tx, _rx) = tokio::sync::broadcast::channel(256);
        Self { tx }
    }

    /// Subscribe to messages.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<MailboxMessage> {
        self.tx.subscribe()
    }

    /// Broadcast a message to all active subscribers.
    pub fn send(&self, msg: MailboxMessage) -> Result<(), MailboxError> {
        let _ = self.tx.send(msg);
        Ok(())
    }

    /// Returns the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

/// Error type for mailbox operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailboxError {
    /// The mailbox has been closed and no more messages can be sent.
    Closed,
    /// The message channel is full.
    Full,
}

impl fmt::Display for MailboxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MailboxError::Closed => write!(f, "Mailbox closed"),
            MailboxError::Full => write!(f, "Mailbox full"),
        }
    }
}

impl std::error::Error for MailboxError {}

/// A team of sub-agents working toward a shared goal.
#[derive(Clone)]
pub struct AgentTeam {
    /// Human-readable team name.
    pub name: String,
    /// High-level objective.
    pub goal: String,
    /// Member specifications.
    pub members: Vec<RunSpec>,
    /// Shared mailbox for loose coordination.
    pub mailbox: Mailbox,
    /// Parallel execution configuration.
    pub config: ParallelConfig,
}

impl AgentTeam {
    /// Create a new team with the given name and goal.
    pub fn new(name: impl Into<String>, goal: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            goal: goal.into(),
            members: Vec::new(),
            mailbox: Mailbox::new(),
            config: ParallelConfig::default(),
        }
    }

    /// Add a member to the team (builder pattern).
    pub fn with_member(mut self, spec: RunSpec) -> Self {
        self.members.push(spec);
        self
    }

    /// Batch-add members (builder pattern).
    pub fn with_members(mut self, specs: Vec<RunSpec>) -> Self {
        self.members.extend(specs);
        self
    }

    /// Set parallel execution config (builder pattern).
    pub fn with_config(mut self, config: ParallelConfig) -> Self {
        self.config = config;
        self
    }

    /// Replace the default mailbox with a custom one.
    pub fn with_mailbox(mut self, mailbox: Mailbox) -> Self {
        self.mailbox = mailbox;
        self
    }

    /// Returns true if the team has no members.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Number of members.
    pub fn len(&self) -> usize {
        self.members.len()
    }
}

/// Unified result after executing an AgentTeam.
#[derive(Debug, Clone)]
pub struct TeamResult {
    /// Underlying parallel execution results.
    pub parallel: ParallelResult,
    /// Messages collected from the team's mailbox during execution.
    pub messages: Vec<MailboxMessage>,
}

impl TeamResult {
    /// Check if every member succeeded.
    pub fn all_succeeded(&self) -> bool {
        self.parallel.all_succeeded()
    }

    /// Aggregate success rate.
    pub fn success_rate(&self) -> f64 {
        self.parallel.success_rate()
    }

    /// Filter messages by payload type.
    pub fn filter_text(&self) -> Vec<&MailboxMessage> {
        self.messages
            .iter()
            .filter(|m| matches!(m.payload, MessagePayload::Text(_)))
            .collect()
    }

    /// Filter intermediate results.
    pub fn filter_intermediate(&self) -> Vec<&MailboxMessage> {
        self.messages
            .iter()
            .filter(|m| matches!(m.payload, MessagePayload::IntermediateResult(_)))
            .collect()
    }
}

// ============================================================================
// registry.rs types
// ============================================================================

/// Definition of a subagent type.
#[derive(Debug, Clone)]
pub struct AgentTypeDefinition {
    /// Agent type name (e.g. "coder", "explore").
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// System prompt for this agent type.
    pub system_prompt: String,
    /// Allowed tools (None = all tools).
    pub allowed_tools: Option<Vec<String>>,
    /// Default maximum iterations.
    pub max_iterations: usize,
    /// Default model alias, e.g. `kimi-coding/kimi-for-coding`.
    pub model: Option<String>,
    /// Capability tags used for routing and UI labeling.
    pub capabilities: Vec<String>,
    /// Per-agent timeout in seconds.
    pub timeout_seconds: Option<u64>,
    /// Whether this agent is prohibited from mutating tools.
    pub read_only: bool,
    /// Maximum tokens to request from the LLM.
    pub max_tokens: Option<usize>,
}

/// Registry for subagent types (LaborMarket).
#[derive(Clone)]
pub struct LaborMarket {
    types: HashMap<String, AgentTypeDefinition>,
}

impl Default for LaborMarket {
    fn default() -> Self {
        Self::new()
    }
}

impl LaborMarket {
    /// Create a new labor market with built-in types.
    pub fn new() -> Self {
        let mut market = Self {
            types: HashMap::new(),
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
            model: None,
            capabilities: vec!["code".to_string(), "write".to_string(), "debug".to_string()],
            timeout_seconds: None,
            read_only: false,
            max_tokens: None,
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
            model: None,
            capabilities: vec!["search".to_string(), "read".to_string()],
            timeout_seconds: Some(120),
            read_only: true,
            max_tokens: Some(8192),
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
            model: None,
            capabilities: vec!["plan".to_string(), "design".to_string()],
            timeout_seconds: Some(180),
            read_only: false,
            max_tokens: Some(16384),
        });
        self.register(AgentTypeDefinition {
            name: "review".to_string(),
            description: "Code reviewer for style, safety, and architectural adherence".to_string(),
            system_prompt: REVIEW_SYSTEM_PROMPT.to_string(),
            allowed_tools: Some(vec![
                "file_read".to_string(),
                "glob".to_string(),
                "grep".to_string(),
            ]),
            max_iterations: 5,
            model: None,
            capabilities: vec!["review".to_string(), "analyze".to_string()],
            timeout_seconds: Some(180),
            read_only: true,
            max_tokens: Some(16384),
        });
        self.register(AgentTypeDefinition {
            name: "simplify".to_string(),
            description: "Code simplifier for clarity, consistency, and maintainability"
                .to_string(),
            system_prompt: SIMPLIFY_SYSTEM_PROMPT.to_string(),
            allowed_tools: Some(vec![
                "file_read".to_string(),
                "file_write".to_string(),
                "file_edit".to_string(),
            ]),
            max_iterations: 10,
            model: None,
            capabilities: vec!["refactor".to_string(), "simplify".to_string()],
            timeout_seconds: Some(120),
            read_only: false,
            max_tokens: Some(8192),
        });
        self.register(AgentTypeDefinition {
            name: "test".to_string(),
            description: "Test coverage analyzer and test case generator".to_string(),
            system_prompt: TEST_SYSTEM_PROMPT.to_string(),
            allowed_tools: Some(vec![
                "file_read".to_string(),
                "glob".to_string(),
                "grep".to_string(),
                "bash".to_string(),
            ]),
            max_iterations: 10,
            model: None,
            capabilities: vec!["test".to_string(), "analyze".to_string()],
            timeout_seconds: Some(180),
            read_only: true,
            max_tokens: Some(8192),
        });
    }

    /// Register a custom agent type.
    pub fn register(&mut self, type_def: AgentTypeDefinition) {
        self.types.insert(type_def.name.clone(), type_def);
    }

    /// Get a type definition by name.
    pub fn get(&self, name: &str) -> Option<&AgentTypeDefinition> {
        self.types.get(name)
    }

    /// Require a type definition, returning an error if unknown.
    pub fn require(&self, name: &str) -> Result<&AgentTypeDefinition, crate::AgentError> {
        self.get(name)
            .ok_or_else(|| crate::AgentError::Registry(format!("Unknown agent type: {}", name)))
    }

    /// List all registered types.
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

const EXPLORE_SYSTEM_PROMPT: &str = r#"You are a fast read-only search agent. Your job is to locate code, files, and symbols across a codebase.

Rules:
- Only use read/search tools (Glob, Grep, Read). Never write, edit, or execute code.
- When searching, prefer Grep for exact symbols and Glob for file patterns.
- Read files conservatively — prefer excerpts over full files.
- Return concise results: file paths and relevant line numbers.
- If a search yields too many results, narrow the pattern.

Output Format:
```
[File]: path/to/file.ext (lines N-M)
[Match]: relevant excerpt
```
"#;

const PLAN_SYSTEM_PROMPT: &str = r#"You are an architecture planning agent. Design implementation strategies before code is written.

Rules:
- Interface definition precedes implementation.
- Identify critical files and their responsibilities.
- Flag architectural trade-offs with concrete pros/cons.
- Estimate complexity (time/space) for key algorithms.
- Mark dependencies between steps (blocks/blockedBy).
- Reject speculative abstractions; design for current requirements.

Output Format:
```
[Step]: N
[File]: path/to/file.ext
[Action]: what to do
[DependsOn]: [step numbers]
[Complexity]: O(...) or note
[Risk]: low | medium | high
```
"#;

const REVIEW_SYSTEM_PROMPT: &str = r#"You are a code reviewer. Review changes for correctness, style, security, and maintainability.

Rules:
- Check for silent failures, unhandled errors, and implicit assumptions.
- Flag magic numbers, global mutable state, and blocking operations in async contexts.
- Verify that public APIs have clear preconditions and postconditions.
- Look for security issues: injection, XSS, path traversal, hardcoded secrets.
- Respect project-specific style guides.

Output Format:
```
[Severity]: HIGH | MEDIUM | LOW | INFO
[Category]: correctness | style | security | performance | architecture
[Location]: file.ext:line
[Issue]: concise description
[Suggestion]: how to fix
```
"#;

const SIMPLIFY_SYSTEM_PROMPT: &str = r#"You are a code simplifier. Refactor code for clarity, consistency, and maintainability while preserving all functionality.

Rules:
- Remove unnecessary abstractions; prefer concrete types over layered generics.
- Inline single-use helpers unless they clarify intent.
- Name variables and functions to answer "what", not "how".
- Eliminate redundant comments; keep only non-obvious "why" comments.
- Preserve error handling and edge-case behavior.
- Do not change public interfaces without deprecation.

Output Format:
```
[File]: path/to/file.ext
[Change]: brief description
[Before]: excerpt (optional)
[After]: excerpt (optional)
```
"#;

const TEST_SYSTEM_PROMPT: &str = r#"You are a test coverage analyzer. Identify untested logic, edge cases, and suggest test cases.

Rules:
- Focus on public API contracts and error paths.
- Flag tests that mock too much (integration tests preferred for DB/network).
- Check for flakiness: time-dependent, race-condition, or non-deterministic tests.
- Suggest property-based tests for complex invariants.
- Verify that every fallible path has a corresponding test.

Output Format:
```
[File]: path/to/test.ext or path/to/source.ext
[Gap]: what's untested
[Severity]: critical | important | nice-to-have
[Suggestion]: test case description
```
"#;

// ============================================================================
// store.rs types
// ============================================================================

/// Status of a subagent instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubagentStatus {
    /// Subagent is idle and waiting.
    Idle,
    /// Subagent is currently running.
    Running,
    /// Subagent completed successfully.
    Completed,
    /// Subagent failed.
    Failed,
}

/// Persistent state of a subagent instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentState {
    /// Agent identifier.
    pub agent_id: String,
    /// Agent type.
    pub agent_type: String,
    /// Current status.
    pub status: SubagentStatus,
    /// Conversation history.
    pub history: Vec<crate::Message>,
    /// Creation timestamp (Unix seconds).
    pub created_at: u64,
    /// Last update timestamp (Unix seconds).
    pub updated_at: u64,
    /// Maximum iterations allowed for this subagent (used for progress estimation).
    pub max_iterations: Option<usize>,
    /// Actual steps taken in the last run (for progress reporting).
    pub steps_taken: usize,
}

impl SubagentState {
    /// Create a new idle subagent state.
    pub fn new(agent_id: String, agent_type: String) -> Self {
        let now = now_timestamp();
        Self {
            agent_id,
            agent_type,
            status: SubagentStatus::Idle,
            history: Vec::new(),
            created_at: now,
            updated_at: now,
            max_iterations: None,
            steps_taken: 0,
        }
    }
}

fn now_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

// ============================================================================
// UI progress types (previously in clarity-egui::ui::types)
// ============================================================================

/// Progress summary for a parallel batch of subagents.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct SubAgentProgress {
    /// Unique batch identifier.
    pub batch_id: String,
    /// Total number of subagents in the batch.
    pub total: usize,
    /// Number of subagents that have completed.
    pub completed: usize,
    /// Number of subagents that have failed.
    pub failed: usize,
    /// Human-readable status string.
    pub status: String,
    /// When the progress was last refreshed.
    pub last_poll: std::time::Instant,
}

/// Live progress for a single subagent invoked via /coder or /explore.
#[derive(Clone, Debug)]
pub struct SingleSubagentProgress {
    /// Agent type (e.g. "coder", "explore").
    pub agent_type: String,
    /// Human-readable status string.
    pub status: String,
    /// Execution stages already reached.
    pub stages: Vec<String>,
    /// Recent output lines emitted by the agent.
    pub output_lines: Vec<String>,
    /// When the subagent started.
    pub started_at: std::time::Instant,
    /// When the subagent completed, if it has.
    pub completed_at: Option<std::time::Instant>,
    /// Budget progress: steps taken.
    pub steps: usize,
    /// Budget progress: maximum allowed steps.
    pub max_steps: usize,
}

// ============================================================================
// Git context (from runner.rs)
// ============================================================================

/// Git context information.
#[derive(Debug, Clone, Default)]
pub struct GitContext {
    /// Current branch
    pub branch: Option<String>,
    /// Recent commits
    pub recent_commits: Vec<String>,
    /// Status summary
    pub status_summary: String,
    /// Repository root directory
    pub repo_root: Option<PathBuf>,
}

impl GitContext {
    /// Collect Git context from a working directory.
    pub async fn collect(working_dir: impl AsRef<Path>) -> Option<GitContext> {
        let working_dir = working_dir.as_ref();
        let git_dir = working_dir.join(".git");
        if !git_dir.exists() {
            return None;
        }

        let branch = get_git_branch(working_dir).await.ok().flatten();
        let recent_commits = get_recent_commits(working_dir, 3).await.unwrap_or_default();
        let changed_count = get_changed_files_count(working_dir).await.unwrap_or(0);
        let status_summary = if changed_count > 0 {
            format!("{} uncommitted files", changed_count)
        } else {
            "clean".to_string()
        };
        let repo_root = get_git_repo_root(working_dir)
            .await
            .ok()
            .flatten()
            .or_else(|| Some(working_dir.to_path_buf()));

        Some(GitContext {
            branch,
            recent_commits,
            status_summary,
            repo_root,
        })
    }

    /// Format as a prompt block.
    pub fn to_prompt_string(&self) -> String {
        let mut s = String::from("# Git Context\n\n");
        if let Some(branch) = &self.branch {
            s.push_str(&format!("Current branch: {}\n", branch));
        }
        if !self.recent_commits.is_empty() {
            s.push_str("Recent commits:\n");
            for commit in &self.recent_commits {
                s.push_str(&format!("- {}\n", commit));
            }
        }
        if !self.status_summary.is_empty() {
            s.push_str(&format!("Status: {}\n", self.status_summary));
        }
        s
    }
}

async fn get_git_branch(working_dir: impl AsRef<Path>) -> Result<Option<String>, std::io::Error> {
    let output = tokio::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(working_dir)
        .output()
        .await?;

    if output.status.success() {
        let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(Some(branch))
    } else {
        Ok(None)
    }
}

async fn get_git_repo_root(
    working_dir: impl AsRef<Path>,
) -> Result<Option<PathBuf>, std::io::Error> {
    let output = tokio::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(working_dir)
        .output()
        .await?;

    if output.status.success() {
        let root = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(Some(PathBuf::from(root)))
    } else {
        Ok(None)
    }
}

async fn get_recent_commits(
    working_dir: impl AsRef<Path>,
    count: usize,
) -> Result<Vec<String>, std::io::Error> {
    let output = tokio::process::Command::new("git")
        .args([
            "log",
            &format!("-n {}", count),
            "--oneline",
            "--no-decorate",
        ])
        .current_dir(working_dir)
        .output()
        .await?;

    if output.status.success() {
        let commits: Vec<String> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();
        Ok(commits)
    } else {
        Ok(Vec::new())
    }
}

async fn get_changed_files_count(working_dir: impl AsRef<Path>) -> Result<usize, std::io::Error> {
    let output = tokio::process::Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(working_dir)
        .output()
        .await?;

    if output.status.success() {
        let count = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter(|line| !line.is_empty())
            .count();
        Ok(count)
    } else {
        Ok(0)
    }
}

/// Collect Git context (returns formatted string).
pub async fn collect_git_context(working_dir: impl AsRef<Path>) -> Option<String> {
    GitContext::collect(working_dir)
        .await
        .map(|ctx| ctx.to_prompt_string())
}

// ============================================================================
// SubagentOrchestrator trait — breaks agent↔subagents coupling
// ============================================================================

use async_trait::async_trait;
use std::sync::Arc;

/// Handle for progress tracking in parallel subagent execution.
pub type BatchProgressHandle = Arc<parking_lot::Mutex<BatchProgress>>;

/// Trait for subagent orchestration (parallel execution, team coordination).
///
/// Implemented by `SubagentManager` in `clarity-core`. `Agent` can hold
/// an `Option<Arc<dyn SubagentOrchestrator>>` and delegate `run_parallel`
/// / `run_team` calls to it, avoiding a direct dependency on `SubagentManager`.
#[async_trait]
pub trait SubagentOrchestrator: Send + Sync {
    /// Execute multiple subagent specs in parallel.
    async fn run_parallel(
        &self,
        specs: Vec<RunSpec>,
        config: ParallelConfig,
        progress: Option<BatchProgressHandle>,
    ) -> Result<ParallelResult, SubagentError>;

    /// Execute an agent team collaboratively.
    async fn run_team(&self, team: AgentTeam) -> Result<TeamResult, SubagentError>;
}

// ============================================================================
// AgentExecutor trait (migrated from clarity-core to break subagents↔agent cycle)
// ============================================================================

/// Minimal trait for anything that can execute an agent turn.
///
/// Extracted from `clarity-core::agent::executor` so that `clarity-subagents`
/// can depend on the trait without pulling in the concrete `Agent` type.
#[async_trait]
pub trait AgentExecutor: Send + Sync {
    /// Run a single turn with the given user query.
    async fn run_turn(&self, query: &str) -> Result<String, AgentError>;
    /// Return the number of messages exchanged in the last turn.
    fn last_turn_message_count(&self) -> usize;
}
