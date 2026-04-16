//! Subagent builder
//!
//! Builds Agent instances for subagents.

use crate::agent::{Agent, AgentConfig, Message};
use crate::registry::ToolRegistry;
use crate::subagents::registry::{AgentTypeDefinition, LaborMarket};
use crate::subagents::store::SubagentStore;

/// Builder for subagent instances
pub struct SubagentBuilder {
    labor_market: LaborMarket,
    tool_registry: ToolRegistry,
    parent_working_dir: std::path::PathBuf,
    git_context: Option<String>,
}

impl SubagentBuilder {
    /// Create new builder
    pub fn new(
        tool_registry: ToolRegistry,
        parent_working_dir: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            labor_market: LaborMarket::new(),
            tool_registry,
            parent_working_dir: parent_working_dir.into(),
            git_context: None,
        }
    }

    /// Attach an optional Git context string to prepend to the system prompt
    pub fn with_git_context(mut self, git_context: Option<String>) -> Self {
        self.git_context = git_context;
        self
    }

    /// Build a subagent from type definition
    pub fn build(
        &self,
        agent_id: &str,
        type_def: &AgentTypeDefinition,
        store: &mut SubagentStore,
    ) -> anyhow::Result<Agent> {
        // Create state in store
        store.create(agent_id.to_string(), type_def.name.clone());

        // Filter tools based on allowed_tools
        let filtered_registry = self.filter_tools(&type_def.allowed_tools)?;

        // Build system prompt, optionally prepending Git context
        let system_prompt = if let Some(git_ctx) = &self.git_context {
            format!("{}\n\n{}", git_ctx, type_def.system_prompt)
        } else {
            type_def.system_prompt.clone()
        };

        // Build agent config
        let config = AgentConfig::new()
            .with_max_iterations(type_def.max_iterations)
            .with_working_dir(&self.parent_working_dir)
            .with_system_prompt(&system_prompt);

        let agent = Agent::with_config(filtered_registry, config);

        Ok(agent)
    }

    /// Build from type name (convenience method)
    pub fn build_by_type(
        &self,
        agent_id: &str,
        type_name: &str,
        store: &mut SubagentStore,
    ) -> anyhow::Result<Agent> {
        let type_def = self.labor_market.require(type_name);
        self.build(agent_id, type_def, store)
    }

    /// Build with existing conversation context
    pub fn build_with_context(
        &self,
        agent_id: &str,
        type_def: &AgentTypeDefinition,
        store: &mut SubagentStore,
        _parent_context: &[Message], // 继承的上下文
    ) -> anyhow::Result<Agent> {
        let agent = self.build(agent_id, type_def, store)?;

        // Note: To actually set context, we would need Agent to support
        // setting initial messages. For now, we just build the agent.
        // The context can be passed as part of the first query.

        Ok(agent)
    }

    /// Filter tools based on allowed list
    fn filter_tools(&self, allowed: &Option<Vec<String>>) -> anyhow::Result<ToolRegistry> {
        match allowed {
            None => Ok(self.tool_registry.clone()),
            Some(allowed_tools) => {
                let filtered = ToolRegistry::new();
                for name in allowed_tools {
                    if let Ok(Some(tool)) = self.tool_registry.get(name) {
                        let _ = filtered.register_shared(tool);
                    }
                }
                Ok(filtered)
            }
        }
    }

    /// Get labor market reference
    pub fn labor_market(&self) -> &LaborMarket {
        &self.labor_market
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subagents::store::SubagentStatus;

    fn create_test_registry() -> ToolRegistry {
        ToolRegistry::with_builtin_tools()
    }

    #[test]
    fn test_build_subagent() {
        let registry = create_test_registry();
        let builder = SubagentBuilder::new(registry, "/tmp");
        let mut store = SubagentStore::new("/tmp/store");

        let type_def = builder.labor_market().require("coder");
        let agent = builder.build("test-1", type_def, &mut store);

        assert!(agent.is_ok());

        // Verify state was created
        let state = store.get("test-1").unwrap();
        assert_eq!(state.agent_type, "coder");
        assert_eq!(state.status, SubagentStatus::Idle);
    }

    #[test]
    fn test_build_by_type() {
        let registry = create_test_registry();
        let builder = SubagentBuilder::new(registry, "/tmp");
        let mut store = SubagentStore::new("/tmp/store");

        let agent = builder.build_by_type("test-1", "explore", &mut store);

        assert!(agent.is_ok());

        let state = store.get("test-1").unwrap();
        assert_eq!(state.agent_type, "explore");
    }

    #[test]
    #[should_panic(expected = "Unknown agent type")]
    fn test_build_unknown_type() {
        let registry = create_test_registry();
        let builder = SubagentBuilder::new(registry, "/tmp");
        let mut store = SubagentStore::new("/tmp/store");

        let _result = builder.build_by_type("test-1", "unknown", &mut store);
    }
}
