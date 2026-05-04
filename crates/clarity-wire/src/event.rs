use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::DraftEvent;

static EVENT_ID_COUNTER: AtomicU64 = AtomicU64::new(1);

fn next_event_id() -> String {
    format!("evt-{}", EVENT_ID_COUNTER.fetch_add(1, Ordering::SeqCst))
}

/// Event-driven output model — future protocol layer for Soul→UI communication.
///
/// Inspired by Codex's `Event { id, msg }` protocol. UI renders events,
/// not raw wire messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub id: String,
    pub msg: EventMsg,
}

impl Event {
    /// Creates a new `Event` with an auto-generated sequential ID.
    pub fn new(msg: EventMsg) -> Self {
        Self {
            id: next_event_id(),
            msg,
        }
    }
}

impl From<crate::WireMessage> for Event {
    fn from(msg: crate::WireMessage) -> Self {
        Self::new(msg.into())
    }
}

/// Message payload for an `Event`.
///
/// Mirrors all variants of [`WireMessage`](crate::WireMessage) so that
/// `WireMessage` can be losslessly converted into the event protocol.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum EventMsg {
    TurnBegin {
        user_input: String,
    },
    StepBegin {
        tool_name: String,
    },
    ContentPart {
        text: String,
    },
    DraftEvent {
        event: DraftEvent,
    },
    ToolCall {
        id: String,
        name: String,
        arguments: Value,
    },
    ToolResult {
        id: String,
        result: String,
    },
    TurnEnd,
    Usage {
        prompt_tokens: u32,
        completion_tokens: u32,
        total_tokens: u32,
    },
    StatusUpdate {
        message: String,
    },
    CompactionBegin,
    CompactionEnd,
    PlanStepBegin {
        step_id: String,
        tool_name: String,
    },
    PlanStepEnd {
        step_id: String,
        success: bool,
    },
}

impl From<crate::WireMessage> for EventMsg {
    fn from(msg: crate::WireMessage) -> Self {
        match msg {
            crate::WireMessage::TurnBegin { user_input } => EventMsg::TurnBegin { user_input },
            crate::WireMessage::StepBegin { tool_name } => EventMsg::StepBegin { tool_name },
            crate::WireMessage::ContentPart { text } => EventMsg::ContentPart { text },
            crate::WireMessage::DraftEvent { event } => EventMsg::DraftEvent { event },
            crate::WireMessage::ToolCall {
                id,
                name,
                arguments,
            } => EventMsg::ToolCall {
                id,
                name,
                arguments,
            },
            crate::WireMessage::ToolResult { id, result } => EventMsg::ToolResult { id, result },
            crate::WireMessage::TurnEnd => EventMsg::TurnEnd,
            crate::WireMessage::Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens,
            } => EventMsg::Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens,
            },
            crate::WireMessage::StatusUpdate { message } => EventMsg::StatusUpdate { message },
            crate::WireMessage::CompactionBegin => EventMsg::CompactionBegin,
            crate::WireMessage::CompactionEnd => EventMsg::CompactionEnd,
            crate::WireMessage::PlanStepBegin { step_id, tool_name } => {
                EventMsg::PlanStepBegin { step_id, tool_name }
            }
            crate::WireMessage::PlanStepEnd { step_id, success } => {
                EventMsg::PlanStepEnd { step_id, success }
            }
        }
    }
}

use tokio::sync::broadcast;

/// Broadcast channel wrapper for emitting Events.
///
/// Soul-side producer. UI/front-end subscribes via the paired [`broadcast::Receiver`].
#[derive(Clone, Debug)]
pub struct EventBus {
    tx: broadcast::Sender<Event>,
}

impl EventBus {
    /// Create a new bus with the given buffer capacity.
    /// Returns `(bus, receiver)` — receiver can be cloned for multiple consumers.
    pub fn new(capacity: usize) -> (Self, broadcast::Receiver<Event>) {
        let (tx, rx) = broadcast::channel(capacity);
        (Self { tx }, rx)
    }

