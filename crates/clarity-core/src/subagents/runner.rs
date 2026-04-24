//! Subagent Runner - 子代理执行器
//!
//! 负责执行子代理任务，管理生命周期，处理上下文传递和结果收集。
//!
//! 设计参考：
//! - std::process::Command 的构建器模式
//! - std::io 的读写抽象
//! - Rust 错误处理的最佳实践

use crate::agent::Agent;
use crate::approval::{ApprovalMode, ApprovalRuntime};
use crate::error::{AgentError, ToolError};
use crate::llm::api::{LlmProvider, Message};
use crate::llm::{build_provider_from_registry, ModelRegistry};
use crate::registry::ToolRegistry;
use crate::subagents::builder::SubagentBuilder;
use crate::subagents::registry::{AgentTypeDefinition, LaborMarket};
use crate::subagents::store::{SubagentStatus, SubagentStore};
use crate::subagents::token::CapabilityToken;
use clarity_wire::{Wire, WireMessage};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tracing::{debug, info, warn};

// =============================================================================
// 错误类型定义
// =============================================================================

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
        use crate::error::AgentError;
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

// =============================================================================
// 执行结果类型
// =============================================================================

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

// =============================================================================
// 运行规格定义
// =============================================================================

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
}

// =============================================================================
// 上下文管理
// =============================================================================

/// 子代理执行上下文
///
/// 负责管理子代理的持久化状态，包括对话历史、系统提示词等。
pub struct ExecutionContext {
    /// 上下文存储路径
    context_dir: PathBuf,
    /// 代理 ID
    agent_id: String,
    /// 系统提示词（恢复时加载）
    system_prompt: Option<String>,
    /// 消息历史
    history: Vec<Message>,
    /// 能力令牌
    capability_token: Option<CapabilityToken>,
}

impl ExecutionContext {
    /// 创建新的执行上下文
    pub fn new(context_dir: impl AsRef<Path>, agent_id: impl Into<String>) -> Self {
        Self {
            context_dir: context_dir.as_ref().to_path_buf(),
            agent_id: agent_id.into(),
            system_prompt: None,
            history: Vec::new(),
            capability_token: None,
        }
    }

    /// 上下文文件路径
    fn context_path(&self) -> PathBuf {
        self.context_dir.join(&self.agent_id).join("context.json")
    }

    /// 系统提示词路径
    fn system_prompt_path(&self) -> PathBuf {
        self.context_dir
            .join(&self.agent_id)
            .join("system_prompt.txt")
    }

    /// 提示词快照路径
    fn prompt_path(&self) -> PathBuf {
        self.context_dir.join(&self.agent_id).join("prompt.txt")
    }

    /// 输出路径
    fn output_path(&self) -> PathBuf {
        self.context_dir.join(&self.agent_id).join("output.txt")
    }

    /// 恢复上下文
    pub async fn restore(&mut self) -> Result<(), SubagentError> {
        let context_file = self.context_path();
        if context_file.exists() {
            let content = fs::read_to_string(&context_file).await.map_err(|e| {
                SubagentError::ResumeFailed(format!("Failed to read context: {}", e))
            })?;

            let messages: Vec<Message> = serde_json::from_str(&content).map_err(|e| {
                SubagentError::ResumeFailed(format!("Failed to parse context: {}", e))
            })?;

            info!(
                "Restored {} messages for agent {}",
                messages.len(),
                self.agent_id
            );
            self.history = messages;
        }

        // 加载系统提示词
        let system_prompt_file = self.system_prompt_path();
        if system_prompt_file.exists() {
            let prompt = fs::read_to_string(&system_prompt_file).await.map_err(|e| {
                SubagentError::ResumeFailed(format!("Failed to read system prompt: {}", e))
            })?;
            self.system_prompt = Some(prompt);
        }

        Ok(())
    }

