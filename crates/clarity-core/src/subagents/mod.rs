//! Subagent management system
//!
//! 提供子代理的完整生命周期管理，包括：
//! - **LaborMarket**: 子代理类型注册表
//! - **SubagentStore**: 状态存储
//! - **SubagentBuilder**: 构建器
//! - **SubagentRunner**: 执行器（新增）
//!
//! 设计参考 std::process::Command 的构建器模式。

pub mod builder;
pub mod registry;
pub mod runner;
mod store;

pub use builder::SubagentBuilder;
pub use registry::{AgentTypeDefinition, LaborMarket};
pub use runner::{
    collect_git_context, ExecutionContext, ExecutionStatus, GitContext, OutputCollector,
    RunSpec, SubagentError, SubagentResult, SubagentRunner,
};
pub use store::{SubagentState, SubagentStatus, SubagentStore};

use crate::registry::ToolRegistry;
use std::path::Path;

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

    /// 运行子代理
    pub async fn run(
        &mut self,
        spec: RunSpec,
    ) -> Result<SubagentResult, SubagentError> {
        self.runner.run(spec, &mut self.store, None).await
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
}
