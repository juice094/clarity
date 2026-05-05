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
        let (mut messages, tools, llm, cancel_token) =
            self.prepare_sync_turn(query.as_ref()).await?;
        info!("Starting agent loop for query: {}", query.as_ref());
        self.send_wire_message(WireMessage::TurnBegin {
            user_input: query.as_ref().to_string(),
        });
        let (final_response, completed, tool_names) = self
            .run_sync_loop(&mut messages, &tools, llm, &cancel_token)
            .await?;
        self.finalize_sync_turn(query.as_ref(), final_response, completed, &tool_names, &messages)
            .await
    }

    /// Run a synchronous agent loop with pre-built messages.
    pub async fn run_with_messages_sync(
        &self,
        mut messages: Vec<Message>,
    ) -> Result<String, AgentError> {
        self.ensure_initialized().await?;
        let (tools, llm, cancel_token) = self.setup_turn().await?;
        let (final_response, completed, tool_names) = self
            .run_sync_loop(&mut messages, &tools, llm, &cancel_token)
            .await?;
        self.finish_sync_turn(final_response, completed, &tool_names).await
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
        self.inner.read().unwrap().approval_mode == ApprovalMode::Plan
    }

    async fn prepare_sync_turn(
        &self,
        query: &str,
    ) -> Result<(Vec<Message>, Value, Arc<dyn crate::llm::api::LlmProvider>, CancellationToken), AgentError>
    {
        let cancel_token = self.begin_turn()?;
        self.activate_skills();
        let llm = self.llm().ok_or(AgentError::Unconfigured)?;
        let tools = self.filter_tools_value(&self.registry.get_tool_schemas()?);
        let messages = self.build_messages_with_cache(query).await?;
        Ok((messages, tools, llm, cancel_token))
    }

    async fn setup_turn(
        &self,
    ) -> Result<(Value, Arc<dyn crate::llm::api::LlmProvider>, CancellationToken), AgentError> {
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
