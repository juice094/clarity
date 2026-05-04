//! Agent execution loops: synchronous, streaming, and shared core.

mod dispatch;
mod loop_helpers;
mod loop_streaming;
mod loop_sync;

use crate::agent::Agent;
use crate::error::AgentError;
use crate::llm::api::{Message, MessageRole};
use clarity_wire::WireMessage;
use tracing::{info, warn};

use loop_helpers::*;

impl Agent {
    /// Main entry point: run the agent with a user query.
    pub async fn run(&self, query: impl AsRef<str>) -> Result<String, AgentError> {
        self.ensure_initialized().await?;

        let mode = self.inner.read().unwrap().approval_mode;
        if mode == crate::approval::ApprovalMode::Plan {
            let plan = self.plan(query.as_ref()).await?;
            self.send_wire_message(WireMessage::TurnBegin {
                user_input: query.as_ref().to_string(),
            });
            if !plan.is_empty() {
                self.send_wire_message(WireMessage::ContentPart {
                    text: format!("📋 Executing plan: {}\n{}", plan.title, plan.to_markdown()),
                });
            }
            let results = self.execute_plan(&plan).await?;
            let final_response = format_plan_results(&results);
            self.send_wire_message(WireMessage::TurnEnd);
            return Ok(final_response);
        }

        let cancel_token = self.begin_turn()?;
        if let Some(ref registry) = self.skill_registry() {
            registry.discover_for_path(&self.config.working_dir);
            let paths = self.active_file_paths();
            if !paths.is_empty() {
                registry.activate_by_path(&paths);
            }
        }
        let llm = self.llm().ok_or(AgentError::Unconfigured)?;
        let tools = self.filter_tools_value(&self.registry.get_tool_schemas()?);
        let system_prompt = self.build_system_prompt_with_memory(query.as_ref()).await;
        let mut messages = vec![Message::system(system_prompt), Message::user(query.as_ref())];

        info!("Starting agent loop for query: {}", query.as_ref());
        self.send_wire_message(WireMessage::TurnBegin {
            user_input: query.as_ref().to_string(),
        });

        let (final_response, completed, tool_names) = self
            .run_sync_loop(&mut messages, &tools, llm, &cancel_token)
            .await?;
        let usage = self.get_session_usage();
        let final_response = self.finish_and_deliver(final_response, &tool_names, usage).await?;
        self.persist_turn_memory(query.as_ref(), &final_response, completed).await;

        if completed {
            let transcript = serde_json::to_string(&messages).unwrap_or_default();
            self.maybe_extract_memories(transcript);
            if let Some(ref hooks) = self.hook_registry {
                let summary = serde_json::json!({
                    "query": query.as_ref(),
                    "response": &final_response,
                    "completed": true,
                });
                hooks.run_session_termination(&summary.to_string()).await;
            }
            Ok(final_response)
        } else {
            warn!("Max iterations ({}) reached", self.config.max_iterations);
            Err(AgentError::MaxIterationsExceeded(self.config.max_iterations))
        }
    }

    /// Run a synchronous agent loop with pre-built messages.
    pub async fn run_with_messages_sync(
        &self,
        mut messages: Vec<Message>,
    ) -> Result<String, AgentError> {
        self.ensure_initialized().await?;
        let cancel_token = self.begin_turn()?;
        if let Some(ref registry) = self.skill_registry() {
            registry.discover_for_path(&self.config.working_dir);
            let paths = self.active_file_paths();
            if !paths.is_empty() {
                registry.activate_by_path(&paths);
            }
        }
        let llm = self.llm().ok_or(AgentError::Unconfigured)?;
        let tools = self.filter_tools_value(&self.registry.get_tool_schemas()?);
        let (final_response, completed, tool_names) = self
            .run_sync_loop(&mut messages, &tools, llm, &cancel_token)
            .await?;
        let usage = self.get_session_usage();
        let final_response = self.finish_and_deliver(final_response, &tool_names, usage).await?;
        if completed {
            Ok(final_response)
        } else {
            warn!("Max iterations ({}) reached", self.config.max_iterations);
            Err(AgentError::MaxIterationsExceeded(self.config.max_iterations))
        }
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
        let system_prompt = self.build_system_prompt_with_memory(query.as_ref()).await;
        let messages = vec![Message::system(system_prompt), Message::user(query.as_ref())];
        self.run_streaming_turn(messages, query.as_ref(), on_chunk).await
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
        self.run_streaming_turn(messages, &query_hint, on_chunk).await
    }
}
