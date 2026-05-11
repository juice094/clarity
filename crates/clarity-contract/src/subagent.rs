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

/// 子代理执行错误
#[derive(Debug, Clone)]
pub enum SubagentError {
    /// 构建失败
    BuildFailed(String),
    /// 执行失败
    ExecutionFailed { message: String, brief: String },
    /// 达到最大步数
    MaxStepsReached { steps: usize, phase: String },
    /// LLM 错误
    LlmError(String),
    /// 已取消
    Cancelled,
    /// 存储错误
    StoreError(String),
    /// 无效的响应
    InvalidResponse(String),
    /// 未知代理类型
    UnknownAgentType(String),
    /// 恢复失败
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
    Stage { agent_id: String, name: String },
    /// New output text was appended.
    Output { agent_id: String, text: String },
    /// The agent status changed.
    StatusChange {
        agent_id: String,
        agent_type: String,
        status: SubagentStatus,
    },
    /// Budget progress update (steps taken / max steps).
    Progress {
        agent_id: String,
        steps: usize,
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

/// 执行状态
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ExecutionStatus {
    Success,
    Failed,
    Cancelled,
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

/// 子代理运行规格
#[derive(Debug, Clone)]
pub struct RunSpec {
    /// 任务描述
    pub description: String,
    /// 提示词
    pub prompt: String,
    /// 请求的代理类型
    pub requested_type: String,
    /// 模型覆盖（可选）
    pub model_override: Option<String>,
    /// 恢复之前的代理实例（可选）
    pub resume: Option<String>,
    /// 最大迭代次数（覆盖类型定义）
    pub max_iterations: Option<usize>,
    /// 是否启用 Git 上下文
    pub git_context: bool,
    /// 能力令牌（可选）
    pub capability_token: Option<CapabilityToken>,
    /// 目标标签（用于 Jumpy Predictor 路由决策）
    pub goal_tags: Vec<String>,
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
}

// ============================================================================
// parallel.rs types
// ============================================================================

/// 并行执行配置
#[derive(Debug, Clone)]
pub struct ParallelConfig {
    /// 最大并发数
    pub max_concurrency: usize,
    /// 超时时间（秒）
    pub timeout_secs: Option<u64>,
    /// 是否取消所有任务当其中一个失败
    pub cancel_on_error: bool,
    /// 是否启用结果聚合
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

/// 并行执行结果
#[derive(Debug, Clone)]
pub struct ParallelResult {
    /// 成功的执行结果
    pub results: Vec<SubagentResult>,
    /// 失败的执行
    pub failures: Vec<(String, String)>,
    /// 总耗时（毫秒）
    pub total_elapsed_ms: u64,
    /// 实际并发数
    pub actual_concurrency: usize,
    /// 聚合摘要（如果启用）
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
    Running,
    Completed,
    Cancelled,
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

    /// Register a custom agent type.
    pub fn register(&mut self, type_def: AgentTypeDefinition) {
        self.types.insert(type_def.name.clone(), type_def);
    }

    /// Get a type definition by name.
    pub fn get(&self, name: &str) -> Option<&AgentTypeDefinition> {
        self.types.get(name)
    }

    /// Require a type definition (panics if unknown).
    pub fn require(&self, name: &str) -> &AgentTypeDefinition {
        self.get(name)
            .unwrap_or_else(|| panic!("Unknown agent type: {}", name))
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

// ============================================================================
// store.rs types
// ============================================================================

/// Status of a subagent instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubagentStatus {
    Idle,
    Running,
    Completed,
    Failed,
}

/// State of a subagent instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentState {
    pub agent_id: String,
    pub agent_type: String,
    pub status: SubagentStatus,
    pub history: Vec<crate::Message>,
    pub created_at: u64,
    pub updated_at: u64,
    /// Maximum iterations allowed for this subagent (used for progress estimation).
    pub max_iterations: Option<usize>,
    /// Actual steps taken in the last run (for progress reporting).
    pub steps_taken: usize,
}

impl SubagentState {
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
        .unwrap()
        .as_secs()
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
