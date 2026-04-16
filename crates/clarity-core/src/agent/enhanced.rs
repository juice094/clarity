//! Enhanced Agent Features
//!
//! Provides advanced capabilities for the agent:
//! - Error recovery with retry and fallback
//! - Execution tracing and metrics
//! - Conversation state persistence
//! - Parallel tool execution

use super::{AgentError, Message, ToolCall};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Execution trace for a single step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionStep {
    /// Step number
    pub step_number: usize,
    /// Step type
    pub step_type: StepType,
    /// Start time
    pub start_time: chrono::DateTime<chrono::Utc>,
    /// Duration
    pub duration_ms: u64,
    /// Input (truncated for large data)
    pub input_summary: String,
    /// Output (truncated for large data)
    pub output_summary: String,
    /// Error if any
    pub error: Option<String>,
    /// Token usage if applicable
    pub token_usage: Option<TokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum StepType {
    LlmRequest,
    LlmResponse,
    ToolExecution,
    ToolResult,
    ErrorRecovery,
    StateSave,
    StateLoad,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Execution tracer for monitoring agent performance
#[derive(Debug, Clone)]
pub struct ExecutionTracer {
    steps: Arc<RwLock<Vec<ExecutionStep>>>,
    session_id: String,
    start_time: Instant,
}

impl ExecutionTracer {
    /// Create a new execution tracer
    pub fn new() -> Self {
        Self {
            steps: Arc::new(RwLock::new(Vec::new())),
            session_id: uuid::Uuid::new_v4().to_string(),
            start_time: Instant::now(),
        }
    }

    /// Record a step
    pub async fn record_step(
        &self,
        step_number: usize,
        step_type: StepType,
        input: impl Into<String>,
        output: impl Into<String>,
        duration: Duration,
        token_usage: Option<TokenUsage>,
    ) {
        let step = ExecutionStep {
            step_number,
            step_type,
            start_time: chrono::Utc::now(),
            duration_ms: duration.as_millis() as u64,
            input_summary: truncate_string(&input.into(), 200),
            output_summary: truncate_string(&output.into(), 200),
            error: None,
            token_usage,
        };

        let mut steps = self.steps.write().await;
        steps.push(step);
    }

    /// Record an error
    pub async fn record_error(&self, step_number: usize, error: &str) {
        let mut steps = self.steps.write().await;
        if let Some(step) = steps.iter_mut().find(|s| s.step_number == step_number) {
            step.error = Some(error.to_string());
        }
    }

    /// Get all steps
    pub async fn get_steps(&self) -> Vec<ExecutionStep> {
        self.steps.read().await.clone()
    }

    /// Get session summary
    pub async fn get_summary(&self) -> ExecutionSummary {
        let steps = self.steps.read().await;
        let total_duration = self.start_time.elapsed();

        let llm_requests = steps
            .iter()
            .filter(|s| s.step_type == StepType::LlmRequest)
            .count();
        let tool_executions = steps
            .iter()
            .filter(|s| s.step_type == StepType::ToolExecution)
            .count();
        let errors = steps.iter().filter(|s| s.error.is_some()).count();

        let total_tokens: u32 = steps
            .iter()
            .filter_map(|s| s.token_usage.as_ref())
            .map(|u| u.total_tokens)
            .sum();

        ExecutionSummary {
            session_id: self.session_id.clone(),
            total_steps: steps.len(),
            llm_requests,
            tool_executions,
            errors,
            total_duration_ms: total_duration.as_millis() as u64,
            total_tokens,
        }
    }

    /// Export trace to JSON
    pub async fn export_json(&self) -> anyhow::Result<String> {
        let steps = self.steps.read().await;
        Ok(serde_json::to_string_pretty(&*steps)?)
    }

    /// Clear trace
    pub async fn clear(&self) {
        let mut steps = self.steps.write().await;
        steps.clear();
    }
}

impl Default for ExecutionTracer {
    fn default() -> Self {
        Self::new()
    }
}

/// Summary of execution statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub session_id: String,
    pub total_steps: usize,
    pub llm_requests: usize,
    pub tool_executions: usize,
    pub errors: usize,
    pub total_duration_ms: u64,
    pub total_tokens: u32,
}

/// Error recovery strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryStrategy {
    /// Retry the operation
    Retry { max_attempts: u32 },
    /// Fall back to alternative provider
    Fallback,
    /// Skip and continue
    Skip,
    /// Fail immediately
    Fail,
}

