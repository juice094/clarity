//! Test helpers shared by `clarity-subagents` unit tests.
//!
//! These fakes replace the former dependency on `clarity-core` concrete types
//! now that `clarity-subagents` consumes only contract traits.

#![cfg(test)]

use clarity_contract::error::{AgentError, ToolError};
use clarity_contract::llm::{LlmProvider, LlmResponse};
use clarity_contract::subagent::{
    AgentBuilder, AgentBuilderFactory, AgentExecutor, SubagentBuildSpec,
};
use clarity_contract::task::{TaskId, TaskManager, TaskResult, TaskSpec, TaskStatus};
use clarity_contract::tool::{SharedTool, ToolContext, ToolRegistry};
use clarity_contract::{Message, StreamDelta};
use std::collections::HashMap;
use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// Tool registry stub that advertises a single tool and never returns one.
#[derive(Default)]
pub struct FakeToolRegistry;

#[async_trait::async_trait]
impl ToolRegistry for FakeToolRegistry {
    fn get(&self, _name: &str) -> Result<Option<SharedTool>, AgentError> {
        Ok(None)
    }

    async fn execute(
        &self,
        _name: &str,
        _args: serde_json::Value,
        _ctx: ToolContext,
    ) -> Result<serde_json::Value, ToolError> {
        Err(ToolError::NotFound("fake".into()))
    }

    fn list(&self) -> Result<Vec<String>, AgentError> {
        Ok(vec!["file_read".to_string()])
    }

    fn contains(&self, _name: &str) -> Result<bool, AgentError> {
        Ok(false)
    }

    fn len(&self) -> Result<usize, AgentError> {
        Ok(1)
    }
}

/// In-memory task manager that runs tasks synchronously inside `spawn`.
#[derive(Default)]
pub struct FakeTaskManager {
    counter: AtomicUsize,
    results: Mutex<HashMap<TaskId, TaskResult>>,
}

#[async_trait::async_trait]
impl TaskManager for FakeTaskManager {
    async fn spawn(
        &self,
        _spec: TaskSpec,
        task: Pin<Box<dyn Future<Output = anyhow::Result<TaskResult>> + Send>>,
    ) -> anyhow::Result<TaskId> {
        let id = format!("task_{}", self.counter.fetch_add(1, Ordering::SeqCst));
        let result = task
            .await
            .unwrap_or_else(|e| TaskResult::failed(e.to_string()));
        self.results.lock().unwrap().insert(id.clone(), result);
        Ok(id)
    }

    async fn wait(&self, task_id: &TaskId) -> anyhow::Result<TaskResult> {
        Ok(self
            .results
            .lock()
            .unwrap()
            .get(task_id)
            .cloned()
            .unwrap_or_else(|| TaskResult::failed("task not found")))
    }

    async fn cancel(&self, _task_id: &TaskId) -> anyhow::Result<()> {
        Ok(())
    }

    async fn status(&self, _task_id: &TaskId) -> anyhow::Result<TaskStatus> {
        Ok(TaskStatus::Completed)
    }
}

/// Factory that builds fake agents from a `SubagentBuildSpec`.
pub struct FakeAgentBuilderFactory;

impl AgentBuilderFactory for FakeAgentBuilderFactory {
    fn create_builder(
        &self,
        _tool_registry: Arc<dyn ToolRegistry>,
        spec: SubagentBuildSpec,
    ) -> Box<dyn AgentBuilder> {
        Box::new(FakeAgentBuilder { spec })
    }
}

pub struct FakeAgentBuilder {
    spec: SubagentBuildSpec,
}

impl AgentBuilder for FakeAgentBuilder {
    fn with_llm(self: Box<Self>, _llm: Arc<dyn LlmProvider>) -> Box<dyn AgentBuilder> {
        self
    }

    fn with_approval_runtime(
        self: Box<Self>,
        _runtime: Arc<dyn clarity_contract::approval::ApprovalRuntime>,
    ) -> Box<dyn AgentBuilder> {
        self
    }

    fn with_approval_mode(
        self: Box<Self>,
        _mode: clarity_contract::tool::ApprovalMode,
    ) -> Box<dyn AgentBuilder> {
        self
    }

    fn set_llm(&mut self, _llm: Arc<dyn LlmProvider>) {}

    fn build(self: Box<Self>) -> Box<dyn AgentExecutor> {
        Box::new(FakeAgentExecutor {
            working_dir: self.spec.working_dir.clone(),
            budget: self.spec.iteration_budget.clone(),
        })
    }
}

pub struct FakeAgentExecutor {
    working_dir: PathBuf,
    budget: Option<Arc<AtomicUsize>>,
}

#[async_trait::async_trait]
impl AgentExecutor for FakeAgentExecutor {
    async fn run_turn(&self, _query: &str) -> Result<String, AgentError> {
        if let Some(budget) = &self.budget {
            let remaining = budget.fetch_sub(1, Ordering::SeqCst);
            if remaining == 0 {
                return Err(AgentError::MaxIterationsExceeded(0));
            }
        }
        Ok(format!(
            "This is a sufficiently long mock response from the fake agent running in {:?}.",
            self.working_dir
        ))
    }

    fn last_turn_message_count(&self) -> usize {
        0
    }
}

/// Minimal LLM provider that returns a one-shot stream and a complete response.
pub struct MockLlm;

#[async_trait::async_trait]
impl LlmProvider for MockLlm {
    async fn complete(
        &self,
        _messages: &[Message],
        _tools: &serde_json::Value,
    ) -> Result<LlmResponse, AgentError> {
        Ok(LlmResponse {
            content: "mock".to_string(),
            tool_calls: vec![],
            is_complete: true,
        })
    }

    fn stream(
        &self,
        _messages: &[Message],
        _tools: &serde_json::Value,
    ) -> Result<tokio::sync::mpsc::Receiver<Result<StreamDelta, AgentError>>, AgentError> {
        let (tx, rx) = tokio::sync::mpsc::channel(1);
        let _ = tx.try_send(Ok(StreamDelta {
            content: Some("mock".to_string()),
            reasoning_content: None,
            tool_calls: Vec::new(),
            partial_tool_calls: Vec::new(),
        }));
        Ok(rx)
    }

    fn set_prompt_cache_key(&self, _key: &str) {}
}
