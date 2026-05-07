//! Agent Teams — collaborative sub-agent execution with shared Mailbox.
//!
//! An `AgentTeam` is a group of sub-agents that share a `Mailbox`, allowing
//! loose coordination: members can broadcast intermediate results or status
//! updates, and the team coordinator collects everything into a unified
//! `TeamResult`.
//!
//! # Example
//!
//! ```rust,no_run
//! use clarity_core::subagents::{AgentTeam, RunSpec, ParallelConfig, TeamCoordinator};
//!
//! #[tokio::main]
//! async fn main() -> anyhow::Result<()> {
//!     let team = AgentTeam::new("refactor-squad", "Refactor the auth module")
//!         .with_member(RunSpec::new("Extract traits", "Identify shared traits").with_type("explore"))
//!         .with_member(RunSpec::new("Migrate code", "Move impl blocks").with_type("coder"))
//!         .with_config(ParallelConfig::new().with_max_concurrency(2));
//!
//!     // coordinator.execute_team(team).await?;
//!     Ok(())
//! }
//! ```

use super::parallel::{ParallelConfig, ParallelExecutor, ParallelResult, SubagentBatch};
use super::runner::{RunSpec, SubagentRunner};
use super::store::SubagentStatus;
use crate::background::BackgroundTaskManager;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// A message sent between team members via the shared [`Mailbox`].
#[derive(Debug, Clone)]
pub struct MailboxMessage {
    /// Agent ID of the sender.
    pub from: String,
    /// Message payload.
    pub payload: MessagePayload,
    /// Unix timestamp (seconds).
    pub timestamp: u64,
}

/// Payload variants for [`MailboxMessage`].
#[derive(Debug, Clone)]
pub enum MessagePayload {
    /// Free-form text broadcast.
    Text(String),
    /// Status update (started, completed, failed, etc.).
    StatusUpdate(SubagentStatus),
    /// Intermediate result that other members may consume.
    IntermediateResult(String),
}

/// Shared message bus for an [`AgentTeam`].
///
/// Uses a `tokio::sync::broadcast` channel under the hood. Any clone of the
/// mailbox can send; receivers are obtained via [`Mailbox::subscribe`].
#[derive(Clone)]
pub struct Mailbox {
    tx: tokio::sync::broadcast::Sender<MailboxMessage>,
}

impl Default for Mailbox {
    fn default() -> Self {
        Self::new()
    }
}

impl Mailbox {
    /// Create a new mailbox with capacity for 256 in-flight messages.
    pub fn new() -> Self {
        let (tx, _rx) = tokio::sync::broadcast::channel(256);
        Self { tx }
    }

    /// Subscribe to messages. Callers will receive messages sent *after* the
    /// subscription is created.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<MailboxMessage> {
        self.tx.subscribe()
    }

    /// Broadcast a message to all active subscribers.
    pub fn send(&self, msg: MailboxMessage) -> Result<(), MailboxError> {
        // If there are no receivers the message is silently dropped;
        // that is acceptable for a fire-and-forget broadcast.
        let _ = self.tx.send(msg);
        Ok(())
    }

    /// Returns the number of active subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.tx.receiver_count()
    }
}

/// Error type for mailbox operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MailboxError {
    /// The mailbox has been closed and no more messages can be sent.
    Closed,
    /// The message channel is full.
    Full,
}

impl std::fmt::Display for MailboxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MailboxError::Closed => write!(f, "Mailbox closed"),
            MailboxError::Full => write!(f, "Mailbox full"),
        }
    }
}

impl std::error::Error for MailboxError {}

/// A team of sub-agents working toward a shared goal.
#[derive(Clone)]
pub struct AgentTeam {
    /// Human-readable team name.
    pub name: String,
    /// High-level objective.
    pub goal: String,
    /// Member specifications.
    pub members: Vec<RunSpec>,
    /// Shared mailbox for loose coordination.
    pub mailbox: Mailbox,
    /// Parallel execution configuration.
    pub config: ParallelConfig,
}

impl AgentTeam {
    /// Create a new team with the given name and goal.
    pub fn new(name: impl Into<String>, goal: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            goal: goal.into(),
            members: Vec::new(),
            mailbox: Mailbox::new(),
            config: ParallelConfig::default(),
        }
    }

    /// Add a member to the team (builder pattern).
    pub fn with_member(mut self, spec: RunSpec) -> Self {
        self.members.push(spec);
        self
    }

    /// Batch-add members (builder pattern).
    pub fn with_members(mut self, specs: Vec<RunSpec>) -> Self {
        self.members.extend(specs);
        self
    }

    /// Set parallel execution config (builder pattern).
    pub fn with_config(mut self, config: ParallelConfig) -> Self {
        self.config = config;
        self
    }

    /// Replace the default mailbox with a custom one.
    pub fn with_mailbox(mut self, mailbox: Mailbox) -> Self {
        self.mailbox = mailbox;
        self
    }

    /// Returns true if the team has no members.
    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// Number of members.
    pub fn len(&self) -> usize {
        self.members.len()
    }
}