/// Error recovery configuration
#[derive(Debug, Clone)]
pub struct ErrorRecoveryConfig {
    pub llm_error_strategy: RecoveryStrategy,
    pub tool_error_strategy: RecoveryStrategy,
    pub retry_base_delay_ms: u64,
    pub retry_max_delay_ms: u64,
    pub retry_backoff_multiplier: f64,
}

impl Default for ErrorRecoveryConfig {
    fn default() -> Self {
        Self {
            llm_error_strategy: RecoveryStrategy::Retry { max_attempts: 3 },
            tool_error_strategy: RecoveryStrategy::Retry { max_attempts: 2 },
            retry_base_delay_ms: 1000,
            retry_max_delay_ms: 30000,
            retry_backoff_multiplier: 2.0,
        }
    }
}

/// Error recovery handler
pub struct ErrorRecovery {
    config: ErrorRecoveryConfig,
}

impl ErrorRecovery {
    /// Create new error recovery handler
    pub fn new(config: ErrorRecoveryConfig) -> Self {
        Self { config }
    }

    /// Execute with retry logic
    pub async fn execute_with_retry<F, Fut, T>(
        &self,
        operation: F,
        strategy: RecoveryStrategy,
    ) -> Result<T, AgentError>
    where
        F: Fn() -> Fut,
        Fut: std::future::Future<Output = Result<T, AgentError>>,
    {
        match strategy {
            RecoveryStrategy::Retry { max_attempts } => {
                let mut last_error = None;

                for attempt in 0..max_attempts {
                    match operation().await {
                        Ok(result) => {
                            if attempt > 0 {
                                info!("Operation succeeded after {} retries", attempt);
                            }
                            return Ok(result);
                        }
                        Err(e) => {
                            last_error = Some(e);
                            if attempt < max_attempts - 1 {
                                let delay = self.calculate_delay(attempt);
                                warn!(
                                    "Operation failed (attempt {}/{}), retrying after {:?}",
                                    attempt + 1,
                                    max_attempts,
                                    delay
                                );
                                tokio::time::sleep(delay).await;
                            }
                        }
                    }
                }

                Err(last_error.expect("Last error should exist"))
            }
            RecoveryStrategy::Fail => operation().await,
            _ => operation().await,
        }
    }

    /// Calculate retry delay with exponential backoff
    fn calculate_delay(&self, attempt: u32) -> Duration {
        let exponential = self.config.retry_base_delay_ms as f64
            * self.config.retry_backoff_multiplier.powi(attempt as i32);
        let delay_ms = exponential.min(self.config.retry_max_delay_ms as f64) as u64;

        // Add jitter (±25%)
        let jitter = 0.75 + rand::random::<f64>() * 0.5;
        Duration::from_millis((delay_ms as f64 * jitter) as u64)
    }
}

/// Conversation state for persistence
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationState {
    pub session_id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub messages: Vec<Message>,
    pub metadata: HashMap<String, serde_json::Value>,
    pub iteration_count: usize,
}

impl ConversationState {
    /// Create new conversation state
    pub fn new() -> Self {
        let now = chrono::Utc::now();
        Self {
            session_id: uuid::Uuid::new_v4().to_string(),
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
            metadata: HashMap::new(),
            iteration_count: 0,
        }
    }

    /// Add a message
    pub fn add_message(&mut self, message: Message) {
        self.messages.push(message);
        self.updated_at = chrono::Utc::now();
    }

    /// Set metadata
    pub fn set_metadata(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.metadata.insert(key.into(), value);
    }
}

impl Default for ConversationState {
    fn default() -> Self {
        Self::new()
    }
}

/// State persistence manager
#[derive(Debug, Clone)]
pub struct StatePersistence {
    storage_dir: PathBuf,
}