    /// 保存上下文
    pub async fn save(&self) -> Result<(), SubagentError> {
        let agent_dir = self.context_dir.join(&self.agent_id);
        fs::create_dir_all(&agent_dir)
            .await
            .map_err(|e| SubagentError::StoreError(format!("Failed to create directory: {}", e)))?;

        let context_file = self.context_path();
        let content = serde_json::to_string_pretty(&self.history).map_err(|e| {
            SubagentError::StoreError(format!("Failed to serialize context: {}", e))
        })?;

        fs::write(&context_file, content)
            .await
            .map_err(|e| SubagentError::StoreError(format!("Failed to write context: {}", e)))?;

        Ok(())
    }

    /// 写入系统提示词
    pub async fn write_system_prompt(&self, prompt: impl AsRef<str>) -> Result<(), SubagentError> {
        let agent_dir = self.context_dir.join(&self.agent_id);
        fs::create_dir_all(&agent_dir)
            .await
            .map_err(|e| SubagentError::StoreError(format!("Failed to create directory: {}", e)))?;

        let path = self.system_prompt_path();
        fs::write(&path, prompt.as_ref()).await.map_err(|e| {
            SubagentError::StoreError(format!("Failed to write system prompt: {}", e))
        })?;

        Ok(())
    }

    /// 写入提示词快照
    pub async fn write_prompt_snapshot(
        &self,
        prompt: impl AsRef<str>,
    ) -> Result<(), SubagentError> {
        let path = self.prompt_path();
        fs::write(&path, prompt.as_ref()).await.map_err(|e| {
            SubagentError::StoreError(format!("Failed to write prompt snapshot: {}", e))
        })?;
        Ok(())
    }

    /// 追加消息到历史
    pub fn push_message(&mut self, message: Message) {
        self.history.push(message);
    }

    /// 获取历史
    pub fn history(&self) -> &[Message] {
        &self.history
    }

    /// 获取系统提示词
    pub fn system_prompt(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }

    /// 设置系统提示词
    pub fn set_system_prompt(&mut self, prompt: impl Into<String>) {
        self.system_prompt = Some(prompt.into());
    }

    /// 获取输出路径
    pub fn get_output_path(&self) -> PathBuf {
        self.output_path()
    }

    /// 设置能力令牌
    pub fn set_capability_token(&mut self, token: Option<CapabilityToken>) {
        self.capability_token = token;
    }

    /// 获取能力令牌
    pub fn capability_token(&self) -> Option<&CapabilityToken> {
        self.capability_token.as_ref()
    }
}

// =============================================================================
// 输出收集器
// =============================================================================

/// 子代理输出收集器
///
/// 负责收集和持久化子代理的输出。
pub struct OutputCollector {
    output_path: PathBuf,
    stages: Vec<String>,
    content: Vec<String>,
}

impl OutputCollector {
    /// 创建新的输出收集器
    pub fn new(output_path: impl AsRef<Path>) -> Self {
        Self {
            output_path: output_path.as_ref().to_path_buf(),
            stages: Vec::new(),
            content: Vec::new(),
        }
    }

    /// 记录阶段
    pub fn stage(&mut self, stage: impl Into<String>) {
        let stage = stage.into();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        self.stages.push(format!("[{}] {}", timestamp, stage));
        debug!("Subagent stage: {}", stage);
    }

    /// 追加内容
    pub fn append(&mut self, text: impl Into<String>) {
        self.content.push(text.into());
    }

    /// 写入 Wire 消息
    pub fn write_wire_message(&mut self, msg: &WireMessage) {
        let text = format!("[{:?}] {:?}", msg, std::time::Instant::now());
        self.content.push(text);
    }

    /// 保存摘要
    pub async fn save_summary(&self, summary: impl AsRef<str>) -> Result<(), SubagentError> {
        let output = format!(
            "# Stages\n{}\n\n# Output\n{}\n\n# Summary\n{}",
            self.stages.join("\n"),
            self.content.join("\n"),
            summary.as_ref()
        );

        fs::write(&self.output_path, output)
            .await
            .map_err(|e| SubagentError::StoreError(format!("Failed to write output: {}", e)))?;

        Ok(())
    }

