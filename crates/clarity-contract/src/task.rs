//! Background task abstraction for the Clarity contract layer.
//!
//! These types allow `clarity-subagents` to spawn and monitor background work
//! without depending on the concrete `clarity-core::background::BackgroundTaskManager`.

use serde::{Deserialize, Serialize};
use std::future::Future;
use std::pin::Pin;

/// Task identifier.
pub type TaskId = String;

/// Task priority.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord, Hash, Default,
)]
pub enum TaskPriority {
    /// Background, lowest priority.
    Background = 0,
    /// Low priority.
    Low = 1,
    /// Normal priority (default).
    #[default]
    Normal = 2,
    /// High priority.
    High = 3,
    /// Critical, highest priority.
    Critical = 4,
}

impl TaskPriority {
    /// Numeric priority value.
    pub fn value(&self) -> u8 {
        *self as u8
    }

    /// Create a priority from its numeric value.
    pub fn from_value(value: u8) -> Self {
        match value {
            0 => TaskPriority::Background,
            1 => TaskPriority::Low,
            2 => TaskPriority::Normal,
            3 => TaskPriority::High,
            4 => TaskPriority::Critical,
            _ => TaskPriority::Normal,
        }
    }
}

/// Task execution status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskStatus {
    /// Task is pending.
    Pending,
    /// Task is running.
    Running,
    /// Task completed.
    Completed,
    /// Task failed.
    Failed,
    /// Task cancelled.
    Cancelled,
}

impl TaskStatus {
    /// Whether the status is terminal.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        )
    }

    /// String representation.
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Pending => "pending",
            TaskStatus::Running => "running",
            TaskStatus::Completed => "completed",
            TaskStatus::Failed => "failed",
            TaskStatus::Cancelled => "cancelled",
        }
    }
}

/// Task specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    /// Display name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Agent type identifier.
    pub agent_type: String,
    /// Prompt text.
    pub prompt: String,
    /// Maximum iteration count.
    pub max_iterations: Option<usize>,
    /// Timeout in seconds.
    #[serde(alias = "timeout_secs")]
    pub timeout_seconds: Option<u64>,
    /// Task priority.
    #[serde(default)]
    pub priority: TaskPriority,
    /// Optional model alias override.
    #[serde(default)]
    pub model_alias: Option<String>,
}

impl TaskSpec {
    /// Create a new task spec.
    pub fn new(name: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: String::new(),
            agent_type: "default".to_string(),
            prompt: prompt.into(),
            max_iterations: None,
            timeout_seconds: None,
            priority: TaskPriority::Normal,
            model_alias: None,
        }
    }

    /// Set the description.
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Set the agent type.
    pub fn with_agent_type(mut self, agent_type: impl Into<String>) -> Self {
        self.agent_type = agent_type.into();
        self
    }

    /// Set the maximum iterations.
    pub fn with_max_iterations(mut self, max_iterations: usize) -> Self {
        self.max_iterations = Some(max_iterations);
        self
    }

    /// Set the timeout in seconds.
    pub fn with_timeout_seconds(mut self, timeout_seconds: u64) -> Self {
        self.timeout_seconds = Some(timeout_seconds);
        self
    }

    /// Set the priority.
    pub fn with_priority(mut self, priority: TaskPriority) -> Self {
        self.priority = priority;
        self
    }

    /// Set the model alias override.
    pub fn with_model_alias(mut self, alias: impl Into<String>) -> Self {
        self.model_alias = Some(alias.into());
        self
    }
}

/// Task execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    /// Task status.
    pub status: TaskStatus,
    /// Task output text.
    pub output: String,
    /// Elapsed time in milliseconds.
    pub elapsed_ms: u64,
    /// Execution steps.
    pub steps: usize,
}

impl TaskResult {
    /// Create a successful result.
    pub fn success(output: impl Into<String>) -> Self {
        Self {
            status: TaskStatus::Completed,
            output: output.into(),
            elapsed_ms: 0,
            steps: 0,
        }
    }

    /// Create a failed result.
    pub fn failed(error: impl Into<String>) -> Self {
        Self {
            status: TaskStatus::Failed,
            output: error.into(),
            elapsed_ms: 0,
            steps: 0,
        }
    }

    /// Set the elapsed time in milliseconds.
    pub fn with_elapsed_ms(mut self, elapsed_ms: u64) -> Self {
        self.elapsed_ms = elapsed_ms;
        self
    }

    /// Set the number of execution steps.
    pub fn with_steps(mut self, steps: usize) -> Self {
        self.steps = steps;
        self
    }
}

/// Abstract task manager.
///
/// Implemented by `clarity_core::background::BackgroundTaskManager` and by
/// lightweight test doubles in `clarity-subagents`.
#[async_trait::async_trait]
pub trait TaskManager: Send + Sync {
    /// Spawn a task described by `spec` and returning `task`.
    async fn spawn(
        &self,
        spec: TaskSpec,
        task: Pin<Box<dyn Future<Output = anyhow::Result<TaskResult>> + Send>>,
    ) -> anyhow::Result<TaskId>;

    /// Wait for a task to complete and return its result.
    async fn wait(&self, task_id: &TaskId) -> anyhow::Result<TaskResult>;

    /// Cancel a running or pending task.
    async fn cancel(&self, task_id: &TaskId) -> anyhow::Result<()>;

    /// Get the current status of a task.
    async fn status(&self, task_id: &TaskId) -> anyhow::Result<TaskStatus>;
}
