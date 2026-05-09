//! Subagent management system
//!
//! 提供子代理的完整生命周期管理，包括：
//! - **LaborMarket**: 子代理类型注册表
//! - **SubagentStore**: 状态存储
//! - **SubagentBuilder**: 构建器
//! - **SubagentRunner**: 执行器
//! - **ParallelExecutor**: 并行执行
//!
//! 设计参考 std::process::Command 的构建器模式。

pub mod builder;
mod parallel;
pub mod registry;
pub mod runner;
mod store;
pub mod team;
pub mod token;

// Re-export contract-level subagent types so existing `use clarity_subagents::TypeName`
// continue to work. New code should prefer `clarity_contract::subagent::TypeName`.
pub use clarity_contract::subagent::{
    collect_git_context, AgentTeam, AgentTypeDefinition, BatchProgress, BatchProgressHandle,
    BatchStatus, CapabilityToken, ExecutionStatus, GitContext, LaborMarket, Mailbox, MailboxError,
    MailboxMessage, MessagePayload, ParallelConfig, ParallelResult, RunSpec, SubagentError,
    SubagentOrchestrator, SubagentProgressEvent, SubagentResult, SubagentState, SubagentStatus,
    TeamResult, TokenError,
};

// Re-export local types with logic.
pub use builder::SubagentBuilder;
pub use parallel::{
    run_parallel, ParallelExecutor, SubagentBatch,
};
pub use runner::{
    ExecutionContext, OutputCollector, SubagentRunner,
};
pub use store::SubagentStore;
pub use team::TeamCoordinator;

use clarity_core::agent::jumpy::predictor::OutcomePredictor;
use clarity_core::agent::jumpy::state::JumpyState;
use clarity_core::registry::ToolRegistry;
use clarity_llm::ModelRegistry;
use std::sync::Arc;

/// 子代理管理器
///
/// 整合所有子代理功能的高级接口。
pub struct SubagentManager {
    /// 存储
    store: SubagentStore,
    /// 执行器
    runner: SubagentRunner,
    /// Jumpy Predictor（可选）
    predictor: Option<Arc<dyn OutcomePredictor>>,
}

impl SubagentManager {
    /// 创建新的子代理管理器
    pub fn new(
        tool_registry: ToolRegistry,
        working_dir: impl AsRef<std::path::Path>,
        context_dir: impl AsRef<std::path::Path>,
    ) -> Self {
        let store = SubagentStore::new(&context_dir);
        let runner = SubagentRunner::new(tool_registry, working_dir, context_dir);

        Self {
            store,
            runner,
            predictor: None,
        }
    }

    /// 设置默认 LLM（builder 模式）
    pub fn with_llm(mut self, llm: Arc<dyn clarity_llm::api::LlmProvider>) -> Self {
        self.runner = self.runner.with_llm(llm);
        self
    }

    /// 设置模型注册表（支持 model_override 动态选择）
    pub fn with_registry(mut self, registry: ModelRegistry) -> Self {
        self.runner = self.runner.with_registry(registry);
        self
    }

    /// 设置 Jumpy Outcome Predictor（builder 模式）
    pub fn with_predictor(mut self, predictor: Arc<dyn OutcomePredictor>) -> Self {
        self.predictor = Some(predictor);
        self
    }

    /// Run a single subagent spec.
    pub async fn run(
        &mut self,
        spec: RunSpec,
        progress_tx: Option<
            tokio::sync::mpsc::Sender<SubagentProgressEvent>,
        >,
    ) -> Result<SubagentResult, SubagentError> {
        let runner = if let Some(tx) = progress_tx {
            self.runner.clone().with_progress_tx(tx)
        } else {
            self.runner.clone()
        };
        runner.run(spec, &mut self.store, None).await
    }

    /// 并行运行多个子代理
    pub async fn run_parallel(
        &self,
        specs: Vec<RunSpec>,
        config: ParallelConfig,
        progress: Option<std::sync::Arc<parking_lot::Mutex<BatchProgress>>>,
        cancel: Option<tokio_util::sync::CancellationToken>,
    ) -> anyhow::Result<ParallelResult> {
        use clarity_core::background::BackgroundTaskManager;

        let task_manager = BackgroundTaskManager::new(
            self.runner.working_dir().join("tasks"),
            self.runner.working_dir(),
            self.runner.working_dir().join("context"),
        );

        let batch = SubagentBatch::new().add_many(specs).with_config(config);

        let mut executor = ParallelExecutor::new(task_manager, self.runner.clone());
        executor.execute(batch, progress, cancel).await
    }

    /// Execute an [`AgentTeam`] and return a unified [`TeamResult`].
    pub async fn run_team(&self, team: AgentTeam) -> anyhow::Result<TeamResult> {
        use clarity_core::background::BackgroundTaskManager;

        let task_manager = BackgroundTaskManager::new(
            self.runner.working_dir().join("tasks"),
            self.runner.working_dir(),
            self.runner.working_dir().join("context"),
        );

        let mut coordinator = TeamCoordinator::new(task_manager, self.runner.clone());
        coordinator.execute_team(team).await
    }

    /// 捕获当前状态为 JumpyState
    fn capture_current_state(&self) -> JumpyState {
        JumpyState {
            tags: self.store.current_tags(),
            memory: self.store.working_memory(),
            active_files: self.store.active_files(),
            context_summary: self.store.context_summary(),
            progress: self.store.progress(),
        }
    }