impl StatePersistence {
    /// Create new state persistence manager
    pub fn new(storage_dir: impl Into<PathBuf>) -> Self {
        let dir = storage_dir.into();
        std::fs::create_dir_all(&dir).ok();
        Self { storage_dir: dir }
    }

    /// Save conversation state
    pub async fn save(&self, state: &ConversationState) -> anyhow::Result<()> {
        let path = self.storage_dir.join(format!("{}.json", state.session_id));
        let content = serde_json::to_string_pretty(state)?;
        tokio::fs::write(&path, content).await?;
        debug!("Saved conversation state to {:?}", path);
        Ok(())
    }

    /// Load conversation state
    pub async fn load(&self, session_id: &str) -> anyhow::Result<ConversationState> {
        let path = self.storage_dir.join(format!("{}.json", session_id));
        let content = tokio::fs::read_to_string(&path).await?;
        let state: ConversationState = serde_json::from_str(&content)?;
        Ok(state)
    }

    /// List all saved sessions
    pub async fn list_sessions(&self) -> anyhow::Result<Vec<String>> {
        let mut sessions = Vec::new();
        let mut entries = tokio::fs::read_dir(&self.storage_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            if let Some(name) = entry.file_name().to_str() {
                if name.ends_with(".json") {
                    sessions.push(name.trim_end_matches(".json").to_string());
                }
            }
        }

        Ok(sessions)
    }

    /// Delete a session
    pub async fn delete(&self, session_id: &str) -> anyhow::Result<()> {
        let path = self.storage_dir.join(format!("{}.json", session_id));
        tokio::fs::remove_file(&path).await?;
        Ok(())
    }

    /// Get default storage directory
    pub fn default_dir() -> PathBuf {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("clarity")
            .join("sessions")
    }
}

impl Default for StatePersistence {
    fn default() -> Self {
        Self::new(Self::default_dir())
    }
}

/// Parallel tool execution support
pub struct ParallelToolExecutor;

impl ParallelToolExecutor {
    /// Execute multiple tool calls in parallel
    pub async fn execute_parallel<F, Fut, T, E>(
        tool_calls: Vec<ToolCall>,
        executor: F,
    ) -> Vec<(String, Result<T, E>)>
    where
        F: Fn(&ToolCall) -> Fut + Clone + Send + Sync,
        Fut: std::future::Future<Output = Result<T, E>> + Send,
        T: Send,
        E: Send,
    {
        let futures: Vec<_> = tool_calls
            .into_iter()
            .map(|tool_call| {
                let exec = executor.clone();
                async move {
                    let id = tool_call.id.clone();
                    let result = exec(&tool_call).await;
                    (id, result)
                }
            })
            .collect();

        futures::future::join_all(futures).await
    }
}

/// Helper function to truncate strings
fn truncate_string(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}... (truncated)", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_step_creation() {
        let step = ExecutionStep {
            step_number: 1,
            step_type: StepType::LlmRequest,
            start_time: chrono::Utc::now(),
            duration_ms: 100,
            input_summary: "test input".to_string(),
            output_summary: "test output".to_string(),
            error: None,
            token_usage: Some(TokenUsage {
                prompt_tokens: 10,
                completion_tokens: 5,
                total_tokens: 15,
            }),
        };

        assert_eq!(step.step_number, 1);
        assert!(step.error.is_none());
    }

    #[test]
    fn test_conversation_state() {
        let mut state = ConversationState::new();
        assert!(state.messages.is_empty());

        state.add_message(Message::user("Hello"));
        assert_eq!(state.messages.len(), 1);
        assert_eq!(state.iteration_count, 0);
    }

    #[tokio::test]
    async fn test_tracer() {
        let tracer = ExecutionTracer::new();

        tracer
            .record_step(
                1,
                StepType::LlmRequest,
                "input",
                "output",
                Duration::from_millis(100),
                None,
            )
            .await;

        let steps = tracer.get_steps().await;
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].step_number, 1);
    }

    #[test]
    fn test_truncate_string() {
        let long = "a".repeat(300);
        let truncated = truncate_string(&long, 200);
        assert!(truncated.len() < long.len());
        assert!(truncated.contains("truncated"));
    }
}
