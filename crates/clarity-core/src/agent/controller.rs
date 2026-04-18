//! AgentController - Event-driven operation dispatcher for Agent.
//!
//! Inspired by Codex's event loop, the controller owns an [`Agent`] and
//! processes [`Op`]s sent over an async channel.  This allows the UI (or any
//! other frontend) to:
//!
//! * submit new user turns,
//! * interrupt an in-flight turn,
//! * resolve pending tool approvals, and
//! * shut the agent down gracefully.

use crate::agent::ops::Op;
use crate::agent::Agent;
use crate::approval::ApprovalResponse;
use crate::error::AgentError;
use std::sync::Arc;
use tokio::sync::mpsc::{self, UnboundedReceiver, UnboundedSender};
use tokio::task::JoinHandle;
use tracing::{debug, info, warn};

/// Events emitted by [`AgentController`] during a turn.
#[derive(Debug, Clone)]
pub enum ControllerEvent {
    /// A streaming text chunk from the model.
    Chunk(String),
    /// The turn completed successfully with the final response.
    Complete(String),
    /// The turn failed with an error.
    Error(String),
    /// A tool call was initiated by the model (arguments fully assembled).
    ToolCallStart {
        id: String,
        name: String,
        arguments: serde_json::Value,
    },
    /// A tool call finished executing.
    ToolResult {
        id: String,
        result: String,
    },
    /// A new step (tool execution) began.
    StepBegin {
        tool_name: String,
    },
}

/// State machine for the controller's background agent task.
enum ControllerState {
    /// No turn is currently running.
    Idle,
    /// A turn is in progress on a background task.
    Running(JoinHandle<Result<String, AgentError>>),
}

/// Event-driven controller around an [`Agent`].
///
/// Create a controller with [`AgentController::new`], obtain a cheaply-clonable
/// sender via [`AgentController::new_with_sender`], and then spawn or await the
/// controller's own event loop.
pub struct AgentController {
    agent: Agent,
    rx: UnboundedReceiver<Op>,
    event_tx: Option<UnboundedSender<ControllerEvent>>,
}

impl AgentController {
    /// Wrap `agent` in a new controller.
    pub fn new(agent: Agent) -> Self {
        let (_tx, rx) = mpsc::unbounded_channel();
        Self {
            agent,
            rx,
            event_tx: None,
        }
    }

    /// Wrap `agent` in a new controller and return both the controller and a
    /// clonable sender.
    pub fn new_with_sender(agent: Agent) -> (Self, UnboundedSender<Op>) {
        let (tx, rx) = mpsc::unbounded_channel();
        (
            Self {
                agent,
                rx,
                event_tx: None,
            },
            tx,
        )
    }

    /// Wrap `agent` in a new controller with an event channel for streaming
    /// output, and return both the controller and a clonable sender.
    ///
    /// Creates an internal [`clarity_wire::Wire`] and injects it into the agent
    /// so that tool-calling lifecycle events (`ToolCall`, `ToolResult`, `StepBegin`)
    /// are forwarded as [`ControllerEvent`]s.
    pub fn new_with_events(
        mut agent: Agent,
        event_tx: UnboundedSender<ControllerEvent>,
    ) -> (Self, UnboundedSender<Op>) {
        let (tx, rx) = mpsc::unbounded_channel();

        // Inject a wire so that tool-calling events can be observed.
        let wire = Arc::new(clarity_wire::Wire::new());
        agent = agent.with_wire(wire.clone());

        // Spawn a background task that listens on the wire's UI side and
        // forwards tool-related messages as ControllerEvents.
        let wire_event_tx = event_tx.clone();
        let mut wire_ui = wire.ui_side(false);
        tokio::spawn(async move {
            while let Some(msg) = wire_ui.recv().await {
                let event = match msg {
                    clarity_wire::WireMessage::ToolCall { id, name, arguments } => {
                        Some(ControllerEvent::ToolCallStart { id, name, arguments })
                    }
                    clarity_wire::WireMessage::ToolResult { id, result } => {
                        Some(ControllerEvent::ToolResult { id, result })
                    }
                    clarity_wire::WireMessage::StepBegin { tool_name } => {
                        Some(ControllerEvent::StepBegin { tool_name })
                    }
                    _ => None,
                };
                if let Some(ev) = event {
                    if wire_event_tx.send(ev).is_err() {
                        break;
                    }
                }
            }
        });

        (
            Self {
                agent,
                rx,
                event_tx: Some(event_tx),
            },
            tx,
        )
    }

    /// Convenience constructor that spawns the event loop and returns the
    /// clonable sender.
    pub fn spawn(agent: Agent) -> UnboundedSender<Op> {
        let (controller, tx) = Self::new_with_sender(agent);
        tokio::spawn(controller.run());
        tx
    }