    /// Emit an event. Fire-and-forget: channel full → oldest dropped.
    pub fn emit(&self, event: Event) {
        let _ = self.tx.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::WireMessage;

    #[test]
    fn test_event_msg_from_wire_message_turn_begin() {
        let wire = WireMessage::TurnBegin {
            user_input: "hello".to_string(),
        };
        let event_msg: EventMsg = wire.into();
        assert_eq!(
            event_msg,
            EventMsg::TurnBegin {
                user_input: "hello".to_string(),
            }
        );
    }

    #[test]
    fn test_event_msg_from_wire_message_step_begin() {
        let wire = WireMessage::StepBegin {
            tool_name: "read_file".to_string(),
        };
        let event_msg: EventMsg = wire.into();
        assert_eq!(
            event_msg,
            EventMsg::StepBegin {
                tool_name: "read_file".to_string(),
            }
        );
    }

    #[test]
    fn test_event_msg_from_wire_message_content_part() {
        let wire = WireMessage::ContentPart {
            text: "chunk".to_string(),
        };
        let event_msg: EventMsg = wire.into();
        assert_eq!(
            event_msg,
            EventMsg::ContentPart {
                text: "chunk".to_string(),
            }
        );
    }

    #[test]
    fn test_event_msg_from_wire_message_draft_event() {
        let wire = WireMessage::DraftEvent {
            event: DraftEvent::Clear,
        };
        let event_msg: EventMsg = wire.into();
        assert_eq!(
            event_msg,
            EventMsg::DraftEvent {
                event: DraftEvent::Clear,
            }
        );
    }

    #[test]
    fn test_event_msg_from_wire_message_tool_call() {
        let wire = WireMessage::ToolCall {
            id: "call_1".to_string(),
            name: "write_file".to_string(),
            arguments: serde_json::json!({ "path": "/tmp/test" }),
        };
        let event_msg: EventMsg = wire.into();
        assert_eq!(
            event_msg,
            EventMsg::ToolCall {
                id: "call_1".to_string(),
                name: "write_file".to_string(),
                arguments: serde_json::json!({ "path": "/tmp/test" }),
            }
        );
    }

    #[test]
    fn test_event_msg_from_wire_message_tool_result() {
        let wire = WireMessage::ToolResult {
            id: "call_1".to_string(),
            result: "ok".to_string(),
        };
        let event_msg: EventMsg = wire.into();
        assert_eq!(
            event_msg,
            EventMsg::ToolResult {
                id: "call_1".to_string(),
                result: "ok".to_string(),
            }
        );
    }

    #[test]
    fn test_event_msg_from_wire_message_turn_end() {
        let wire = WireMessage::TurnEnd;
        let event_msg: EventMsg = wire.into();
        assert_eq!(event_msg, EventMsg::TurnEnd);
    }

    #[test]
    fn test_event_msg_from_wire_message_usage() {
        let wire = WireMessage::Usage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        };
        let event_msg: EventMsg = wire.into();
        assert_eq!(
            event_msg,
            EventMsg::Usage {
                prompt_tokens: 10,
                completion_tokens: 20,
                total_tokens: 30,
            }
        );
    }

    #[test]
    fn test_event_msg_from_wire_message_status_update() {
        let wire = WireMessage::StatusUpdate {
            message: "working".to_string(),
        };
        let event_msg: EventMsg = wire.into();
        assert_eq!(
            event_msg,
            EventMsg::StatusUpdate {
                message: "working".to_string(),
            }
        );
    }

    #[test]
    fn test_event_msg_from_wire_message_compaction_begin() {
        let wire = WireMessage::CompactionBegin;
        let event_msg: EventMsg = wire.into();
        assert_eq!(event_msg, EventMsg::CompactionBegin);
    }

