//! Operations that can be dispatched to an AgentController.

use clarity_contract::IdentityContext;

/// An operation submitted to the controller.
#[derive(Debug, Clone)]
pub enum Op {
    /// Start or continue a user turn.
    UserTurn {
        /// The user's prompt text.
        input: String,
        /// Identity context for this turn (overrides AgentConfig defaults).
        identity: Option<IdentityContext>,
    },
    /// Cancel the current in-flight agent run.
    Interrupt,
    /// Respond to a pending tool-approval request.
    ToolApproval {
        /// Request identifier.
        request_id: String,
        /// Whether the request is approved.
        approved: bool,
    },
    /// Trigger context compaction.
    Compact,
    /// Shut down the controller gracefully.
    Shutdown,
}

impl Op {
    /// Create a UserTurn with no identity override.
    ///
    /// This preserves the pre-Phase-6 API for callers that don't
    /// supply identity context.
    pub fn user_turn(input: impl Into<String>) -> Self {
        Op::UserTurn {
            input: input.into(),
            identity: None,
        }
    }
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