    /// Convenience constructor that spawns the event loop with streaming
    /// events and returns the clonable sender.
    pub fn spawn_with_events(
        agent: Agent,
        event_tx: UnboundedSender<ControllerEvent>,
    ) -> UnboundedSender<Op> {
        let (controller, tx) = Self::new_with_events(agent, event_tx);
        tokio::spawn(controller.run());
        tx
    }

    /// Run the event loop until [`Op::Shutdown`] is received or the channel
    /// closes.
    ///
    /// The final accumulated response from the last successful turn is
    /// returned.
    pub async fn run(mut self) -> Result<String, AgentError> {
        let mut result = String::new();
        let mut state = ControllerState::Idle;

        info!("AgentController event loop started");

        loop {
            tokio::select! {
                biased;

                op = self.rx.recv() => {
                    match op {
                        Some(Op::UserTurn(prompt)) => {
                            debug!("Controller: UserTurn (len={})", prompt.len());
                            self.agent.reset_cancel_token();
                            let agent = self.agent.clone();
                            let event_tx = self.event_tx.clone();
                            let event_tx2 = event_tx.clone();
                            let handle = tokio::spawn(async move {
                                let result = agent.run_streaming(&prompt, move |chunk| {
                                    if let Some(ref tx) = event_tx {
                                        let _ = tx.send(ControllerEvent::Chunk(chunk.to_string()));
                                    }
                                }).await;

                                if let Some(ref tx) = event_tx2 {
                                    match &result {
                                        Ok(response) => {
                                            let _ = tx.send(ControllerEvent::Complete(response.clone()));
                                        }
                                        Err(e) => {
                                            let _ = tx.send(ControllerEvent::Error(e.to_string()));
                                        }
                                    }
                                }

                                result
                            });
                            state = ControllerState::Running(handle);
                        }

                        Some(Op::ConversationTurn(messages)) => {
                            debug!("Controller: ConversationTurn ({} messages)", messages.len());
                            self.agent.reset_cancel_token();
                            let agent = self.agent.clone();
                            let event_tx = self.event_tx.clone();
                            let event_tx2 = event_tx.clone();
                            let handle = tokio::spawn(async move {
                                let result = agent.run_streaming_with_messages(messages, move |chunk| {
                                    if let Some(ref tx) = event_tx {
                                        let _ = tx.send(ControllerEvent::Chunk(chunk.to_string()));
                                    }
                                }).await;

                                if let Some(ref tx) = event_tx2 {
                                    match &result {
                                        Ok(response) => {
                                            let _ = tx.send(ControllerEvent::Complete(response.clone()));
                                        }
                                        Err(e) => {
                                            let _ = tx.send(ControllerEvent::Error(e.to_string()));
                                        }
                                    }
                                }

                                result
                            });
                            state = ControllerState::Running(handle);
                        }

                        Some(Op::ConversationTurnSync(messages)) => {
                            debug!("Controller: ConversationTurnSync ({} messages)", messages.len());
                            self.agent.reset_cancel_token();
                            let agent = self.agent.clone();
                            let event_tx = self.event_tx.clone();
                            let event_tx2 = event_tx.clone();
                            let handle = tokio::spawn(async move {
                                let result = agent.run_with_messages_sync(messages).await;

                                if let Some(ref tx) = event_tx2 {
                                    match &result {
                                        Ok(response) => {
                                            let _ = tx.send(ControllerEvent::Complete(response.clone()));
                                        }
                                        Err(e) => {
                                            let _ = tx.send(ControllerEvent::Error(e.to_string()));
                                        }
                                    }
                                }

                                result
                            });
                            state = ControllerState::Running(handle);
                        }

                        Some(Op::Interrupt) => {
                            debug!("Controller: Interrupt");
                            self.agent.cancel();
                        }

                        Some(Op::ToolApproval { request_id, approved }) => {
                            debug!("Controller: ToolApproval {} approved={}", request_id, approved);
                            if let Some(ref rt) = self.agent.approval_runtime() {
                                let response = if approved {
                                    ApprovalResponse::Approve
                                } else {
                                    ApprovalResponse::Reject
                                };
                                if let Err(e) = rt.resolve(&request_id, response).await {
                                    warn!("Failed to resolve approval request: {}", e);
                                }
                            } else {
                                warn!("ToolApproval received but no approval runtime configured");
                            }
                        }

                        Some(Op::Compact) => {
                            debug!("Controller: Compact (no-op; compaction runs automatically inside Agent)");
                        }

                        Some(Op::Shutdown) | None => {
                            debug!("Controller: Shutdown");
                            break;
                        }
                    }
                }

                // Only poll the background task when one is running.
                turn_result = Self::poll_state(&mut state), if matches!(state, ControllerState::Running(_)) => {
                    state = ControllerState::Idle;
                    match turn_result {
                        Some(Ok(Ok(response))) => {
                            result = response;
                        }
                        Some(Ok(Err(AgentError::Cancelled))) => {
                            warn!("Agent turn was cancelled");
                            // Intentionally preserve the previous result
                        }
                        Some(Ok(Err(e))) => {
                            return Err(e);
                        }
                        Some(Err(_)) => {
                            return Err(AgentError::Llm(
                                "Agent background task panicked".to_string(),
                            ));
                        }
                        None => {}
                    }
                }
            }
        }

        // If a turn is still running when shutdown arrives, give it a moment
        // to observe cancellation, but don't block indefinitely.
        if let ControllerState::Running(handle) = state {
            let _ = tokio::time::timeout(tokio::time::Duration::from_secs(5), handle).await;
        }

        info!("AgentController event loop finished");
        Ok(result)
    }

