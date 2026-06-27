//! Turn-level lifecycle state machine for the Agent loop.
//!
//! These types live in the contract crate so that `clarity-rollout`, frontends,
//! and sync transports can all reason about a single turn's lifecycle without
//! depending on `clarity-core`.

use crate::error::AgentError;
use serde::{Deserialize, Serialize};

/// Lifecycle state of a single turn inside the Agent loop.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunState {
    /// Turn has not started.
    #[default]
    Idle,
    /// LLM is being asked to decide the next step.
    Planning,
    /// LLM requested tool calls; they are being executed.
    AwaitingTools,
    /// Tool results are being synthesized into a final or next response.
    Synthesizing,
    /// The loop is paused waiting for user input/approval.
    AwaitingUser { question: String },
    /// The turn completed normally with a final response.
    Complete { response: String },
    /// The turn was interrupted (cancelled, guardrail stop, or non-fatal halt).
    Interrupted { reason: String },
    /// The turn failed with a fatal error.
    Error { error: String },
}

impl RunState {
    /// True if the turn has reached a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::Complete { .. } | Self::Interrupted { .. } | Self::Error { .. }
        )
    }

    /// True if the loop is waiting for the user.
    pub fn is_awaiting_user(&self) -> bool {
        matches!(self, Self::AwaitingUser { .. })
    }

    /// Extract the final response if the state is terminal and has one.
    pub fn response(&self) -> Option<&str> {
        match self {
            Self::Complete { response } => Some(response),
            Self::AwaitingUser { question } => Some(question),
            Self::Interrupted { reason } => Some(reason),
            Self::Error { error } => Some(error),
            _ => None,
        }
    }
}

/// Events that drive `RunState` transitions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RunEvent {
    /// User message received; turn starts.
    UserTurn {
        /// The user's prompt text.
        input: String,
        /// User identifier for attribution.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        user_id: Option<String>,
        /// Team identifier for team-scoped turns.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        team_id: Option<String>,
        /// Organization identifier.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        org_id: Option<String>,
    },
    /// LLM produced tool calls.
    ToolCallsRequested { count: usize },
    /// LLM produced a final response without tool calls.
    FinalResponse { response: String },
    /// Tool calls executed successfully.
    ToolsSucceeded { tool_names: Vec<String> },
    /// The loop asked the user a question.
    AskUser { question: String },
    /// A guardrail or policy stopped the loop.
    Stopped { reason: String },
    /// The user or system cancelled the turn.
    Cancelled { reason: String },
    /// A fatal error occurred.
    Fatal { error: String },
}

/// Invalid transition error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvalidTransition {
    pub from: RunState,
    pub event: RunEvent,
}

impl std::fmt::Display for InvalidTransition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid transition from {:?} via {:?}",
            self.from, self.event
        )
    }
}

impl std::error::Error for InvalidTransition {}

impl From<InvalidTransition> for AgentError {
    fn from(value: InvalidTransition) -> Self {
        Self::InvalidStateTransition(value.to_string())
    }
}

impl RunState {
    /// Apply a lifecycle event and return the new state.
    // SAFETY: Err variant is large (RunEvent carries identity String fields),
    // but apply() is called infrequently (once per state transition, max ~10/turn).
    #[allow(clippy::result_large_err)]
    pub fn apply(self, event: RunEvent) -> Result<Self, InvalidTransition> {
        match (self, event) {
            (Self::Idle, RunEvent::UserTurn { .. }) => Ok(Self::Planning),

            (Self::Planning, RunEvent::ToolCallsRequested { .. }) => Ok(Self::AwaitingTools),
            (Self::Planning, RunEvent::FinalResponse { response }) => {
                Ok(Self::Complete { response })
            }
            (Self::Planning, RunEvent::AskUser { question }) => Ok(Self::AwaitingUser { question }),
            (Self::Planning, RunEvent::Cancelled { reason }) => Ok(Self::Interrupted { reason }),
            (Self::Planning, RunEvent::Fatal { error }) => Ok(Self::Error { error }),

            (Self::AwaitingTools, RunEvent::ToolsSucceeded { .. }) => Ok(Self::Synthesizing),
            (Self::AwaitingTools, RunEvent::AskUser { question }) => {
                Ok(Self::AwaitingUser { question })
            }
            (Self::AwaitingTools, RunEvent::Stopped { reason }) => Ok(Self::Interrupted { reason }),
            (Self::AwaitingTools, RunEvent::Cancelled { reason }) => {
                Ok(Self::Interrupted { reason })
            }
            (Self::AwaitingTools, RunEvent::Fatal { error }) => Ok(Self::Error { error }),

            (Self::Synthesizing, RunEvent::ToolCallsRequested { .. }) => Ok(Self::AwaitingTools),
            (Self::Synthesizing, RunEvent::FinalResponse { response }) => {
                Ok(Self::Complete { response })
            }
            (Self::Synthesizing, RunEvent::AskUser { question }) => {
                Ok(Self::AwaitingUser { question })
            }
            (Self::Synthesizing, RunEvent::Stopped { reason }) => Ok(Self::Interrupted { reason }),
            (Self::Synthesizing, RunEvent::Cancelled { reason }) => {
                Ok(Self::Interrupted { reason })
            }
            (Self::Synthesizing, RunEvent::Fatal { error }) => Ok(Self::Error { error }),

            (Self::AwaitingUser { .. }, RunEvent::UserTurn { .. }) => Ok(Self::Planning),
            (Self::AwaitingUser { .. }, RunEvent::Cancelled { reason }) => {
                Ok(Self::Interrupted { reason })
            }

            (from, event) => Err(InvalidTransition { from, event }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_to_planning() {
        let state = RunState::Idle;
        let next = state
            .apply(RunEvent::UserTurn {
                input: "hi".into(),
                user_id: None,
                team_id: None,
                org_id: None,
            })
            .unwrap();
        assert_eq!(next, RunState::Planning);
    }

    #[test]
    fn planning_to_complete() {
        let state = RunState::Planning;
        let next = state
            .apply(RunEvent::FinalResponse {
                response: "done".into(),
            })
            .unwrap();
        assert_eq!(
            next,
            RunState::Complete {
                response: "done".into()
            }
        );
    }

    #[test]
    fn invalid_transition_rejected() {
        let state = RunState::Idle;
        let result = state.apply(RunEvent::FinalResponse {
            response: "oops".into(),
        });
        assert!(result.is_err());
    }

    #[test]
    fn user_turn_backward_compat_deserialize() {
        // Old format without identity fields should deserialize with None.
        let old_json = r#"{"UserTurn":{"input":"hello"}}"#;
        let event: RunEvent = serde_json::from_str(old_json).unwrap();
        assert_eq!(
            event,
            RunEvent::UserTurn {
                input: "hello".into(),
                user_id: None,
                team_id: None,
                org_id: None,
            }
        );
    }
}