    #[test]
    fn test_event_msg_from_wire_message_compaction_end() {
        let wire = WireMessage::CompactionEnd;
        let event_msg: EventMsg = wire.into();
        assert_eq!(event_msg, EventMsg::CompactionEnd);
    }

    #[test]
    fn test_event_msg_from_wire_message_plan_step_begin() {
        let wire = WireMessage::PlanStepBegin {
            step_id: "s1".to_string(),
            tool_name: "read".to_string(),
        };
        let event_msg: EventMsg = wire.into();
        assert_eq!(
            event_msg,
            EventMsg::PlanStepBegin {
                step_id: "s1".to_string(),
                tool_name: "read".to_string(),
            }
        );
    }

    #[test]
    fn test_event_msg_from_wire_message_plan_step_end() {
        let wire = WireMessage::PlanStepEnd {
            step_id: "s1".to_string(),
            success: true,
        };
        let event_msg: EventMsg = wire.into();
        assert_eq!(
            event_msg,
            EventMsg::PlanStepEnd {
                step_id: "s1".to_string(),
                success: true,
            }
        );
    }

    #[test]
    fn test_event_from_wire_message_generates_id() {
        let wire = WireMessage::TurnEnd;
        let event: Event = wire.into();
        assert!(!event.id.is_empty());
        assert!(event.id.starts_with("evt-"));
        assert!(matches!(event.msg, EventMsg::TurnEnd));
    }

    #[test]
    fn test_event_serde_roundtrip() {
        let event = Event {
            id: "evt-42".to_string(),
            msg: EventMsg::ContentPart {
                text: "hello".to_string(),
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        let decoded: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(event, decoded);
    }

    #[test]
    fn test_event_msg_serde_roundtrip() {
        for msg in [
            EventMsg::TurnBegin {
                user_input: "hi".to_string(),
            },
            EventMsg::StepBegin {
                tool_name: "t".to_string(),
            },
            EventMsg::ContentPart {
                text: "c".to_string(),
            },
            EventMsg::DraftEvent {
                event: DraftEvent::Progress {
                    text: "p".to_string(),
                },
            },
            EventMsg::ToolCall {
                id: "1".to_string(),
                name: "n".to_string(),
                arguments: serde_json::json!({}),
            },
            EventMsg::ToolResult {
                id: "1".to_string(),
                result: "r".to_string(),
            },
            EventMsg::TurnEnd,
            EventMsg::Usage {
                prompt_tokens: 1,
                completion_tokens: 2,
                total_tokens: 3,
            },
            EventMsg::StatusUpdate {
                message: "m".to_string(),
            },
            EventMsg::CompactionBegin,
            EventMsg::CompactionEnd,
            EventMsg::PlanStepBegin {
                step_id: "s".to_string(),
                tool_name: "t".to_string(),
            },
            EventMsg::PlanStepEnd {
                step_id: "s".to_string(),
                success: false,
            },
        ] {
            let json = serde_json::to_string(&msg).unwrap();
            let decoded: EventMsg = serde_json::from_str(&json).unwrap();
            assert_eq!(msg, decoded);
        }
    }

    #[tokio::test]
    async fn test_event_bus_emit_and_receive() {
        let (bus, mut rx) = EventBus::new(16);
        let event = Event::new(EventMsg::TurnBegin {
            user_input: "hello".to_string(),
        });
        bus.emit(event.clone());
        let received = rx.recv().await.unwrap();
        assert_eq!(received, event);
    }

    #[tokio::test]
    async fn test_event_bus_multiple_receivers() {
        let (bus, mut rx1) = EventBus::new(16);
        let mut rx2 = bus.tx.subscribe();

        let event = Event::new(EventMsg::TurnEnd);
        bus.emit(event.clone());

        let received1 = rx1.recv().await.unwrap();
        let received2 = rx2.recv().await.unwrap();
        assert_eq!(received1, event);
        assert_eq!(received2, event);
    }
}