    async fn poll_state(
        state: &mut ControllerState,
    ) -> Option<Result<Result<String, AgentError>, tokio::task::JoinError>> {
        match state {
            ControllerState::Running(handle) => Some(handle.await),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{Agent, AgentConfig};
    use crate::registry::ToolRegistry;

    fn dummy_agent() -> Agent {
        let registry = ToolRegistry::with_builtin_tools();
        Agent::with_config(registry, AgentConfig::default())
    }

    #[tokio::test]
    async fn test_controller_shutdown() {
        let agent = dummy_agent();
        let (controller, tx) = AgentController::new_with_sender(agent);
        let handle = tokio::spawn(controller.run());

        tx.send(Op::Shutdown).unwrap();
        let result = handle.await.unwrap().unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_controller_interrupt_without_turn() {
        let agent = dummy_agent();
        let (controller, tx) = AgentController::new_with_sender(agent);
        let handle = tokio::spawn(controller.run());

        tx.send(Op::Interrupt).unwrap();
        tx.send(Op::Shutdown).unwrap();
        let result = handle.await.unwrap().unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_controller_sender_clone() {
        let agent = dummy_agent();
        let (controller, tx) = AgentController::new_with_sender(agent);
        let tx2 = tx.clone();

        let handle = tokio::spawn(controller.run());
        tx2.send(Op::Shutdown).unwrap();
        let result = handle.await.unwrap().unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn test_controller_interrupt_then_new_turn() {
        use crate::agent::{AgentConfig, MockLlm};
        use std::sync::Arc;

        let registry = ToolRegistry::new();
        let agent =
            Agent::with_config(registry, AgentConfig::default()).with_llm(Arc::new(MockLlm));
        let (controller, tx) = AgentController::new_with_sender(agent);
        let handle = tokio::spawn(controller.run());

        tx.send(Op::UserTurn("hello".to_string())).unwrap();
        tx.send(Op::Interrupt).unwrap();
        tx.send(Op::UserTurn("world".to_string())).unwrap();
        tx.send(Op::Shutdown).unwrap();

        let result = handle.await.unwrap();
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_controller_streaming_events() {
        use crate::agent::{AgentConfig, MockLlm};
        use std::sync::Arc;
        use tokio::time::{timeout, Duration};

        let registry = ToolRegistry::new();
        let agent =
            Agent::with_config(registry, AgentConfig::default()).with_llm(Arc::new(MockLlm));

        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<ControllerEvent>();
        let (controller, op_tx) = AgentController::new_with_events(agent, event_tx);
        let handle = tokio::spawn(controller.run());

        op_tx.send(Op::UserTurn("hello".to_string())).unwrap();

        // MockLlm streams one chunk and then completes, so we expect
        // Chunk followed by Complete.
        let mut saw_chunk = false;
        let mut saw_complete = false;
        while let Ok(Some(event)) = timeout(Duration::from_secs(2), event_rx.recv()).await {
            match event {
                ControllerEvent::Chunk(text) => {
                    assert_eq!(text, "This is a mock response");
                    saw_chunk = true;
                }
                ControllerEvent::Complete(text) => {
                    assert_eq!(text, "This is a mock response");
                    saw_complete = true;
                    break;
                }
                _ => panic!("unexpected event: {:?}", event),
            }
        }
        assert!(saw_chunk, "expected at least one Chunk event");
        assert!(saw_complete, "expected Complete event");

        op_tx.send(Op::Shutdown).unwrap();
        let _ = handle.await;
    }

}
