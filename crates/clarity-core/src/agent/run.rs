//! Agent execution loops: synchronous, streaming, and shared core.

mod dispatch;
mod loop_helpers;
mod loop_steps;
mod loop_streaming;
mod loop_sync;
mod loop_trait;

pub(crate) use loop_helpers::format_plan_results;

use crate::agent::Agent;
use crate::approval::ApprovalMode;
use crate::error::AgentError;
use crate::llm::api::{Message, MessageRole};
use crate::registry::ToolRegistry;
use clarity_wire::WireMessage;
use serde_json::Value;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use tracing::info;

impl Agent {
    /// Main entry point: run the agent with a user query.
    pub async fn run(&self, query: impl AsRef<str>) -> Result<String, AgentError> {
        self.ensure_initialized().await?;
        if self.is_plan_mode() {
            return self.execute_plan_mode(query.as_ref()).await;
        }
        if self.config.enable_jumpy {
            return self.execute_jumpy_mode(query.as_ref()).await;
        }
        let (mut messages, tools, llm, cancel_token) =
            self.prepare_sync_turn(query.as_ref()).await?;

        self.maybe_snapshot_pre_turn().await;

        info!("Starting agent loop for query: {}", query.as_ref());
        self.send_wire_message(WireMessage::TurnBegin {
            user_input: query.as_ref().to_string(),
        });
        let (final_response, completed, tool_names) = self
            .run_sync_loop(&mut messages, &tools, llm, &cancel_token)
            .await?;

        self.maybe_snapshot_post_turn().await;

        {
            let mut inner = self.inner.write();
            inner.last_turn_message_count = messages.len();
        }
        self.finalize_sync_turn(
            query.as_ref(),
            final_response,
            completed,
            &tool_names,
            &messages,
        )
        .await
    }

    /// Jumpy World Model execution path.
    /// Uses SkillComposer to plan and execute skill sequences instead of the
    /// standard turn-based LLM loop.
    async fn execute_jumpy_mode(&self, query: &str) -> Result<String, AgentError> {
        use crate::agent::jumpy::{
            Goal, HierarchicalPlanner, JumpyState, PlannerConfig, SkillComposer,
        };

        // 1. Build predictor (user-provided or auto-wrapped via LlmAdapter)
        let predictor: Arc<dyn crate::agent::jumpy::OutcomePredictor> = {
            let inner = self.inner.read();
            if let Some(ref p) = inner.jumpy_predictor {
                p.clone()
            } else if let Some(ref llm) = inner.llm {
                let adapted = crate::agent::jumpy::LlmAdapter::new(llm.clone());
                Arc::new(crate::agent::jumpy::LlmAugmentedPredictor::new(Arc::new(
                    adapted,
                )))
            } else {
                return Err(AgentError::Llm(
                    "No LLM provider configured for Jumpy mode".into(),
                ));
            }
        };

        // 2. Build SkillComposer with available tools from registry
        let composer_config = self.config.jumpy_config.clone().unwrap_or_default();
        let available_skills = self
            .registry
            .list_tools()
            .map_err(|e| AgentError::Registry(format!("Failed to list tools: {}", e)))?
            .into_iter()
            .map(|name| (name, "{}".to_string()))
            .collect();
        let planner_config = PlannerConfig {
            available_skills,
            ..PlannerConfig::default()
        };
        let planner = HierarchicalPlanner::new(predictor, planner_config);
        let mut composer = SkillComposer::new(planner, composer_config);

        // 3. Goal & initial state
        let goal = Goal::new(vec![query.to_string()]);
        let initial_state = JumpyState::from_query(query);

        // 4. Execute with tool-backed skill function
        let registry = self.registry.clone();
        let working_dir = self.config.working_dir.clone();
        let read_only = self.config.read_only;
        let result = composer
            .compose(&goal, &initial_state, |skill_id, params| {
                let skill_id = skill_id.to_string();
                let params = params.to_string();
                let registry = registry.clone();
                let working_dir = working_dir.clone();
                async move {
                    Self::execute_skill_for_jumpy(
                        &registry,
                        &skill_id,
                        &params,
                        &working_dir,
                        read_only,
                    )
                    .await
                }
            })
            .await;

        // 5. Format result
        match result {
            Ok(composition_result) => {
                let summary = Self::format_jumpy_result(&composition_result);
                Ok(summary)
            }
            Err(e) => Err(AgentError::Llm(e)),
        }
    }