/// Unified result after executing an [`AgentTeam`].
#[derive(Debug, Clone)]
pub struct TeamResult {
    /// Underlying parallel execution results.
    pub parallel: ParallelResult,
    /// Messages collected from the team's mailbox during execution.
    pub messages: Vec<MailboxMessage>,
}

impl TeamResult {
    /// Check if every member succeeded.
    pub fn all_succeeded(&self) -> bool {
        self.parallel.all_succeeded()
    }

    /// Aggregate success rate.
    pub fn success_rate(&self) -> f64 {
        self.parallel.success_rate()
    }

    /// Filter messages by payload type.
    pub fn filter_text(&self) -> Vec<&MailboxMessage> {
        self.messages
            .iter()
            .filter(|m| matches!(m.payload, MessagePayload::Text(_)))
            .collect()
    }

    /// Filter intermediate results.
    pub fn filter_intermediate(&self) -> Vec<&MailboxMessage> {
        self.messages
            .iter()
            .filter(|m| matches!(m.payload, MessagePayload::IntermediateResult(_)))
            .collect()
    }
}

/// Coordinates the execution of an [`AgentTeam`].
///
/// Wraps a [`ParallelExecutor`] and adds a mailbox-message collector task.
pub struct TeamCoordinator {
    executor: ParallelExecutor,
}

impl TeamCoordinator {
    /// Create a new coordinator from a task manager and a runner.
    pub fn new(task_manager: BackgroundTaskManager, runner: SubagentRunner) -> Self {
        Self {
            executor: ParallelExecutor::new(task_manager, runner),
        }
    }

    /// Execute the team and collect both parallel results and mailbox messages.
    pub async fn execute_team(&mut self, team: AgentTeam) -> anyhow::Result<TeamResult> {
        if team.is_empty() {
            return Ok(TeamResult {
                parallel: ParallelResult {
                    results: Vec::new(),
                    failures: Vec::new(),
                    total_elapsed_ms: 0,
                    actual_concurrency: 0,
                    aggregated_summary: None,
                },
                messages: Vec::new(),
            });
        }

        let mut rx = team.mailbox.subscribe();
        let collected = Arc::new(Mutex::new(Vec::new()));
        let collected_clone = collected.clone();

        let cancel = CancellationToken::new();
        let cancel_clone = cancel.clone();

        // Background task: collect mailbox messages until cancelled.
        let collector = tokio::spawn(async move {
            loop {
                tokio::select! {
                    Ok(msg) = rx.recv() => {
                        collected_clone.lock().await.push(msg);
                    }
                    _ = cancel_clone.cancelled() => break,
                }
            }
        });

        let batch = SubagentBatch::new()
            .add_many(team.members)
            .with_config(team.config);

        let parallel_result = self.executor.execute(batch, None, Some(cancel.clone())).await;

        // Gracefully stop the collector.
        cancel.cancel();
        // Give the collector a short grace period to drain pending messages.
        let _ = tokio::time::timeout(tokio::time::Duration::from_secs(1), collector).await;

        let messages = Arc::try_unwrap(collected)
            .map_err(|_| anyhow::anyhow!("Mailbox collector still referenced"))?
            .into_inner();

        Ok(TeamResult {
            parallel: parallel_result?,
            messages,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mailbox_send_and_receive() {
        let mb = Mailbox::new();
        let mut rx = mb.subscribe();

        let msg = MailboxMessage {
            from: "agent-1".to_string(),
            payload: MessagePayload::Text("hello".to_string()),
            timestamp: 0,
        };
        mb.send(msg.clone()).unwrap();

        let received = rx.try_recv().expect("Should receive message");
        assert_eq!(received.from, "agent-1");
        match received.payload {
            MessagePayload::Text(t) => assert_eq!(t, "hello"),
            _ => panic!("Expected Text payload"),
        }
    }

    #[test]
    fn test_team_builder() {
        let team = AgentTeam::new("test-team", "Do something")
            .with_member(RunSpec::new("Task A", "Desc A"))
            .with_member(RunSpec::new("Task B", "Desc B"))
            .with_config(ParallelConfig::new().with_max_concurrency(2));

        assert_eq!(team.name, "test-team");
        assert_eq!(team.goal, "Do something");
        assert_eq!(team.len(), 2);
        assert_eq!(team.config.max_concurrency, 2);
    }

    #[test]
    fn test_team_empty() {
        let team = AgentTeam::new("empty", "Nothing");
        assert!(team.is_empty());
    }

    #[test]
    fn test_team_result_filtering() {
        let result = TeamResult {
            parallel: ParallelResult {
                results: Vec::new(),
                failures: Vec::new(),
                total_elapsed_ms: 0,
                actual_concurrency: 0,
                aggregated_summary: None,
            },
            messages: vec![
                MailboxMessage {
                    from: "a".to_string(),
                    payload: MessagePayload::Text("t".to_string()),
                    timestamp: 0,
                },
                MailboxMessage {
                    from: "b".to_string(),
                    payload: MessagePayload::IntermediateResult("r".to_string()),
                    timestamp: 1,
                },
            ],
        };

        assert_eq!(result.filter_text().len(), 1);
        assert_eq!(result.filter_intermediate().len(), 1);
    }
}
