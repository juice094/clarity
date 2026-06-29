//! Wire ↔ TUI event adapter.
//!
//! ADR-006 Phase C.2 (2026-05-11): view channel adapter removed.
//! Only WireMessage broadcast remains; ViewCommand is now populated
//! directly by `SettingsViewModel::commands()` (see `commands.rs`).

use clarity_wire::{WireMessage, WireUISide};
use tokio::sync::mpsc::UnboundedSender;

use crate::events::{Event, ToolCallInfo, ToolStatus};

/// Spawn a background task that reads from a Wire UI side and translates
/// `WireMessage`s into `Event`s on the existing MPSC channel.
pub fn spawn_wire_adapter(mut ui_side: WireUISide, event_tx: UnboundedSender<Event>) {
    tokio::spawn(async move {
        while let Some(msg) = ui_side.recv().await {
            let event = match msg {
                WireMessage::ContentPart { text, .. } if !text.is_empty() => {
                    Some(Event::StreamResponse(text))
                }
                WireMessage::ToolCall {
                    id: _,
                    name,
                    arguments,
                    ..
                } => Some(Event::ToolCall(ToolCallInfo {
                    name,
                    params: arguments.to_string(),
                    status: ToolStatus::Running,
                })),
                WireMessage::ToolResult { id: _, result, .. } => {
                    Some(Event::ToolResult(ToolCallInfo {
                        name: "result".to_string(),
                        params: result,
                        status: ToolStatus::Success,
                    }))
                }
                WireMessage::StepBegin { tool_name, .. } => Some(Event::ToolCall(ToolCallInfo {
                    name: tool_name,
                    params: String::new(),
                    status: ToolStatus::Running,
                })),
                WireMessage::StatusUpdate { message, .. } => Some(Event::StreamResponse(message)),
                WireMessage::TurnEnd { .. } => Some(Event::ResponseComplete),
                WireMessage::Usage {
                    prompt_tokens,
                    completion_tokens,
                    total_tokens,
                    ..
                } => Some(Event::Usage {
                    prompt_tokens,
                    completion_tokens,
                    total_tokens,
                }),
                _ => None,
            };

            if let Some(ev) = event
                && event_tx.send(ev).is_err()
            {
                break;
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_wire::Wire;
    use tokio::sync::mpsc;

    #[tokio::test]
    async fn test_adapter_content_part() {
        let wire = Wire::new();
        let (tx, mut rx) = mpsc::unbounded_channel::<Event>();
        spawn_wire_adapter(wire.ui_side(false), tx);

        let _ = wire.soul_side().send(WireMessage::ContentPart {
            turn_id: String::new(),
            text: "hello".to_string(),
        });

        let ev: Event = rx.recv().await.unwrap();
        assert!(matches!(ev, Event::StreamResponse(t) if t == "hello"));
    }

    #[tokio::test]
    async fn test_adapter_tool_call_and_result() {
        let wire = Wire::new();
        let (tx, mut rx) = mpsc::unbounded_channel::<Event>();
        spawn_wire_adapter(wire.ui_side(false), tx);

        let _ = wire.soul_side().send(WireMessage::ToolCall {
            turn_id: String::new(),
            id: "1".to_string(),
            name: "read_file".to_string(),
            arguments: serde_json::json!({"path": "/tmp"}),
        });

        let ev: Event = rx.recv().await.unwrap();
        match ev {
            Event::ToolCall(info) => {
                assert_eq!(info.name, "read_file");
                assert!(info.params.contains("/tmp"));
                assert!(matches!(info.status, ToolStatus::Running));
            }
            _ => panic!("expected ToolCall event"),
        }

        let _ = wire.soul_side().send(WireMessage::ToolResult {
            turn_id: String::new(),
            id: "1".to_string(),
            result: "ok".to_string(),
            display_result: None,
        });

        let ev: Event = rx.recv().await.unwrap();
        match ev {
            Event::ToolResult(info) => {
                assert_eq!(info.params, "ok");
                assert!(matches!(info.status, ToolStatus::Success));
            }
            _ => panic!("expected ToolResult event"),
        }
    }

    #[tokio::test]
    async fn test_adapter_turn_end() {
        let wire = Wire::new();
        let (tx, mut rx) = mpsc::unbounded_channel::<Event>();
        spawn_wire_adapter(wire.ui_side(false), tx);

        let _ = wire.soul_side().send(WireMessage::TurnEnd {
            turn_id: String::new(),
        });

        let ev: Event = rx.recv().await.unwrap();
        assert!(matches!(ev, Event::ResponseComplete));
    }

    #[tokio::test]
    async fn test_adapter_empty_content_part_filtered() {
        let wire = Wire::new();
        let (tx, mut rx) = mpsc::unbounded_channel::<Event>();
        spawn_wire_adapter(wire.ui_side(false), tx);

        let _ = wire.soul_side().send(WireMessage::ContentPart {
            turn_id: String::new(),
            text: String::new(),
        });
        let _ = wire.soul_side().send(WireMessage::TurnEnd {
            turn_id: String::new(),
        });

        // Should only receive TurnEnd, empty ContentPart is filtered
        let ev: Event = rx.recv().await.unwrap();
        assert!(matches!(ev, Event::ResponseComplete));
    }
}