    /// 预测驱动路由执行
    pub async fn run_with_prediction(
        &mut self,
        spec: RunSpec,
        progress_tx: Option<tokio::sync::mpsc::Sender<SubagentProgressEvent>>,
    ) -> Result<SubagentResult, SubagentError> {
        match &self.predictor {
            Some(predictor) => {
                let current = self.capture_current_state();
                match predictor
                    .predict(&spec.requested_type, &spec.prompt, &current, 0.9)
                    .await
                {
                    Ok(predicted) => {
                        let runner = if let Some(tx) = progress_tx {
                            self.runner.clone().with_progress_tx(tx)
                        } else {
                            self.runner.clone()
                        };
                        if predicted.satisfies(&spec.goal_tags) {
                            runner.run(spec, &mut self.store, None).await
                        } else {
                            runner
                                .run_with_monitoring(spec, &mut self.store, None)
                                .await
                        }
                    }
                    Err(_) => {
                        let runner = if let Some(tx) = progress_tx {
                            self.runner.clone().with_progress_tx(tx)
                        } else {
                            self.runner.clone()
                        };
                        runner.run(spec, &mut self.store, None).await
                    }
                }
            }
            None => self.run(spec, progress_tx).await,
        }
    }

    /// 获取 LaborMarket 引用
    pub fn labor_market(&self) -> &LaborMarket {
        self.runner.labor_market()
    }
}

#[async_trait::async_trait]
impl clarity_contract::subagent::SubagentOrchestrator for SubagentManager {
    async fn run_parallel(
        &self,
        specs: Vec<clarity_contract::subagent::RunSpec>,
        config: clarity_contract::subagent::ParallelConfig,
        progress: Option<clarity_contract::subagent::BatchProgressHandle>,
    ) -> Result<clarity_contract::subagent::ParallelResult, clarity_contract::subagent::SubagentError> {
        self.run_parallel(specs, config, progress, None)
            .await
            .map_err(|e| clarity_contract::subagent::SubagentError::BuildFailed(e.to_string()))
    }

    async fn run_team(
        &self,
        team: clarity_contract::subagent::AgentTeam,
    ) -> Result<clarity_contract::subagent::TeamResult, clarity_contract::subagent::SubagentError> {
        self.run_team(team)
            .await
            .map_err(|e| clarity_contract::subagent::SubagentError::BuildFailed(e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_registry() -> ToolRegistry {
        ToolRegistry::with_builtin_tools()
    }

    #[tokio::test]
    async fn test_manager_creation() {
        let registry = create_test_registry();
        let work_dir = TempDir::new().unwrap();
        let context_dir = TempDir::new().unwrap();

        let manager = SubagentManager::new(registry, work_dir.path(), context_dir.path());
        assert!(manager.labor_market().get("coder").is_some());
    }

    #[tokio::test]
    async fn test_subagent_batch() {
        let registry = create_test_registry();
        let work_dir = TempDir::new().unwrap();
        let context_dir = TempDir::new().unwrap();

        let mut manager = SubagentManager::new(registry, work_dir.path(), context_dir.path())
            .with_llm(Arc::new(clarity_core::agent::MockLlm));

        let spec = RunSpec::new("Test", "Do something")
            .with_type("coder")
            .without_git_context();

        let result = manager.run(spec, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_with_prediction_success() {
        let registry = create_test_registry();
        let work_dir = TempDir::new().unwrap();
        let context_dir = TempDir::new().unwrap();

        let mut manager = SubagentManager::new(registry, work_dir.path(), context_dir.path())
            .with_llm(Arc::new(clarity_core::agent::MockLlm));

        let spec = RunSpec::new("Test", "Do something")
            .with_type("coder")
            .without_git_context();

        let result = manager.run_with_prediction(spec.clone(), None).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().monitoring_enabled);
    }

    #[tokio::test]
    async fn test_run_with_prediction_uncertain() {
        use clarity_core::agent::jumpy::predictor::{
            ConsistentPredictor, HistoricalPredictor, HybridPredictor, LlmAdapter,
            LlmAugmentedPredictor, OutcomePredictor, SkillObservation,
        };

        let registry = create_test_registry();
        let work_dir = TempDir::new().unwrap();
        let context_dir = TempDir::new().unwrap();

        let mut manager = SubagentManager::new(registry, work_dir.path(), context_dir.path())
            .with_llm(Arc::new(clarity_core::agent::MockLlm));

        let spec = RunSpec::new("Test", "Do something")
            .with_type("coder")
            .without_git_context();

        let result = manager.run_with_prediction(spec, None).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_run_without_predictor() {
        let registry = create_test_registry();
        let work_dir = TempDir::new().unwrap();
        let context_dir = TempDir::new().unwrap();

        let mut manager = SubagentManager::new(registry, work_dir.path(), context_dir.path())
            .with_llm(Arc::new(clarity_core::agent::MockLlm));

        let spec = RunSpec::new("Test", "Do something")
            .with_type("coder")
            .without_git_context();

        let result = manager.run_with_prediction(spec, None).await;
        assert!(result.is_ok());
        assert!(!result.unwrap().monitoring_enabled);
    }
}
