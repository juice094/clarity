//! Subagent builder
//!
//! Builds Agent instances for subagents.

use crate::store::SubagentStore;
use clarity_contract::subagent::{AgentTypeDefinition, CapabilityToken, LaborMarket};
use clarity_contract::tool::ToolRegistry as ToolRegistryTrait;
use clarity_core::agent::{Agent, AgentConfig};
use clarity_core::registry::ToolRegistry;
use clarity_llm::api::Message;

/// Builder for subagent instances
pub struct SubagentBuilder {
    labor_market: LaborMarket,
    tool_registry: ToolRegistry,
    parent_working_dir: std::path::PathBuf,
    git_context: Option<String>,
    capability_token: Option<CapabilityToken>,
    iteration_budget: Option<std::sync::Arc<std::sync::atomic::AtomicUsize>>,
    /// Force read-only mode regardless of the agent type definition.
    read_only: bool,
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
            capability_token: None,
            iteration_budget: None,
            read_only: false,
        }
    }

    /// Attach an optional Git context string to prepend to the system prompt
    pub fn with_git_context(mut self, git_context: Option<String>) -> Self {
        self.git_context = git_context;
        self
    }

    /// Attach a capability token for permission isolation
    pub fn with_capability_token(mut self, token: CapabilityToken) -> Self {
        self.capability_token = Some(token);
        self
    }

    /// Attach an iteration budget to share with the subagent.
    pub fn with_iteration_budget(
        mut self,
        budget: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    ) -> Self {
        self.iteration_budget = Some(budget);
        self
    }

    /// Force read-only mode for this build.
    pub fn with_read_only(mut self, read_only: bool) -> Self {
        self.read_only = read_only;
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

        // Filter tools based on allowed_tools and read-only restriction.
        let effective_read_only = self.read_only || type_def.read_only;
        let filtered_registry = self.filter_tools(&type_def.allowed_tools, effective_read_only);

        // Build system prompt, optionally prepending Git context
        let system_prompt = if let Some(git_ctx) = &self.git_context {
            format!("{}\n\n{}", git_ctx, type_def.system_prompt)
        } else {
            type_def.system_prompt.clone()
        };

        // Determine working directory (sandbox takes precedence)
        let working_dir = if let Some(ref token) = self.capability_token {
            if let Some(ref sandbox) = token.sandbox_dir {
                if sandbox.is_absolute() {
                    sandbox.clone()
                } else {
                    self.parent_working_dir.join(sandbox)
                }
            } else {
                self.parent_working_dir.clone()
            }
        } else {
            self.parent_working_dir.clone()
        };

        // Build agent config
        let max_iterations = if let Some(ref token) = self.capability_token {
            token.max_iterations.unwrap_or(type_def.max_iterations)
        } else {
            type_def.max_iterations
        };

        let mut config = AgentConfig::new()
            .with_max_iterations(max_iterations)
            .with_working_dir(&working_dir)
            .with_system_prompt(&system_prompt)
            .with_capability_token(self.capability_token.clone());

        if let Some(ref budget) = self.iteration_budget {
            config = config.with_iteration_budget(budget.clone());
        }

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
        let type_def = self.labor_market.require(type_name)?;
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

    /// Filter tools based on allowed list and read-only restriction.
    fn filter_tools(&self, allowed: &Option<Vec<String>>, read_only: bool) -> ToolRegistry {
        let candidate_names: Vec<String> = match allowed {
            None => self
                .tool_registry
                .list()
                .unwrap_or_default()
                .iter()
                .map(|t| t.to_string())
                .collect(),
            Some(allowed_tools) => allowed_tools.clone(),
        };

        let filtered = ToolRegistry::new();
        for name in candidate_names {
            if read_only && is_mutating_tool(&name) {
                continue;
            }
            if let Ok(Some(tool)) = self.tool_registry.get(&name) {
                let _ = filtered.register_shared(tool);
            }
        }
        filtered
    }

    /// Get labor market reference
    pub fn labor_market(&self) -> &LaborMarket {
        &self.labor_market
    }
}

/// Returns true for tools that may modify files or execute arbitrary commands.
///
/// ponytail: hard-coded name list. Long-term this should come from tool metadata
/// (e.g. `Tool::category()`) rather than string matching.
fn is_mutating_tool(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "file_write" | "file_edit" | "bash" | "shell" | "cmd" | "write_file" | "edit_file"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::subagent::SubagentStatus;

    fn create_test_registry() -> ToolRegistry {
        ToolRegistry::with_builtin_tools()
    }

    #[test]
    fn test_build_subagent() {
        let registry = create_test_registry();
        let builder = SubagentBuilder::new(registry, "/tmp");
        let mut store = SubagentStore::new("/tmp/store");

        let type_def = builder
            .labor_market()
            .require("coder")
            .expect("coder type exists");
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
    fn test_build_unknown_type() {
        let registry = create_test_registry();
        let builder = SubagentBuilder::new(registry, "/tmp");
        let mut store = SubagentStore::new("/tmp/store");

        let result = builder.build_by_type("test-1", "unknown", &mut store);
        assert!(result.is_err());
    }

    #[test]
    fn test_read_only_filter_excludes_mutating_tools() {
        let registry = create_test_registry();
        let builder = SubagentBuilder::new(registry, "/tmp");

        let filtered = builder.filter_tools(
            &Some(vec![
                "file_read".to_string(),
                "file_write".to_string(),
                "bash".to_string(),
            ]),
            true,
        );

        assert!(filtered.get("file_read").unwrap().is_some());
        assert!(filtered.get("file_write").unwrap().is_none());
        assert!(filtered.get("bash").unwrap().is_none());
    }

    #[test]
    fn test_read_only_filter_applies_to_full_registry_when_allowed_tools_none() {
        let registry = create_test_registry();
        let builder = SubagentBuilder::new(registry, "/tmp");

        let filtered = builder.filter_tools(&None, true);

        assert!(filtered.get("file_read").unwrap().is_some());
        assert!(filtered.get("file_write").unwrap().is_none());
        assert!(filtered.get("bash").unwrap().is_none());
    }

    #[test]
    fn test_non_read_only_filter_keeps_mutating_tools() {
        let registry = create_test_registry();
        let builder = SubagentBuilder::new(registry, "/tmp");

        let filtered = builder.filter_tools(
            &Some(vec!["file_read".to_string(), "file_write".to_string()]),
            false,
        );

        assert!(filtered.get("file_read").unwrap().is_some());
        assert!(filtered.get("file_write").unwrap().is_some());
    }

    #[test]
    fn test_is_mutating_tool_recognizes_variants() {
        assert!(is_mutating_tool("file_write"));
        assert!(is_mutating_tool("File_Write"));
        assert!(is_mutating_tool("bash"));
        assert!(is_mutating_tool("shell"));
        assert!(!is_mutating_tool("file_read"));
        assert!(!is_mutating_tool("glob"));
    }
}
