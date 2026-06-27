//! Turn-level lifecycle state machine for the Agent loop.
//!
//! Types are re-exported from `clarity_contract::lifecycle` so that rollouts,
//! frontends, and sync transports can share the same vocabulary without
//! depending on `clarity-core`.

pub use clarity_contract::lifecycle::{InvalidTransition, RunEvent, RunState};

/// Replay a sequence of lifecycle events to recover the turn state.
///
/// ponytail: O(n) sequential replay; fine while event counts stay in the
/// hundreds per turn. Upgrade to snapshot-based recovery if replay latency
/// becomes measurable.
#[allow(dead_code, clippy::result_large_err)] // used by tests and by Phase 7 sync replay; Err is large due to identity fields
pub(crate) fn replay(events: &[RunEvent]) -> Result<RunState, InvalidTransition> {
    let mut state = RunState::Idle;
    for event in events {
        state = state.apply(event.clone())?;
    }
    Ok(state)
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
    fn planning_to_awaiting_tools() {
        let state = RunState::Planning;
        let next = state
            .apply(RunEvent::ToolCallsRequested { count: 2 })
            .unwrap();
        assert_eq!(next, RunState::AwaitingTools);
    }

    #[test]
    fn awaiting_tools_to_synthesizing() {
        let state = RunState::AwaitingTools;
        let next = state
            .apply(RunEvent::ToolsSucceeded {
                tool_names: vec!["read".into()],
            })
            .unwrap();
        assert_eq!(next, RunState::Synthesizing);
    }

    #[test]
    fn synthesizing_loops_back_to_awaiting_tools() {
        let state = RunState::Synthesizing;
        let next = state
            .apply(RunEvent::ToolCallsRequested { count: 1 })
            .unwrap();
        assert_eq!(next, RunState::AwaitingTools);
    }

    #[test]
    fn awaiting_user_resumes_on_user_turn() {
        let state = RunState::AwaitingUser {
            question: "more info?".into(),
        };
        let next = state
            .apply(RunEvent::UserTurn {
                input: "yes".into(),
                user_id: None,
                team_id: None,
                org_id: None,
            })
            .unwrap();
        assert_eq!(next, RunState::Planning);
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
    fn terminal_states_detected() {
        assert!(
            RunState::Complete {
                response: "".into()
            }
            .is_terminal()
        );
        assert!(RunState::Interrupted { reason: "".into() }.is_terminal());
        assert!(RunState::Error { error: "".into() }.is_terminal());
        assert!(!RunState::Planning.is_terminal());
        assert!(
            !RunState::AwaitingUser {
                question: "".into()
            }
            .is_terminal()
        );
    }

    #[test]
    fn replay_recover_complete_state() {
        let events = vec![
            RunEvent::UserTurn {
                input: "hi".into(),
                user_id: None,
                team_id: None,
                org_id: None,
            },
            RunEvent::FinalResponse {
                response: "hello".into(),
            },
        ];
        let state = replay(&events).unwrap();
        assert_eq!(
            state,
            RunState::Complete {
                response: "hello".into()
            }
        );
    }

    #[test]
    fn replay_recover_tool_loop_state() {
        let events = vec![
            RunEvent::UserTurn {
                input: "read file".into(),
                user_id: None,
                team_id: None,
                org_id: None,
            },
            RunEvent::ToolCallsRequested { count: 1 },
            RunEvent::ToolsSucceeded {
                tool_names: vec!["read".into()],
            },
            RunEvent::FinalResponse {
                response: "done".into(),
            },
        ];
        let state = replay(&events).unwrap();
        assert_eq!(
            state,
            RunState::Complete {
                response: "done".into()
            }
        );
    }
}
