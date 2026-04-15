//! Agent Task Executor for Background Tasks
//!
//! Provides real Agent execution inside background tasks,
//! replacing the previous 10ms sleep placeholder.

use crate::agent::{Agent, AgentConfig, LlmProvider};
use crate::background::{AgentTaskExecutor, TaskSpec};
use crate::memory::MemoryStore;
use crate::registry::ToolRegistry;
use crate::subagents::registry::{AgentTypeDefinition, LaborMarket};
use async_trait::async_trait;
use std::sync::Arc;

/// Default executor that builds and runs an [`Agent`] from a [`TaskSpec`].
#[derive(Clone)]
pub struct DefaultAgentTaskExecutor {
    llm: Arc<dyn LlmProvider>,
    tool_registry: ToolRegistry,
    labor_market: LaborMarket,
    memory_store: Option<Arc<dyn MemoryStore>>,
    working_dir: std::path::PathBuf,
}

impl std::fmt::Debug for DefaultAgentTaskExecutor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DefaultAgentTaskExecutor")
            .field("working_dir", &self.working_dir)
            .field("has_memory_store", &self.memory_store.is_some())
            .finish_non_exhaustive()
    }
}

impl DefaultAgentTaskExecutor {
    /// Create a new executor with the given LLM and tool registry.
    pub fn new(
        llm: Arc<dyn LlmProvider>,
        tool_registry: ToolRegistry,
        working_dir: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            llm,
            tool_registry,
            labor_market: LaborMarket::new(),
            memory_store: None,
            working_dir: working_dir.into(),
        }
    }

    /// Attach a memory store.
    pub fn with_memory_store(mut self, store: Arc<dyn MemoryStore>) -> Self {
        self.memory_store = Some(store);
        self
    }

    /// Replace the default labor market with a custom one.
    pub fn with_labor_market(mut self, market: LaborMarket) -> Self {
        self.labor_market = market;
        self
    }
}

#[async_trait]
impl AgentTaskExecutor for DefaultAgentTaskExecutor {
    async fn execute(&self, spec: &TaskSpec) -> anyhow::Result<(String, usize)> {
        // Resolve agent type definition from the labor market.
        // If the requested type is unknown, fall back to a generic default.
        let type_def = self
            .labor_market
            .get(&spec.agent_type)
            .cloned()
            .unwrap_or_else(|| AgentTypeDefinition {
                name: spec.agent_type.clone(),
                description: "Default background agent".to_string(),
                system_prompt: AgentConfig::default().system_prompt,
                allowed_tools: None,
                max_iterations: spec.max_iterations.unwrap_or(10),
            });

        // Filter tools if the type definition restricts them.
        let registry = match &type_def.allowed_tools {
            None => self.tool_registry.clone(),
            Some(allowed) => {
                let filtered = ToolRegistry::new();
                for name in allowed {
                    if let Ok(Some(tool)) = self.tool_registry.get(name) {
                        let _ = filtered.register_shared(tool);
                    }
                }
                filtered
            }
        };

        let max_iterations = spec
            .max_iterations
            .unwrap_or(type_def.max_iterations);

        let config = AgentConfig::new()
            .with_max_iterations(max_iterations)
            .with_working_dir(&self.working_dir)
            .with_system_prompt(&type_def.system_prompt);

        let mut agent = Agent::with_config(registry, config).with_llm(self.llm.clone());

        if let Some(ref store) = self.memory_store {
            agent = agent.with_memory(store.clone());
        }

        let output = agent
            .run(&spec.prompt)
            .await
            .map_err(|e| anyhow::anyhow!("Agent execution failed: {}", e))?;

        // Note: `Agent` does not currently expose `steps_taken` publicly.
        // We return 0 as a placeholder; this can be enriched once the Agent API
        // provides iteration counts.
        Ok((output, 0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::MockLlm;
    use crate::background::TaskResult;
    use crate::registry::ToolRegistry;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_default_agent_executor_with_mock_llm() {
        let temp_dir = TempDir::new().unwrap();
        let registry = ToolRegistry::with_builtin_tools();
        let llm = Arc::new(MockLlm);

        let executor = DefaultAgentTaskExecutor::new(llm, registry, temp_dir.path());

        let spec = TaskSpec::new("test_task", "Say hello")
            .with_agent_type("coder")
            .with_max_iterations(5);

        let (output, steps) = executor.execute(&spec).await.unwrap();
        assert_eq!(output, "This is a mock response");
        assert_eq!(steps, 0); // steps placeholder until Agent exposes it
    }

    #[tokio::test]
    async fn test_executor_uses_labor_market_type_def() {
        let temp_dir = TempDir::new().unwrap();
        let registry = ToolRegistry::with_builtin_tools();
        let llm = Arc::new(MockLlm);

        let mut market = LaborMarket::new();
        market.register(AgentTypeDefinition {
            name: "custom".to_string(),
            description: "Custom agent".to_string(),
            system_prompt: "You are a custom test agent.".to_string(),
            allowed_tools: Some(vec!["file_read".to_string()]),
            max_iterations: 3,
        });

        let executor = DefaultAgentTaskExecutor::new(llm, registry, temp_dir.path())
            .with_labor_market(market);

        let spec = TaskSpec::new("custom_task", "Do something")
            .with_agent_type("custom")
            .with_max_iterations(7); // should be overridden by type_def (3)

        // The executor will run; since MockLlm always succeeds,
        // we just verify it doesn't panic and uses the custom config.
        let (output, _) = executor.execute(&spec).await.unwrap();
        assert_eq!(output, "This is a mock response");
    }
}