    /// Execute a single skill via ToolRegistry and convert the outcome to JumpyState.
    async fn execute_skill_for_jumpy(
        registry: &ToolRegistry,
        skill_id: &str,
        params: &str,
        working_dir: &std::path::Path,
        read_only: bool,
    ) -> Result<crate::agent::jumpy::JumpyState, String> {
        use crate::agent::jumpy::JumpyState;
        use crate::tools::ToolContext;

        let tool = registry
            .get(skill_id)
            .map_err(|e| format!("Registry error: {}", e))?
            .ok_or_else(|| format!("Tool '{}' not found in registry", skill_id))?;

        let args: Value = serde_json::from_str(params)
            .map_err(|e| format!("Invalid JSON params for {}: {}", skill_id, e))?;

        let ctx = ToolContext {
            working_dir: working_dir.to_path_buf(),
            env: std::collections::HashMap::new(),
            timeout_secs: 60,
            max_output_size: 1024 * 1024,
            read_only,
            approval_mode: crate::approval::ApprovalMode::Interactive,
            capability_token: None,
        };

        let result = tool.execute(args, ctx).await;

        let mut state = JumpyState::default();
        match result {
            Ok(value) => {
                state.tags.push("success".to_string());
                state.memory.insert("result".to_string(), value.to_string());
                state.progress = 0.5;
                state.context_summary = format!("Executed {} successfully", skill_id);
                // Heuristic: if result contains file paths, add to active_files
                if let Some(paths) = value.get("paths").and_then(|v: &Value| v.as_array()) {
                    for p in paths {
                        if let Some(s) = p.as_str() {
                            state.active_files.push(s.to_string());
                        }
                    }
                }
            }
            Err(e) => {
                state.tags.push("error".to_string());
                state.memory.insert("error".to_string(), e.to_string());
                state.progress = 0.0;
                state.context_summary = format!("{} failed: {}", skill_id, e);
            }
        }
        Ok(state)
    }

    /// Format a CompositionResult into a human-readable summary.
    fn format_jumpy_result(result: &crate::agent::jumpy::CompositionResult) -> String {
        let mut lines = vec![];
        lines.push(format!(
            "Jumpy execution complete: {} step(s)",
            result.total_steps
        ));
        for (i, step) in result.steps.iter().enumerate() {
            lines.push(format!(
                "  Step {}: {}({}) -> deviation={:.2}{}",
                i + 1,
                step.skill_id,
                step.params,
                step.deviation,
                if step.replanned { " [replanned]" } else { "" }
            ));
        }
        lines.push(format!("Final state tags: {:?}", result.final_state.tags));
        lines.push(format!(
            "Final state progress: {:.2}",
            result.final_state.progress
        ));
        lines.push(format!("Success: {}", result.success));
        lines.join("\n")
    }

    /// Run a synchronous agent loop with pre-built messages.
    pub async fn run_with_messages_sync(
        &self,
        mut messages: Vec<Message>,
    ) -> Result<String, AgentError> {
        self.ensure_initialized().await?;
        let (tools, llm, cancel_token) = self.setup_turn().await?;
        self.maybe_snapshot_pre_turn().await;
        let (final_response, completed, tool_names) = self
            .run_sync_loop(&mut messages, &tools, llm, &cancel_token)
            .await?;
        self.maybe_snapshot_post_turn().await;
        self.finish_sync_turn(final_response, completed, &tool_names)
            .await
    }

    /// Run the agent with streaming response.
    pub async fn run_streaming<F>(
        &self,
        query: impl AsRef<str>,
        on_chunk: F,
    ) -> Result<String, AgentError>
    where
        F: FnMut(&str) + Send + 'static,
    {
        self.ensure_initialized().await?;
        let messages = self.build_messages_with_cache(query.as_ref()).await?;
        self.run_streaming_turn(messages, query.as_ref(), on_chunk)
            .await
    }

    /// Run the streaming agent loop with a pre-built message list.
    pub(crate) async fn run_streaming_with_messages<F>(
        &self,
        messages: Vec<Message>,
        on_chunk: F,
    ) -> Result<String, AgentError>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let query_hint = messages
            .iter()
            .rev()
            .find(|m| m.role == MessageRole::User)
            .map(|m| m.content.clone())
            .unwrap_or_default();
        self.run_streaming_turn(messages, &query_hint, on_chunk)
            .await
    }

    // ------------------------------------------------------------------
    // Private turn lifecycle helpers
    // ------------------------------------------------------------------

    fn is_plan_mode(&self) -> bool {
        self.inner.read().approval_mode == ApprovalMode::Plan
    }

    async fn prepare_sync_turn(
        &self,
        query: &str,
    ) -> Result<
        (
            Vec<Message>,
            Value,
            Arc<dyn crate::llm::api::LlmProvider>,
            CancellationToken,
        ),
        AgentError,
    > {
        let cancel_token = self.begin_turn()?;
        self.activate_skills();
        let llm = self.llm().ok_or(AgentError::Unconfigured)?;
        let tools = self.filter_tools_value(&self.registry.get_tool_schemas()?);
        let messages = self.build_messages_with_cache(query).await?;
        Ok((messages, tools, llm, cancel_token))
    }

    async fn setup_turn(
        &self,
    ) -> Result<
        (
            Value,
            Arc<dyn crate::llm::api::LlmProvider>,
            CancellationToken,
        ),
        AgentError,
    > {
        let cancel_token = self.begin_turn()?;
        self.activate_skills();
        let llm = self.llm().ok_or(AgentError::Unconfigured)?;
        let tools = self.filter_tools_value(&self.registry.get_tool_schemas()?);
        Ok((tools, llm, cancel_token))
    }

    fn activate_skills(&self) {
        if let Some(ref registry) = self.skill_registry() {
            registry.discover_for_path(&self.config.working_dir);
            let paths = self.active_file_paths();
            if !paths.is_empty() {
                registry.activate_by_path(&paths);
            }
        }
    }
}