    /// 获取完整输出
    pub fn full_output(&self) -> String {
        format!(
            "# Stages\n{}\n\n# Output\n{}",
            self.stages.join("\n"),
            self.content.join("\n")
        )
    }
}

// =============================================================================
// Git 上下文收集
// =============================================================================

/// Git 上下文信息
#[derive(Debug, Clone, Default)]
pub struct GitContext {
    /// 当前分支
    pub branch: Option<String>,
    /// 最近的提交
    pub recent_commits: Vec<String>,
    /// 状态摘要
    pub status_summary: String,
    /// 仓库根目录
    pub repo_root: Option<PathBuf>,
}

impl GitContext {
    /// 从工作目录收集 Git 上下文
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

    /// 格式化为提示词块
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

/// 收集 Git 上下文（返回格式化字符串的兼容接口）
pub async fn collect_git_context(working_dir: impl AsRef<Path>) -> Option<String> {
    GitContext::collect(working_dir)
        .await
        .map(|ctx| ctx.to_prompt_string())
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

// =============================================================================
// 子代理 Runner
// =============================================================================

/// 子代理执行器
///
/// 负责执行子代理任务，类似于 std::process::Command 的构建器模式。
pub struct SubagentRunner {
    /// 劳动力市场（类型注册表）
    labor_market: LaborMarket,
    /// 工具注册表
    tool_registry: ToolRegistry,
    /// 工作目录
    working_dir: PathBuf,
    /// 上下文目录
    context_dir: PathBuf,
    /// LLM 提供者（默认）
    llm: Option<Arc<dyn LlmProvider>>,
    /// 模型注册表（用于 model_override 动态选择）
    registry: Option<ModelRegistry>,
    /// 审批运行时
    approval_runtime: Option<Arc<dyn ApprovalRuntime>>,
    /// 审批模式
    approval_mode: ApprovalMode,
}

impl Clone for SubagentRunner {
    fn clone(&self) -> Self {
        Self {
            labor_market: self.labor_market.clone(),
            tool_registry: self.tool_registry.clone(),
            working_dir: self.working_dir.clone(),
            context_dir: self.context_dir.clone(),
            llm: self.llm.clone(),
            registry: self.registry.clone(),
            approval_runtime: self.approval_runtime.clone(),
            approval_mode: self.approval_mode,
        }
    }
}

impl SubagentRunner {
    /// 创建新的子代理执行器
    pub fn new(
        tool_registry: ToolRegistry,
        working_dir: impl AsRef<Path>,
        context_dir: impl AsRef<Path>,
    ) -> Self {
        Self {
            labor_market: LaborMarket::new(),
            tool_registry,
            working_dir: working_dir.as_ref().to_path_buf(),
            context_dir: context_dir.as_ref().to_path_buf(),
            llm: None,
            registry: None,
            approval_runtime: None,
            approval_mode: ApprovalMode::Interactive,
        }
    }

    /// 设置 LLM 提供者（默认）
    pub fn with_llm(mut self, llm: Arc<dyn LlmProvider>) -> Self {
        self.llm = Some(llm);
        self
    }

    /// 设置模型注册表（用于 model_override 动态选择）
    pub fn with_registry(mut self, registry: ModelRegistry) -> Self {
        self.registry = Some(registry);
        self
    }

    /// 设置审批运行时
    pub fn with_approval_runtime(mut self, runtime: Arc<dyn ApprovalRuntime>) -> Self {
        self.approval_runtime = Some(runtime);
        self
    }

    /// 设置审批模式
    pub fn with_approval_mode(mut self, mode: ApprovalMode) -> Self {
        self.approval_mode = mode;
        self
    }

    /// 生成代理 ID
    fn generate_agent_id(&self) -> String {
        use rand::Rng;
        let mut rng = rand::thread_rng();
        let id: String = (0..8)
            .map(|_| rng.sample(rand::distributions::Alphanumeric) as char)
            .collect();
        format!("a{}", id.to_lowercase())
    }

    /// 执行子代理（主要入口）
    pub async fn run(
        &self,
        spec: RunSpec,
        store: &mut SubagentStore,
        parent_wire: Option<&Wire>,
    ) -> Result<SubagentResult, SubagentError> {
        let start_time = SystemTime::now();
        let started_at = start_time.duration_since(UNIX_EPOCH).unwrap().as_secs();

        // 1. 准备或恢复代理实例
        let (agent_id, agent_type, resumed) = self.prepare_instance(&spec, store).await?;

        info!(
            "Starting subagent {} (type: {}, resumed: {})",
            agent_id, agent_type, resumed
        );

        // 2. 获取类型定义
        let type_def = self
            .labor_market
            .get(&agent_type)
            .ok_or_else(|| SubagentError::UnknownAgentType(agent_type.clone()))?
            .clone();

        // 3. 创建执行上下文
        let mut context = ExecutionContext::new(&self.context_dir, &agent_id);
        context.set_capability_token(spec.capability_token.clone());
        context.restore().await?;

        // 4. 创建输出收集器
        let mut collector = OutputCollector::new(context.output_path());
        collector.stage("runner_started");

        // 5. 构建代理
        let agent = self
            .build_agent(
                &agent_id,
                &type_def,
                spec.max_iterations,
                store,
                spec.git_context,
                spec.capability_token.clone(),
            )
            .await?;
        collector.stage("agent_built");

        // 5.5 根据 model_override 动态选择 LLM
        if let Some(ref model_alias) = spec.model_override {
            if let Some(ref registry) = self.registry {
                match registry.get(model_alias) {
                    Some(entry) => {
                        if let Some(provider_cfg) = registry.get_provider(&entry.provider) {
                            match build_provider_from_registry(provider_cfg, &entry.model_id).await
                            {
                                Ok(new_llm) => {
                                    agent.set_llm(Arc::from(new_llm));
                                    collector.stage(format!(
                                        "llm_switched_to: {} (provider: {}, model: {})",
                                        model_alias, entry.provider, entry.model_id
                                    ));
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to build provider for model '{}': {}. Using default LLM.",
                                        model_alias, e
                                    );
                                    collector.stage(format!(
                                        "llm_switch_failed: {} (fallback to default)",
                                        model_alias
                                    ));
                                }
                            }
                        } else {
                            warn!(
                                "Provider '{}' for model '{}' not found in registry. Using default LLM.",
                                entry.provider, model_alias
                            );
                        }
                    }
                    None => {
                        warn!(
                            "Model alias '{}' not found in registry. Using default LLM.",
                            model_alias
                        );
                    }
                }
            } else {
                warn!(
                    "model_override '{}' specified but no ModelRegistry configured. Using default LLM.",
                    model_alias
                );
            }
        }

        // 6. 准备提示词
        let prompt = self
            .prepare_prompt(&spec, &type_def, &context, resumed)
            .await?;
        context.write_prompt_snapshot(&prompt).await?;
        collector.stage("prompt_prepared");

        // 7. 更新状态为运行中
        store.update_status(&agent_id, SubagentStatus::Running);
        collector.stage("status_updated_to_running");

        // 8. 执行代理循环
        let result = self
            .execute_agent(&agent, &prompt, &mut context, &mut collector, parent_wire)
            .await;

        // 9. 处理结果
        let elapsed = SystemTime::now().duration_since(start_time).unwrap();
        let completed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        match result {
            Ok(summary) => {
                collector.stage("execution_succeeded");
                collector.save_summary(&summary).await?;
                store.update_status(&agent_id, SubagentStatus::Completed);

                Ok(SubagentResult {
                    agent_id,
                    agent_type,
                    status: ExecutionStatus::Success,
                    summary: summary.clone(),
                    full_output: collector.full_output(),
                    resumed,
                    steps_taken: context.history().len(),
                    elapsed_ms: elapsed.as_millis() as u64,
                    started_at,
                    completed_at,
                })
            }
            Err(SubagentError::Cancelled) => {
                collector.stage("execution_cancelled");
                store.update_status(&agent_id, SubagentStatus::Failed);

                Err(SubagentError::Cancelled)
            }
            Err(e) => {
                collector.stage(format!("execution_failed: {}", e));
                store.update_status(&agent_id, SubagentStatus::Failed);

                Err(e)
            }
        }
    }

    /// 准备或恢复代理实例
    async fn prepare_instance(
        &self,
        spec: &RunSpec,
        store: &mut SubagentStore,
    ) -> Result<(String, String, bool), SubagentError> {
        if let Some(resume_id) = &spec.resume {
            // 恢复现有实例
            let state = store.get(resume_id).ok_or_else(|| {
                SubagentError::ResumeFailed(format!("Agent instance {} not found", resume_id))
            })?;

            if state.status == SubagentStatus::Running {
                return Err(SubagentError::ResumeFailed(format!(
                    "Agent instance {} is already running",
                    resume_id
                )));
            }

            info!(
                "Resuming subagent {} (type: {})",
                resume_id, state.agent_type
            );
            Ok((resume_id.clone(), state.agent_type.clone(), true))
        } else {
            // 创建新实例
            let agent_id = self.generate_agent_id();
            let agent_type = spec.requested_type.clone();

            store.create(agent_id.clone(), agent_type.clone());

            // 确保上下文目录存在
            let agent_dir = self.context_dir.join(&agent_id);
            fs::create_dir_all(&agent_dir).await.map_err(|e| {
                SubagentError::StoreError(format!("Failed to create context directory: {}", e))
            })?;

            Ok((agent_id, agent_type, false))
        }
    }

    /// 构建代理
    async fn build_agent(
        &self,
        agent_id: &str,
        type_def: &AgentTypeDefinition,
        max_iterations_override: Option<usize>,
        _store: &SubagentStore,
        enable_git_context: bool,
        capability_token: Option<CapabilityToken>,
    ) -> Result<Agent, SubagentError> {
        let git_ctx = if enable_git_context {
            GitContext::collect(&self.working_dir)
                .await
                .map(|ctx| ctx.to_prompt_string())
        } else {
            None
        };

        let mut builder = SubagentBuilder::new(self.tool_registry.clone(), &self.working_dir)
            .with_git_context(git_ctx);

        if let Some(token) = capability_token {
            builder = builder.with_capability_token(token);
        }

        let mut store_for_build = SubagentStore::new(&self.context_dir);
        store_for_build.create(agent_id.to_string(), type_def.name.clone());

        let agent = builder.build(agent_id, type_def, &mut store_for_build)?;

        // 应用覆盖设置
        let _max_iterations = max_iterations_override.unwrap_or(type_def.max_iterations);

        // 如果有 LLM，设置 LLM
        let agent = if let Some(llm) = &self.llm {
            agent.with_llm(llm.clone())
        } else {
            agent
        };

        // 如果有审批运行时，设置审批
        let agent = if let Some(runtime) = &self.approval_runtime {
            agent
                .with_approval_runtime(runtime.clone())
                .with_approval_mode(self.approval_mode)
        } else {
            agent
        };

        Ok(agent)
    }

    /// 准备提示词
    async fn prepare_prompt(
        &self,
        spec: &RunSpec,
        _type_def: &AgentTypeDefinition,
        context: &ExecutionContext,
        resumed: bool,
    ) -> Result<String, SubagentError> {
        let mut prompt = spec.prompt.clone();

        // 如果有历史上下文，添加为参考
        let history = context.history();
        if !history.is_empty() && resumed {
            let history_summary: Vec<String> = history
                .iter()
                .map(|m| {
                    format!(
                        "[{:?}]: {}",
                        m.role,
                        m.content.chars().take(100).collect::<String>()
                    )
                })
                .collect();

            prompt = format!(
                "# Previous Conversation Context\n{}\n\n# Current Task\n{}",
                history_summary.join("\n"),
                prompt
            );
        }

        Ok(prompt)
    }

    /// 执行代理循环
    async fn execute_agent(
        &self,
        agent: &Agent,
        prompt: &str,
        _context: &mut ExecutionContext,
        collector: &mut OutputCollector,
        _parent_wire: Option<&Wire>,
    ) -> Result<String, SubagentError> {
        collector.stage("execution_started");

        // 执行代理
        let result = agent.run(prompt).await;

        match result {
            Ok(response) => {
                collector.stage("agent_completed_successfully");

                // 验证响应长度
                if response.len() < 100 {
                    // 响应太短，尝试继续
                    collector.stage("response_too_short_attempting_continuation");

                    let continuation_prompt = r#"
Your previous response was too brief. Please provide a more comprehensive summary that includes:

1. Specific technical details and implementations
2. Detailed findings and analysis
3. All important information that should be known
"#;

                    match agent.run(continuation_prompt).await {
                        Ok(extended) => {
                            collector.stage("continuation_succeeded");
                            Ok(format!("{}\n\n{}", response, extended))
                        }
                        Err(e) => {
                            warn!("Continuation failed: {}", e);
                            Ok(response)
                        }
                    }
                } else {
                    Ok(response)
                }
            }
            Err(AgentError::MaxIterationsExceeded(n)) => Err(SubagentError::MaxStepsReached {
                steps: n,
                phase: "execution".into(),
            }),
            Err(e) => Err(SubagentError::ExecutionFailed {
                message: e.to_string(),
                brief: "agent execution failed".into(),
            }),
        }
    }

    /// 列出可用的代理类型
    pub fn list_agent_types(&self) -> Vec<&AgentTypeDefinition> {
        self.labor_market.list()
    }

    /// 获取劳动力市场
    pub fn labor_market(&self) -> &LaborMarket {
        &self.labor_market
    }

    /// 获取工作目录
    pub fn working_dir(&self) -> &Path {
        &self.working_dir
    }
}

// =============================================================================
// 测试
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_registry() -> ToolRegistry {
        ToolRegistry::with_builtin_tools()
    }

