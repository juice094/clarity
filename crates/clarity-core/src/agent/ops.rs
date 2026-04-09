//! Operations that can be dispatched to an AgentController.

use crate::personality::PersonalityConfig;

/// An operation submitted to the controller.
#[derive(Debug, Clone)]
pub enum Op {
    /// Start or continue a user turn.
    UserTurn(String),
    /// Cancel the current in-flight agent run.
    Interrupt,
    /// Respond to a pending tool-approval request.
    ToolApproval { request_id: String, approved: bool },
    /// Trigger context compaction.
    Compact,
    /// Update personality configuration without restarting.
    UpdatePersonality(PersonalityConfig),
    /// Shut down the controller gracefully.
    Shutdown,
}

/// Result of processing an operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpResult {
    /// Operation completed successfully.
    Success(String),
    /// The operation was interrupted.
    Interrupted,
    /// The agent is waiting for approval.
    WaitingForApproval,
    /// An error occurred.
    Error(String),
}
