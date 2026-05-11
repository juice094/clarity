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
//! use clarity_subagents::{AgentTeam, RunSpec, ParallelConfig, TeamCoordinator};
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

use super::parallel::{ParallelExecutor, SubagentBatch};
use super::runner::SubagentRunner;
use clarity_contract::subagent::{AgentTeam, ParallelResult, TeamResult};
use clarity_core::background::BackgroundTaskManager;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

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

        let parallel_result = self
            .executor
            .execute(batch, None, Some(cancel.clone()))
            .await;

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
    use clarity_contract::subagent::{
        Mailbox, MailboxMessage, MessagePayload, ParallelConfig, RunSpec,
    };

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