    fn create_test_runner() -> (SubagentRunner, TempDir, TempDir) {
        let registry = create_test_registry();
        let work_dir = TempDir::new().unwrap();
        let context_dir = TempDir::new().unwrap();

        let runner = SubagentRunner::new(registry, work_dir.path(), context_dir.path());

        (runner, work_dir, context_dir)
    }

    #[tokio::test]
    async fn test_generate_agent_id() {
        let (runner, _work, _context) = create_test_runner();
        let id1 = runner.generate_agent_id();
        let id2 = runner.generate_agent_id();

        assert!(id1.starts_with('a'));
        assert!(id2.starts_with('a'));
        assert_ne!(id1, id2);
        assert_eq!(id1.len(), 9); // 'a' + 8 chars
    }

    #[tokio::test]
    async fn test_run_spec_builder() {
        let spec = RunSpec::new("Test task", "Do something")
            .with_type("coder")
            .with_model("gpt-4")
            .with_max_iterations(20);

        assert_eq!(spec.description, "Test task");
        assert_eq!(spec.prompt, "Do something");
        assert_eq!(spec.requested_type, "coder");
        assert_eq!(spec.model_override, Some("gpt-4".to_string()));
        assert_eq!(spec.max_iterations, Some(20));
    }

    #[tokio::test]
    async fn test_execution_context_save_restore() {
        let temp_dir = TempDir::new().unwrap();
        let mut context = ExecutionContext::new(temp_dir.path(), "test-agent");

        // 添加一些消息
        context.push_message(Message::user("Hello"));
        context.push_message(Message::assistant("Hi there!"));

        // 保存
        context.save().await.unwrap();

        // 恢复
        let mut restored = ExecutionContext::new(temp_dir.path(), "test-agent");
        restored.restore().await.unwrap();

        assert_eq!(restored.history().len(), 2);
    }

