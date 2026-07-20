//! Subagent Runner - 子代理执行器
//!
//! 负责执行子代理任务，管理生命周期，处理上下文传递和结果收集。
//!
//! 设计参考：
//! - std::process::Command 的构建器模式
//! - std::io 的读写抽象
//! - Rust 错误处理的最佳实践

// P1-2: Import the trait alongside the concrete type.
// `execute_agent` now accepts `&dyn AgentExecutor`, but `build_agent` still
// returns the concrete `Agent` so that caller-side builder methods work.
use crate::builder::SubagentBuilder;
use crate::store::SubagentStore;
use clarity_contract::ApprovalMode;
use clarity_contract::error::AgentError;
use clarity_contract::subagent::AgentExecutor;
use clarity_contract::subagent::{
    AgentTypeDefinition, CapabilityToken, ExecutionStatus, GitContext, LaborMarket, RunSpec,
    SubagentError, SubagentProgressEvent, SubagentResult, SubagentStatus,
};
use clarity_core::agent::Agent;
use clarity_core::approval::ApprovalRuntime;
use clarity_core::registry::ToolRegistry;
use clarity_llm::api::{LlmProvider, Message};
use clarity_llm::{ModelRegistry, build_provider_from_registry_entry, default_secret_store};
use clarity_wire::{Wire, WireMessage};

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::fs;
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

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
    progress_tx: Option<mpsc::Sender<SubagentProgressEvent>>,
    agent_id: String,
}

impl OutputCollector {
    /// 创建新的输出收集器
    pub fn new(output_path: impl AsRef<Path>, agent_id: impl Into<String>) -> Self {
        Self {
            output_path: output_path.as_ref().to_path_buf(),
            stages: Vec::new(),
            content: Vec::new(),
            progress_tx: None,
            agent_id: agent_id.into(),
        }
    }

    /// Attach a progress channel for real-time UI updates.
    pub fn with_progress_tx(mut self, tx: mpsc::Sender<SubagentProgressEvent>) -> Self {
        self.progress_tx = Some(tx);
        self
    }

    /// 记录阶段
    pub fn stage(&mut self, stage: impl Into<String>) {
        let stage = stage.into();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_millis();
        self.stages.push(format!("[{}] {}", timestamp, stage));
        debug!("Subagent stage: {}", stage);
        if let Some(ref tx) = self.progress_tx {
            let _ = tx.try_send(SubagentProgressEvent::Stage {
                agent_id: self.agent_id.clone(),
                name: stage,
            });
        }
    }

