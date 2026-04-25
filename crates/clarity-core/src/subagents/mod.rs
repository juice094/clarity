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

pub use builder::SubagentBuilder;
pub use parallel::{run_parallel, ParallelConfig, ParallelExecutor, ParallelResult, SubagentBatch};
pub use registry::{AgentTypeDefinition, LaborMarket};
pub use runner::{
    collect_git_context, ExecutionContext, ExecutionStatus, GitContext, OutputCollector, RunSpec,
    SubagentError, SubagentResult, SubagentRunner,
};
pub use store::{SubagentState, SubagentStatus, SubagentStore};
pub use team::{
    AgentTeam, Mailbox, MailboxError, MailboxMessage, MessagePayload, TeamCoordinator, TeamResult,
};
pub use token::{CapabilityToken, TokenError};

use crate::llm::ModelRegistry;
use crate::registry::ToolRegistry;
use std::path::Path;
use std::sync::Arc;

/// 子代理管理器
///
/// 整合所有子代理功能的高级接口。
pub struct SubagentManager {
    /// 存储
    store: SubagentStore,
    /// 执行器
    runner: SubagentRunner,
}

impl SubagentManager {
    /// 创建新的子代理管理器
    pub fn new(
        tool_registry: ToolRegistry,
        working_dir: impl AsRef<Path>,
        context_dir: impl AsRef<Path>,
    ) -> Self {
        let store = SubagentStore::new(&context_dir);
        let runner = SubagentRunner::new(tool_registry, working_dir, context_dir);

        Self { store, runner }
    }

    /// 设置默认 LLM（builder 模式）
    pub fn with_llm(mut self, llm: Arc<dyn crate::agent::LlmProvider>) -> Self {
        self.runner = self.runner.with_llm(llm);
        self
    }

    /// 设置模型注册表（支持 model_override 动态选择）
    pub fn with_registry(mut self, registry: ModelRegistry) -> Self {
        self.runner = self.runner.with_registry(registry);
        self
    }

    /// 运行子代理
    pub async fn run(&mut self, spec: RunSpec) -> Result<SubagentResult, SubagentError> {
        self.runner.run(spec, &mut self.store, None).await
    }

    /// 并行运行多个子代理
    pub async fn run_parallel(
        &mut self,
        specs: Vec<RunSpec>,
        config: ParallelConfig,
    ) -> anyhow::Result<ParallelResult> {
        use crate::background::BackgroundTaskManager;

        let task_manager = BackgroundTaskManager::new(
            self.runner.working_dir().join("tasks"),
            self.runner.working_dir(),
            self.runner.working_dir().join("context"),
        );

        let batch = SubagentBatch::new().add_many(specs).with_config(config);

        let mut executor = ParallelExecutor::new(task_manager, self.runner.clone());
        executor.execute(batch).await
    }

    /// Execute an [`AgentTeam`] and return a unified [`TeamResult`].
    pub async fn run_team(&mut self, team: AgentTeam) -> anyhow::Result<TeamResult> {
        use crate::background::BackgroundTaskManager;

        let task_manager = BackgroundTaskManager::new(
            self.runner.working_dir().join("tasks"),
            self.runner.working_dir(),
            self.runner.working_dir().join("context"),
        );

        let mut coordinator = TeamCoordinator::new(task_manager, self.runner.clone());
        coordinator.execute_team(team).await
    }

    /// 获取存储
    pub fn store(&self) -> &SubagentStore {
        &self.store
    }

    /// 获取存储（可变）
    pub fn store_mut(&mut self) -> &mut SubagentStore {
        &mut self.store
    }

    /// 获取执行器
    pub fn runner(&self) -> &SubagentRunner {
        &self.runner
    }

    /// 列出所有代理状态
    pub fn list_agents(&self) -> Vec<&SubagentState> {
        self.store.list()
    }

    /// 列出正在运行的代理
    pub fn list_running(&self) -> Vec<&SubagentState> {
        self.store.list_by_status(SubagentStatus::Running)
    }

    /// 列出已完成的代理
    pub fn list_completed(&self) -> Vec<&SubagentState> {
        self.store.list_by_status(SubagentStatus::Completed)
    }

    /// 获取代理状态
    pub fn get_agent(&self, agent_id: &str) -> Option<&SubagentState> {
        self.store.get(agent_id)
    }

    /// 删除代理
    pub fn delete_agent(&mut self, agent_id: &str) -> Option<SubagentState> {
        self.store.delete(agent_id)
    }

    /// 列出可用的代理类型
    pub fn list_agent_types(&self) -> Vec<&AgentTypeDefinition> {
        self.runner.labor_market().list()
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

        // 初始状态应该为空
        assert_eq!(manager.list_agents().len(), 0);

        // 列出可用的代理类型
        let types = manager.list_agent_types();
        assert!(types.iter().any(|t| t.name == "coder"));
        assert!(types.iter().any(|t| t.name == "explore"));
        assert!(types.iter().any(|t| t.name == "plan"));
    }

    #[tokio::test]
    async fn test_subagent_error_display() {
        let err = SubagentError::ExecutionFailed {
            message: "Something went wrong".into(),
            brief: "failed".into(),
        };
        assert!(err.to_string().contains("failed"));
        assert!(err.to_string().contains("Something went wrong"));

        let err = SubagentError::MaxStepsReached {
            steps: 10,
            phase: "execution".into(),
        };
        assert!(err.to_string().contains("10"));
        assert!(err.to_string().contains("execution"));
    }

    #[test]
    fn test_parallel_config() {
        let config = ParallelConfig::new()
            .with_max_concurrency(5)
            .with_timeout(600)
            .cancel_on_error();

        assert_eq!(config.max_concurrency, 5);
        assert_eq!(config.timeout_secs, Some(600));
        assert!(config.cancel_on_error);
    }

    #[test]
    fn test_subagent_batch() {
        let batch = SubagentBatch::new()
            .add(RunSpec::new("Task 1", "Do something").with_type("coder"))
            .add(RunSpec::new("Task 2", "Do another").with_type("explore"));

        assert_eq!(batch.len(), 2);
        assert!(!batch.is_empty());
    }
}