    #[tokio::test]
    async fn test_output_collector() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("output.txt");

        let mut collector = OutputCollector::new(&output_path);
        collector.stage("stage1");
        collector.append("Some output");
        collector.stage("stage2");

        collector.save_summary("Final summary").await.unwrap();

        let content = tokio::fs::read_to_string(&output_path).await.unwrap();
        assert!(content.contains("stage1"));
        assert!(content.contains("stage2"));
        assert!(content.contains("Final summary"));
    }

    #[tokio::test]
    async fn test_subagent_result_serialization() {
        let result = SubagentResult {
            agent_id: "a1234567".into(),
            agent_type: "coder".into(),
            status: ExecutionStatus::Success,
            summary: "Test summary".into(),
            full_output: "Full output".into(),
            resumed: false,
            steps_taken: 5,
            elapsed_ms: 1000,
            started_at: 1234567890,
            completed_at: 1234568890,
        };

        let json = serde_json::to_string(&result).unwrap();
        let deserialized: SubagentResult = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.agent_id, result.agent_id);
        assert_eq!(deserialized.status, result.status);
    }

    #[tokio::test]
    async fn test_git_context_collect_none() {
        let temp_dir = TempDir::new().unwrap();
        let ctx = GitContext::collect(temp_dir.path()).await;
        assert!(ctx.is_none());
    }

    #[tokio::test]
    async fn test_git_context_format() {
        let ctx = GitContext {
            branch: Some("main".to_string()),
            recent_commits: vec!["abc123 fix bug".to_string(), "def456 add feat".to_string()],
            status_summary: "2 uncommitted files".to_string(),
            repo_root: Some(PathBuf::from("/repo")),
        };
        let s = ctx.to_prompt_string();
        assert!(s.contains("main"));
        assert!(s.contains("abc123 fix bug"));
        assert!(s.contains("2 uncommitted files"));
    }

    #[tokio::test]
    async fn test_git_context_collect_repo() {
        let temp_dir = TempDir::new().unwrap();
        let repo_dir = temp_dir.path().join("repo");
        fs::create_dir(&repo_dir).await.unwrap();

        // init git repo
        let init = tokio::process::Command::new("git")
            .args(["init"])
            .current_dir(&repo_dir)
            .output()
            .await
            .unwrap();
        assert!(init.status.success());

        // config user
        let _ = tokio::process::Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(&repo_dir)
            .output()
            .await;
        let _ = tokio::process::Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(&repo_dir)
            .output()
            .await;

        // create file and commit
        fs::write(repo_dir.join("readme.txt"), "hello")
            .await
            .unwrap();
        let add = tokio::process::Command::new("git")
            .args(["add", "."])
            .current_dir(&repo_dir)
            .output()
            .await
            .unwrap();
        assert!(add.status.success());

        let commit = tokio::process::Command::new("git")
            .args(["commit", "-m", "initial"])
            .current_dir(&repo_dir)
            .output()
            .await
            .unwrap();
        assert!(commit.status.success());

        let ctx = GitContext::collect(&repo_dir)
            .await
            .expect("should collect");
        assert!(ctx.branch.is_some());
        assert!(!ctx.recent_commits.is_empty());
        assert_eq!(ctx.status_summary, "clean");
        assert!(ctx.repo_root.is_some());
    }

    #[tokio::test]
    async fn test_subagent_builder_git_context_injection() {
        let registry = create_test_registry();
        let builder = SubagentBuilder::new(registry, "/tmp")
            .with_git_context(Some("# Git Context\n\nCurrent branch: main\n".to_string()));
        let mut store = SubagentStore::new("/tmp/store");

        let type_def = builder.labor_market().require("coder");
        let agent = builder.build("test-git", type_def, &mut store).unwrap();

        assert!(agent.config().system_prompt.contains("Git Context"));
        assert!(agent.config().system_prompt.contains("main"));
    }
}
