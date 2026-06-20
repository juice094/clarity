//! Deserialize `clarity_wire::WireMessage` payloads and forward matching `UiEvent`s.

use crate::ui::types::UiEvent;
use std::sync::mpsc;

/// Deserialize a `clarity_wire::WireMessage` JSON payload and forward the
/// matching `UiEvent` to `tx`.
pub fn dispatch_wire_payload(payload: &serde_json::Value, tx: &mpsc::Sender<UiEvent>) {
    let msg: clarity_wire::WireMessage = match serde_json::from_value(payload.clone()) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("Failed to deserialize WireMessage payload: {}", e);
            return;
        }
    };
    dispatch_wire_message(msg, tx);
}

/// Forward a decoded `clarity_wire::WireMessage` to `tx`.
pub fn dispatch_wire_message(msg: clarity_wire::WireMessage, tx: &mpsc::Sender<UiEvent>) {
    let event = match msg {
        clarity_wire::WireMessage::ContentPart { text, .. } => Some(UiEvent::Chunk(text)),
        clarity_wire::WireMessage::DraftEvent { event, .. } => match event {
            clarity_wire::DraftEvent::Progress { text } => Some(UiEvent::DraftProgress { text }),
            clarity_wire::DraftEvent::Clear => Some(UiEvent::DraftClear),
            clarity_wire::DraftEvent::Content { text } => Some(UiEvent::DraftContent { text }),
        },
        clarity_wire::WireMessage::ToolCall {
            id,
            name,
            arguments,
            ..
        } => Some(UiEvent::ToolStart {
            id,
            name,
            arguments,
        }),
        clarity_wire::WireMessage::ToolResult { id, result, .. } => {
            Some(UiEvent::ToolResult { id, result })
        }
        clarity_wire::WireMessage::StepBegin { tool_name, .. } => {
            Some(UiEvent::StepBegin { tool_name })
        }
        clarity_wire::WireMessage::CompactionBegin { .. } => Some(UiEvent::CompactionBegin),
        clarity_wire::WireMessage::CompactionEnd { .. } => Some(UiEvent::CompactionEnd),
        clarity_wire::WireMessage::PlanStepBegin {
            step_id, tool_name, ..
        } => Some(UiEvent::PlanStepBegin { step_id, tool_name }),
        clarity_wire::WireMessage::PlanStepEnd {
            step_id, success, ..
        } => Some(UiEvent::PlanStepEnd { step_id, success }),
        clarity_wire::WireMessage::PlanStepSkipped { step_id, .. } => {
            Some(UiEvent::PlanStepSkipped { step_id })
        }
        clarity_wire::WireMessage::TurnBegin { user_input, .. } => {
            Some(UiEvent::TurnStart { user_input })
        }
        clarity_wire::WireMessage::TurnEnd { .. } => Some(UiEvent::TurnEnd),
        clarity_wire::WireMessage::StatusUpdate { message, .. } => {
            Some(UiEvent::StatusUpdate { message })
        }
        clarity_wire::WireMessage::ViewStateUpdate { turn, .. } => Some(UiEvent::ViewStateUpdate {
            turn: turn.map(Into::into),
        }),
        clarity_wire::WireMessage::ThreadActive {
            thread_id, title, ..
        } => Some(UiEvent::ThreadActive { thread_id, title }),
        clarity_wire::WireMessage::ThreadList { threads, .. } => {
            let sessions = threads
                .into_iter()
                .map(|t| crate::ui::types::Session {
                    id: t.thread_id,
                    title: t.title.unwrap_or_default(),
                    category: "engineering".to_string(),
                    project_id: None,
                    context: crate::ui::types::SessionContext::default(),
                    lifecycle: crate::ui::types::SessionLifecycle::default(),
                    archived: false,
                    messages: Vec::new(),
                    updated_at: crate::session::now_millis(),
                    turn_heights: Vec::new(),
                })
                .collect();
            Some(UiEvent::ThreadList { threads: sessions })
        }
        clarity_wire::WireMessage::ThreadCreated {
            thread_id, title, ..
        } => Some(UiEvent::ThreadCreated {
            session: crate::ui::types::Session {
                id: thread_id,
                title: title.unwrap_or_default(),
                category: "engineering".to_string(),
                project_id: None,
                context: crate::ui::types::SessionContext::default(),
                lifecycle: crate::ui::types::SessionLifecycle::default(),
                archived: false,
                messages: Vec::new(),
                updated_at: crate::session::now_millis(),
                turn_heights: Vec::new(),
            },
        }),
        clarity_wire::WireMessage::ThreadUpdated {
            thread_id,
            title,
            archived,
            ..
        } => Some(UiEvent::ThreadUpdated {
            thread_id,
            title,
            archived,
        }),
        clarity_wire::WireMessage::Usage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
            ..
        } => Some(UiEvent::Usage {
            prompt_tokens,
            completion_tokens,
            total_tokens,
        }),
    };
    if let Some(ev) = event {
        if let Err(e) = tx.send(ev) {
            tracing::warn!("Failed to send wire event: {}", e);
        }
    }
}
