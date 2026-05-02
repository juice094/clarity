use std::sync::Arc;
use std::time::Instant;

use crate::app_state::ensure_llm;
use crate::session::now_millis;
use crate::ui::types::*;
use crate::App;

impl App {
    pub(crate) fn send(&mut self) {
        let text = self.chat_store.input.trim().to_string();
        if text.is_empty() && self.chat_store.attachments.is_empty() {
            return;
        }
        // Clear any stale plan tracker from a previous turn.
        self.chat_store.plan_tracker = None;

        // If currently streaming, steer: cancel the current turn and queue the
        // message for immediate send when the cancellation completes.
        //
        // FIXME-WEEK1-RISK: cancel() is cooperative; LLM API may block 1-3s before
        //   noticing the cancellation token. If agent is mid-tool-call, side-effects
        //   may already have occurred. Optimize: add visual "stopping..." state.
        // FIXME-WEEK1-RISK: Rapid consecutive Enter presses overwrite pending_send.
        //   Optimize: debounce (200ms) or append to pending_send instead of replace.
        if self.chat_store.is_loading {
            self.stop();
            self.chat_store.pending_send =
                Some((text, std::mem::take(&mut self.chat_store.attachments)));
            self.chat_store.input.clear();
            return;
        }

        let mut full_message = text.clone();
        for att in &self.chat_store.attachments {
            if let Ok(content) = std::fs::read_to_string(&att.path) {
                full_message.push_str(&format!(
                    "\n\n[File: {}]\n```\n{}\n```",
                    att.name, content
                ));
            } else {
                full_message.push_str(&format!(
                    "\n\n[File: {} (binary or unreadable)]",
                    att.name
                ));
            }
        }
        self.chat_store.attachments.clear();

        if let Some(session) = self.session_store.active_session_mut() {
            let mut msg = Message {
                role: Role::User,
                content: full_message.clone(),
                timestamp: Instant::now(),
                parsed: vec![],
                cached_height: None,
                is_error: false,
            };
            msg.prepare();
            session.messages.push(msg);
            session.updated_at = now_millis();
            // Auto-name session from first user message
            if session.title.starts_with("New ") {
                let trimmed = text.trim();
                session.title = if trimmed.chars().count() > 20 {
                    format!("{}…", trimmed.chars().take(20).collect::<String>())
                } else {
                    trimmed.to_string()
                };
            }
        }
        self.chat_store.input.clear();
        self.session_store
            .drafts
            .remove(&self.session_store.active_session_id);
        self.chat_store.is_loading = true;
        self.chat_store.agent_status = AgentStatus::Busy;
        self.chat_store.tool_calls.clear();

        let state = self.state.clone();
        let tx = self.ui_tx.clone();
        let query = full_message;

        // Plan mode command: /plan <query>
        if let Some(plan_query) = text.strip_prefix("/plan ") {
            let plan_query = plan_query.to_string();
            self.runtime.spawn(async move {
                if let Err(e) = ensure_llm(&state).await {
                    if let Err(err) = tx.send(UiEvent::Error(e.to_string())) {
                        tracing::warn!("Failed to send Error: {}", err);
                    }
                    return;
                }
                match state.agent.plan(plan_query).await {
                    Ok(plan) => {
                        if let Err(e) = tx.send(UiEvent::PlanReady(plan)) {
                            tracing::warn!("Failed to send PlanReady: {}", e);
                        }
                    }
                    Err(e) => {
                        if let Err(err) =
                            tx.send(UiEvent::Error(format!("Plan generation failed: {}", e)))
                        {
                            tracing::warn!("Failed to send Error: {}", err);
                        }
                    }
                }
                if let Err(e) = tx.send(UiEvent::Done) {
                    tracing::warn!("Failed to send Done: {}", e);
                }
            });
            return;
        }

        self.runtime.spawn(async move {
            if let Err(e) = ensure_llm(&state).await {
                if let Err(err) = tx.send(UiEvent::Error(e.to_string())) {
                    tracing::warn!("Failed to send Error: {}", err);
                }
                return;
            }

            let wire = Arc::new(clarity_wire::Wire::new());
            let agent = state.agent.clone().with_wire(wire.clone());

            let tx_wire = tx.clone();
            tokio::spawn(async move {
                let mut wire_ui = wire.ui_side(false);
                while let Some(msg) = wire_ui.recv().await {
                    let event = match msg {
                        clarity_wire::WireMessage::ToolCall {
                            id,
                            name,
                            arguments,
                        } => Some(UiEvent::ToolStart {
                            id,
                            name,
                            arguments,
                        }),
                        clarity_wire::WireMessage::ToolResult { id, result } => {
                            Some(UiEvent::ToolResult { id, result })
                        }
                        clarity_wire::WireMessage::StepBegin { tool_name } => {
                            Some(UiEvent::StepBegin { tool_name })
                        }
                        clarity_wire::WireMessage::CompactionBegin => {
                            Some(UiEvent::CompactionBegin)
                        }
                        clarity_wire::WireMessage::CompactionEnd => Some(UiEvent::CompactionEnd),
                        clarity_wire::WireMessage::PlanStepBegin { step_id, tool_name } => {
                            Some(UiEvent::PlanStepBegin { step_id, tool_name })
                        }
                        clarity_wire::WireMessage::PlanStepEnd { step_id, success } => {
                            Some(UiEvent::PlanStepEnd { step_id, success })
                        }
                        clarity_wire::WireMessage::Usage {
                            prompt_tokens,
                            completion_tokens,
                            total_tokens,
                        } => Some(UiEvent::Usage {
                            prompt_tokens,
                            completion_tokens,
                            total_tokens,
                        }),
                        _ => None,
                    };
                    if let Some(ev) = event {
                        if let Err(e) = tx_wire.send(ev) {
                            tracing::warn!("Failed to send wire event: {}", e);
                        }
                    }
                }
            });

            let tx_chunk = tx.clone();
            let result = agent
                .run_streaming(&query, move |chunk: &str| {
                    if let Err(e) = tx_chunk.send(UiEvent::Chunk(chunk.to_string())) {
                        tracing::warn!("Failed to send Chunk: {}", e);
                    }
                })
                .await;

            match result {
                Ok(_) => {
                    if let Err(e) = tx.send(UiEvent::Done) {
                        tracing::warn!("Failed to send Done: {}", e);
                    }
                }
                Err(clarity_core::AgentError::Cancelled) => {
                    if let Err(e) = tx.send(UiEvent::Done) {
                        tracing::warn!("Failed to send Done: {}", e);
                    }
                }
                Err(e) => {
                    if let Err(err) = tx.send(UiEvent::Error(format!("Agent error: {}", e))) {
                        tracing::warn!("Failed to send Error: {}", err);
                    }
                }
            }
        });
    }
}