    /// 追加内容
    pub fn append(&mut self, text: impl Into<String>) {
        let text = text.into();
        self.content.push(text.clone());
        if let Some(ref tx) = self.progress_tx {
            let _ = tx.try_send(SubagentProgressEvent::Output {
                agent_id: self.agent_id.clone(),
                text,
            });
        }
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
    /// 共享迭代预算（父子代理共用）
    iteration_budget: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
    /// Real-time progress channel for UI monitoring.
    progress_tx: Option<mpsc::Sender<SubagentProgressEvent>>,
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
            iteration_budget: self.iteration_budget.clone(),
            progress_tx: self.progress_tx.clone(),
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
            iteration_budget: None,
            progress_tx: None,
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

    /// 设置共享迭代预算
    pub fn with_iteration_budget(
        mut self,
        budget: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) -> Self {
        self.iteration_budget = Some(budget);
        self
    }

    /// Attach a real-time progress channel for UI monitoring.
    pub fn with_progress_tx(mut self, tx: mpsc::Sender<SubagentProgressEvent>) -> Self {
        self.progress_tx = Some(tx);
        self
    }

    /// 生成代理 ID
    fn generate_agent_id(&self) -> String {
        use rand::RngExt;
        let mut rng = rand::rng();
        let id: String = (0..8)
            .map(|_| rng.sample(rand::distr::Alphanumeric) as char)
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
        let started_at = start_time
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs();

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

        // 2.5 设置迭代预算（用于进度估计）
        let max_iters = spec.max_iterations.unwrap_or(type_def.max_iterations);
        store.set_budget(&agent_id, max_iters);

        // 3. 创建执行上下文；如果启用了 worktree，创建隔离工作空间。
        let mut token = spec.capability_token.clone();
        let mut worktree_guard: Option<WorktreeGuard> =
            if token.as_ref().map(|t| t.enable_worktree).unwrap_or(false) {
                match create_worktree_for_agent(&agent_id, &self.working_dir).await {
                    Ok(path) => {
                        if let Some(ref mut t) = token {
                            t.sandbox_dir = Some(path.clone());
                        }
                        Some(WorktreeGuard::new(path))
                    }
                    Err(e) => {
                        warn!("Failed to create worktree for agent {}: {}", agent_id, e);
                        None
                    }
                }
            } else {
                None
            };
        let mut context = ExecutionContext::new(&self.context_dir, &agent_id);
        context.set_capability_token(token);
        context.restore().await?;

        // 4. 创建输出收集器（ponytail: 单次构造，通过 with_progress_tx 链式配置）
        let mut collector = OutputCollector::new(context.output_path(), &agent_id);
        if let Some(ref tx) = self.progress_tx {
            collector = collector.with_progress_tx(tx.clone());
        }
        if worktree_guard.is_some() {
            collector.stage("worktree_created");
        }
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
                spec.read_only,
            )
            .await?;
        collector.stage("agent_built");

        // 5.5 根据 model_override 动态选择 LLM
        let secrets = default_secret_store().ok();
        if let Some(ref model_alias) = spec.model_override {
            if let Some(ref registry) = self.registry {
                match registry.get(model_alias) {
                    Some(entry) => {
                        if let Some(provider_cfg) = registry.get_provider(&entry.provider) {
                            match build_provider_from_registry_entry(
                                provider_cfg,
                                entry,
                                None,
                                secrets.as_ref(),
                            )
                            .await
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

        // 5.6 如果指定了 output_schema，配置结构化输出
        if let Some(ref schema) = spec.output_schema {
            if let Some(ref llm) = agent.llm() {
                let rf = serde_json::json!({
                    "type": "json_schema",
                    "json_schema": {
                        "name": "subagent_output",
                        "schema": schema,
                        "strict": true
                    }
                });
                llm.set_response_format(Some(rf));
                collector.stage("response_format_set");
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
        let _ = store.save(&agent_id).await;
        if let Some(ref tx) = self.progress_tx {
            let _ = tx.try_send(SubagentProgressEvent::StatusChange {
                agent_id: agent_id.clone(),
                agent_type: agent_type.clone(),
                status: SubagentStatus::Running,
            });
            let _ = tx.try_send(SubagentProgressEvent::Progress {
                agent_id: agent_id.clone(),
                steps: 0,
                max_steps: max_iters,
            });
        }
        collector.stage("status_updated_to_running");

        // 8. 执行代理循环
        let result = self
            .execute_agent(&agent, &prompt, &mut context, &mut collector, parent_wire)
            .await;

        // 8.5 读取实际执行的步数
        let steps_taken = agent.last_turn_message_count();
        store.set_steps_taken(&agent_id, steps_taken);

        // 9. 处理结果
        let elapsed = SystemTime::now()
            .duration_since(start_time)
            .unwrap_or(std::time::Duration::ZERO);
        let completed_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(std::time::Duration::ZERO)
            .as_secs();

        match result {
            Ok(summary) => {
                collector.stage("execution_succeeded");
                collector.save_summary(&summary).await?;
                store.update_status(&agent_id, SubagentStatus::Completed);
                let _ = store.save(&agent_id).await;
                if let Some(ref tx) = self.progress_tx {
                    let _ = tx.try_send(SubagentProgressEvent::StatusChange {
                        agent_id: agent_id.clone(),
                        agent_type: agent_type.clone(),
                        status: SubagentStatus::Completed,
                    });
                    let _ = tx.try_send(SubagentProgressEvent::Progress {
                        agent_id: agent_id.clone(),
                        steps: steps_taken,
                        max_steps: max_iters,
                    });
                }

                // Mark worktree for cleanup on success.
                if let Some(ref mut guard) = worktree_guard {
                    guard.mark_success();
                }

                Ok(SubagentResult {
                    agent_id,
                    agent_type,
                    status: ExecutionStatus::Success,
                    summary: summary.clone(),
                    full_output: collector.full_output(),
                    resumed,
                    steps_taken,
                    elapsed_ms: elapsed.as_millis() as u64,
                    started_at,
                    completed_at,
                    monitoring_enabled: false,
                })
            }
            Err(SubagentError::Cancelled) => {
                collector.stage("execution_cancelled");
                store.update_status(&agent_id, SubagentStatus::Failed);
                let _ = store.save(&agent_id).await;
                if let Some(ref tx) = self.progress_tx {
                    let _ = tx.try_send(SubagentProgressEvent::StatusChange {
                        agent_id: agent_id.clone(),
                        agent_type: agent_type.clone(),
                        status: SubagentStatus::Failed,
                    });
                    let _ = tx.try_send(SubagentProgressEvent::Progress {
                        agent_id: agent_id.clone(),
                        steps: steps_taken,
                        max_steps: max_iters,
                    });
                }

                Err(SubagentError::Cancelled)
            }
            Err(e) => {
                collector.stage(format!("execution_failed: {}", e));
                store.update_status(&agent_id, SubagentStatus::Failed);
                let _ = store.save(&agent_id).await;
                if let Some(ref tx) = self.progress_tx {
                    let _ = tx.try_send(SubagentProgressEvent::StatusChange {
                        agent_id: agent_id.clone(),
                        agent_type: agent_type.clone(),
                        status: SubagentStatus::Failed,
                    });
                    let _ = tx.try_send(SubagentProgressEvent::Progress {
                        agent_id: agent_id.clone(),
                        steps: steps_taken,
                        max_steps: max_iters,
                    });
                }

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
            // 恢复现有实例 — 先查内存，再查磁盘
            if store.get(resume_id).is_none() {
                let _ = store.load(resume_id).await;
            }
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
    #[allow(clippy::too_many_arguments)] // Internal helper; grouping would add a new struct.
    async fn build_agent(
        &self,
        agent_id: &str,
        type_def: &AgentTypeDefinition,
        max_iterations_override: Option<usize>,
        _store: &SubagentStore,
        enable_git_context: bool,
        capability_token: Option<CapabilityToken>,
        read_only_override: bool,
    ) -> Result<Agent, SubagentError> {
        let git_ctx = if enable_git_context {
            GitContext::collect(&self.working_dir)
                .await
                .map(|ctx| ctx.to_prompt_string())
        } else {
            None
        };

        let mut builder = SubagentBuilder::new(self.tool_registry.clone(), &self.working_dir)
            .with_git_context(git_ctx)
            .with_read_only(read_only_override);

        if let Some(token) = capability_token {
            builder = builder.with_capability_token(token);
        }

        if let Some(ref budget) = self.iteration_budget {
            builder = builder.with_iteration_budget(budget.clone());
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
        agent: &dyn AgentExecutor,
        prompt: &str,
        _context: &mut ExecutionContext,
        collector: &mut OutputCollector,
        _parent_wire: Option<&Wire>,
    ) -> Result<String, SubagentError> {
        collector.stage("execution_started");

        // 执行代理
        let result = agent.run_turn(prompt).await;

        match result {
            Ok(response) => {
                collector.stage("agent_completed_successfully");

                // 验证响应长度
                if response.len() < MIN_RESPONSE_CHARS {
                    // 响应太短，尝试继续
                    collector.stage("response_too_short_attempting_continuation");

                    let continuation_prompt = r#"
Your previous response was too brief. Please provide a more comprehensive summary that includes:

1. Specific technical details and implementations
2. Detailed findings and analysis
3. All important information that should be known
"#;

                    match agent.run_turn(continuation_prompt).await {
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

    /// 在 monitoring 模式下执行子代理（先简单委托给 run，后续迭代增强）
    pub async fn run_with_monitoring(
        &self,
        spec: RunSpec,
        store: &mut SubagentStore,
        parent_wire: Option<&Wire>,
    ) -> Result<SubagentResult, SubagentError> {
        let mut result = self.run(spec, store, parent_wire).await?;
        result.monitoring_enabled = true;
        Ok(result)
    }

    /// 列出可用的代理类型
    pub fn list_agent_types(&self) -> Vec<&AgentTypeDefinition> {
        self.labor_market.list()
    }

    /// 获取劳动力市场
    pub fn labor_market(&self) -> &LaborMarket {
        &self.labor_market
    }

    /// Register a custom agent type in the labor market.
    pub fn register_type(&mut self, def: AgentTypeDefinition) {
        self.labor_market.register(def);
    }

    /// 获取工作目录
    pub fn working_dir(&self) -> &Path {
        &self.working_dir
    }
}

/// Minimum response length in characters before the runner considers the
/// agent output "too short" and requests an automatic continuation.
/// Set to 100 as a heuristic: anything shorter than a typical paragraph
/// likely means the model stopped prematurely.
const MIN_RESPONSE_CHARS: usize = 100;

// =============================================================================
// Worktree isolation helpers
// =============================================================================

/// RAII guard that cleans up a git worktree on drop.
///
/// Call [`WorktreeGuard::mark_success`] before dropping to remove the
/// worktree on completion; on error (drop without marking), the worktree
/// is preserved for debugging.
struct WorktreeGuard {
    path: std::path::PathBuf,
    should_remove: bool,
}

impl WorktreeGuard {
    fn new(path: std::path::PathBuf) -> Self {
        Self {
            path,
            should_remove: false,
        }
    }

    fn mark_success(&mut self) {
        self.should_remove = true;
    }
}

impl Drop for WorktreeGuard {
    fn drop(&mut self) {
        if self.should_remove {
            let _ = std::fs::remove_dir_all(&self.path);
            // Also prune the worktree from git's metadata.
            let _ = std::process::Command::new("git")
                .args(["worktree", "prune"])
                .output();
        }
    }
}

/// Create a git worktree under `.clarity/worktrees/<agent_id>`.
///
/// Returns the path to the worktree root on success.
async fn create_worktree_for_agent(
    agent_id: &str,
    repo_root: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    let worktree_root = repo_root.join(".clarity").join("worktrees");
    let agent_worktree = worktree_root.join(agent_id);

    // Create parent directory if needed.
    tokio::fs::create_dir_all(&worktree_root)
        .await
        .map_err(|e| format!("Failed to create worktree root: {}", e))?;

    // Remove existing worktree if present (e.g. from a previous failed run).
    if agent_worktree.exists() {
        tokio::fs::remove_dir_all(&agent_worktree)
            .await
            .map_err(|e| format!("Failed to clean stale worktree: {}", e))?;
    }

    // Create a branch name from the agent_id.
    let branch = format!("clarity-wt-{}", &agent_id[..agent_id.len().min(20)]);

    let output = tokio::process::Command::new("git")
        .args([
            "worktree",
            "add",
            "--detach",
            agent_worktree.to_str().ok_or("invalid worktree path")?,
        ])
        .current_dir(repo_root)
        .output()
        .await
        .map_err(|e| format!("Failed to run git worktree add: {}", e))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git worktree add failed: {}", stderr));
    }

    // Delete the branch reference to avoid cluttering `git branch` output.
    let _ = tokio::process::Command::new("git")
        .args(["branch", "-D", &branch])
        .current_dir(repo_root)
        .output()
        .await;

    Ok(agent_worktree)
}

// =============================================================================
// 测试
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::subagent::{
        ExecutionStatus, GitContext, RunSpec, SubagentError, SubagentResult,
    };
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

        let mut collector = OutputCollector::new(&output_path, "test-agent");
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
            monitoring_enabled: false,
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

        let type_def = builder
            .labor_market()
            .require("coder")
            .expect("coder type exists");
        let agent = builder.build("test-git", type_def, &mut store).unwrap();

        assert!(agent.config().system_prompt.contains("Git Context"));
        assert!(agent.config().system_prompt.contains("main"));
    }

    #[tokio::test]
    async fn test_runner_budget_zero_exhaustion() {
        let registry = create_test_registry();
        let work_dir = TempDir::new().unwrap();
        let context_dir = TempDir::new().unwrap();

        let budget = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let runner = SubagentRunner::new(registry, work_dir.path(), context_dir.path())
            .with_llm(std::sync::Arc::new(clarity_core::agent::MockLlm))
            .with_iteration_budget(budget.clone());

        let mut store = SubagentStore::new(context_dir.path());
        let spec = RunSpec::new("Test budget exhaustion", "Do something")
            .with_type("coder")
            .without_git_context();

        let result = runner.run(spec, &mut store, None).await;

        assert!(
            matches!(result, Err(SubagentError::MaxStepsReached { .. })),
            "Expected MaxStepsReached when budget=0, got: {:?}",
            result
        );
    }

    #[tokio::test]
    async fn test_runner_shared_budget_sequential_runs() {
        let registry = create_test_registry();
        let work_dir = TempDir::new().unwrap();
        let context_dir = TempDir::new().unwrap();

        // Budget = 2: first run consumes 2 iterations (main + continuation),
        // second run fails immediately because budget is 0.
        let budget = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(2));
        let runner = SubagentRunner::new(registry, work_dir.path(), context_dir.path())
            .with_llm(std::sync::Arc::new(clarity_core::agent::MockLlm))
            .with_iteration_budget(budget.clone());

        let mut store = SubagentStore::new(context_dir.path());

        // First run: MockLlm returns is_complete=true.
        // execute_agent() calls run_turn (consumes 1) then continuation run_turn (consumes 1).
        let spec1 = RunSpec::new("First", "Do A")
            .with_type("coder")
            .without_git_context();
        let result1 = runner.run(spec1, &mut store, None).await;
        assert!(result1.is_ok(), "First run should succeed: {:?}", result1);

        // Second run: budget is now 0 → immediate MaxStepsReached
        let spec2 = RunSpec::new("Second", "Do B")
            .with_type("coder")
            .without_git_context();
        let result2 = runner.run(spec2, &mut store, None).await;
        assert!(
            matches!(result2, Err(SubagentError::MaxStepsReached { .. })),
            "Second run should fail after budget exhausted: {:?}",
            result2
        );
    }

    // ── WorktreeGuard transactional tests ──

    #[test]
    fn worktree_guard_preserves_on_error() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("preserve_me");
        std::fs::create_dir_all(&path).unwrap();
        std::fs::write(path.join("data.txt"), "important").unwrap();

        {
            let guard = WorktreeGuard::new(path.clone());
            // Drop without mark_success — simulates error path.
            drop(guard);
        }

        assert!(path.exists(), "worktree preserved when not marked success");
    }

    #[test]
    fn worktree_guard_cleans_on_success() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("clean_me");
        std::fs::create_dir_all(&path).unwrap();

        {
            let mut guard = WorktreeGuard::new(path.clone());
            guard.mark_success();
            // Drop after mark_success — simulates success path.
            drop(guard);
        }

        assert!(!path.exists(), "worktree cleaned up on success");
    }

    #[test]
    fn worktree_guard_default_is_preserve() {
        let guard = WorktreeGuard::new(PathBuf::from("/nonexistent"));
        assert!(
            !guard.should_remove,
            "default should_remove is false (preserve)"
        );
    }
}
