use std::time::Instant;

use crate::stores::{ChatStore, SessionStore};
use crate::ui::types::{
    AgentStatus, ContentBlock, DraftStatus, Message, Role, ToastLevel, ToolCallInfo, ToolCallStatus,
};

/// Handles the draft progress event.
pub fn on_draft_progress(chat_store: &mut ChatStore, text: String) {
    chat_store.draft_status = DraftStatus::Progress { text };
}

/// Handles the draft clear event.
pub fn on_draft_clear(chat_store: &mut ChatStore) {
    chat_store.draft_status = DraftStatus::None;
}

/// Handles the draft content event (reasoning/thinking blocks).
pub fn on_draft_content(chat_store: &mut ChatStore, text: String) {
    // Accumulate reasoning content if multiple chunks arrive.
    match &mut chat_store.draft_status {
        DraftStatus::Content { text: existing } => existing.push_str(&text),
        _ => chat_store.draft_status = DraftStatus::Content { text },
    }
}

/// Handles a reasoning chunk from the backend.
///
/// Reasoning content is appended to a `ContentBlock::Think` in the current
/// assistant message so it survives the end of the turn as a collapsible panel.
pub fn on_reasoning_chunk(
    session_store: &mut SessionStore,
    _chat_store: &mut ChatStore,
    text: String,
) {
    if text.is_empty() {
        return;
    }

    if let Some(session) = session_store.active_session_mut() {
        if let Some(last) = session.messages.last_mut() {
            if last.role == Role::Agent {
                if let Some(ContentBlock::Think { steps }) = last.blocks.last_mut() {
                    if let Some(step) = steps.last_mut() {
                        step.push_str(&text);
                    } else {
                        steps.push(text);
                    }
                } else {
                    last.blocks.push(ContentBlock::Think { steps: vec![text] });
                }
                last.cached_height = None;
                return;
            }
        }
        let msg = Message {
            role: Role::Agent,
            content: String::new(),
            blocks: vec![ContentBlock::Think { steps: vec![text] }],
            timestamp: Instant::now(),
            parsed: vec![],
            cached_height: None,
            is_error: false,
            lines: Vec::new(),
        };
        session.messages.push(msg);
    }
}

/// Handles the turn start event.
pub fn on_turn_start(_chat_store: &mut ChatStore, _user_input: String) {
    // Reserved for telemetry, turn attribution, or marking the start of a new
    // streamed response. The user message has already been inserted locally.
}

/// Handles the turn end event.
pub fn on_turn_end(app: &mut crate::App) {
    // Delegate to the existing done handler for now. Over time this should
    // become the canonical turn-completion signal and Done can be retired.
    on_done(app);
}

/// Handles the backend status update event.
pub fn on_status_update(chat_store: &mut ChatStore, message: String) {
    chat_store.status_message = Some(message);
}

/// Handles the done event.
pub fn on_done(app: &mut crate::App) {
    app.view_state.turn = clarity_core::ui::TurnState::Idle;
    app.chat_store.agent_status = AgentStatus::Online;
    app.chat_store.draft_status = DraftStatus::None;
    app.chat_store.status_message = None;
    app.chat_store.chunks_since_save = 0;
    app.state.agent.reset();
    // Trigger deferred markdown parse now that streaming is complete.
    if let Some(session) = app.session_store.active_session_mut() {
        if let Some(last) = session.messages.last_mut() {
            if last.role == Role::Agent {
                last.prepare();
            }
        }
    }
    app.save_current_session();
    // Capture the latest snapshot created by this turn.
    let snapshots = app.state.agent.snapshot_list();
    app.chat_store.last_snapshot = snapshots.last().cloned();
    // Auto-send any queued message.
    if let Some((text, attachments)) = app.chat_store.pending_send.take() {
        app.chat_store.input = text;
        app.chat_store.attachments = attachments;
        app.send();
    }
}

/// Handles the error event.
pub fn on_error(app: &mut crate::App, msg: String) {
    app.view_state.turn = clarity_core::ui::TurnState::Idle;
    app.chat_store.agent_status = AgentStatus::Online;
    app.chat_store.draft_status = DraftStatus::None;
    app.chat_store.status_message = None;
    crate::handlers::system::push_toast(&mut app.ui_store, &msg, ToastLevel::Error);
    // Release queued message back to input so user can retry.
    if let Some((text, mut attachments)) = app.chat_store.pending_send.take() {
        if app.chat_store.input.is_empty() {
            app.chat_store.input = text;
        } else {
            app.chat_store.input.push('\n');
            app.chat_store.input.push_str(&text);
        }
        app.chat_store.attachments.append(&mut attachments);
    }
    if let Some(session) = app.session_store.active_session_mut() {
        let mut m = Message {
            role: Role::Agent,
            content: msg.clone(),
            blocks: vec![ContentBlock::Text { text: msg.clone() }],
            timestamp: Instant::now(),
            parsed: vec![],
            cached_height: None,
            is_error: true,
            lines: Vec::new(),
        };
        m.prepare();
        session.messages.push(m);
    }
}

/// Handles the chunk event.
pub fn on_chunk(session_store: &mut SessionStore, chat_store: &mut ChatStore, text: String) {
    // Real content has arrived — clear any transient draft/status indicator.
    chat_store.draft_status = DraftStatus::None;
    chat_store.status_message = None;
    chat_store.chunks_since_save += 1;

    if let Some(session) = session_store.active_session_mut() {
        if let Some(last) = session.messages.last_mut() {
            if last.role == Role::Agent {
                last.content.push_str(&text);
                if let Some(ContentBlock::Text { text: t }) = last.blocks.last_mut() {
                    t.push_str(&text);
                } else {
                    last.blocks.push(ContentBlock::Text { text: text.clone() });
                }
                // Deferred: prepare() will be called in on_done() after streaming ends.
                return;
            }
        }
        let msg = Message {
            role: Role::Agent,
            content: text.clone(),
            blocks: vec![ContentBlock::Text { text }],
            timestamp: Instant::now(),
            parsed: vec![],
            cached_height: None,
            is_error: false,
            lines: Vec::new(),
        };
        session.messages.push(msg);
    }
}

/// Handles the tool start event.
pub fn on_tool_start(
    session_store: &mut SessionStore,
    chat_store: &mut ChatStore,
    id: String,
    name: String,
    arguments: serde_json::Value,
) {
    chat_store.tool_calls.push(ToolCallInfo {
        id: id.clone(),
        name: name.clone(),
        status: ToolCallStatus::Running,
        result: Some(arguments.to_string()),
    });
    if let Some(session) = session_store.active_session_mut() {
        if let Some(last) = session.messages.last_mut() {
            if last.role == Role::Agent {
                last.blocks.push(ContentBlock::ToolCall {
                    id: id.clone(),
                    name,
                    args: arguments.to_string(),
                });
                return;
            }
        }
        let msg = Message {
            role: Role::Agent,
            content: String::new(),
            blocks: vec![ContentBlock::ToolCall {
                id: id.clone(),
                name,
                args: arguments.to_string(),
            }],
            timestamp: Instant::now(),
            parsed: vec![],
            cached_height: None,
            is_error: false,
            lines: Vec::new(),
        };
        session.messages.push(msg);
    }
}

/// Format tool result for display: extract structured data for known tools.
fn format_tool_output(name: &str, result: &str) -> (String, bool) {
    match name {
        "think" => {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(result) {
                if let Some(summary) = v.get("summary").and_then(|s| s.as_str()) {
                    return (summary.to_string(), false);
                }
            }
            let truncated = result.chars().count() > 2000;
            let out = if truncated {
                let t: String = result.chars().take(2000).collect();
                format!(
                    "{}\n... (truncated, {} chars total)",
                    t,
                    result.chars().count()
                )
            } else {
                result.to_string()
            };
            (out, truncated)
        }
        "glob" | "grep" => {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(result) {
                if let Some(arr) = v.as_array() {
                    if arr.len() > 20 {
                        let preview: Vec<String> = arr
                            .iter()
                            .take(5)
                            .filter_map(|item| item.as_str().map(|s| s.to_string()))
                            .collect();
                        let out = format!(
                            "Found {} results\n```\n{}\n... ({} more)\n```",
                            arr.len(),
                            preview.join("\n"),
                            arr.len() - 5
                        );
                        return (out, false);
                    }
                }
            }
            let truncated = result.chars().count() > 2000;
            let out = if truncated {
                let t: String = result.chars().take(2000).collect();
                format!(
                    "{}\n... (truncated, {} chars total)",
                    t,
                    result.chars().count()
                )
            } else {
                result.to_string()
            };
            (out, truncated)
        }
        _ => {
            let truncated = result.chars().count() > 2000;
            let out = if truncated {
                let t: String = result.chars().take(2000).collect();
                format!(
                    "{}\n... (truncated, {} chars total)",
                    t,
                    result.chars().count()
                )
            } else {
                result.to_string()
            };
            (out, truncated)
        }
    }
}

/// Handles the tool result event.
pub fn on_tool_result(
    session_store: &mut SessionStore,
    chat_store: &mut ChatStore,
    id: String,
    name: String,
    result: String,
) {
    if let Some(tc) = chat_store.tool_calls.iter_mut().find(|t| t.id == id) {
        tc.status = ToolCallStatus::Success;
        tc.result = Some(result.clone());
    }
    if let Some(session) = session_store.active_session_mut() {
        let (display_result, truncated) = format_tool_output(&name, &result);
        let msg = Message {
            role: Role::Agent,
            content: format!("🔧 **{}**\n```json\n{}\n```", name, display_result),
            blocks: vec![ContentBlock::ToolResult {
                name,
                args: None,
                output: display_result,
                truncated,
            }],
            timestamp: Instant::now(),
            parsed: vec![],
            cached_height: None,
            is_error: false,
            lines: Vec::new(),
        };
        session.messages.push(msg);
    }
}

/// Handles the compaction begin event.
pub fn on_compaction_begin(view_state: &mut clarity_core::ui::ViewState) {
    view_state.turn = clarity_core::ui::TurnState::Compacting;
}

/// Handles the compaction end event.
pub fn on_compaction_end(view_state: &mut clarity_core::ui::ViewState) {
    view_state.turn = clarity_core::ui::TurnState::Idle;
}

/// Handles the usage event.
pub fn on_usage(
    chat_store: &mut ChatStore,
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
) {
    chat_store.last_usage = Some((prompt_tokens, completion_tokens, total_tokens));
}

/// Handles the plan ready event.
pub fn on_plan_ready(
    chat_store: &mut ChatStore,
    view_state: &mut clarity_core::ui::ViewState,
    plan: clarity_core::agent::Plan,
) {
    view_state.turn = clarity_core::ui::TurnState::Idle;
    chat_store.agent_status = AgentStatus::Online;
    chat_store.pending_plan = Some(plan);
}

/// Handles the plan step begin event.
pub fn on_plan_step_begin(chat_store: &mut ChatStore, step_id: String, _tool_name: String) {
    if let Some(ref mut tracker) = chat_store.plan_tracker {
        for step in &mut tracker.steps {
            if step.id == step_id {
                step.status = crate::ui::types::PlanStepStatus::Running;
                break;
            }
        }
    }
}

/// Handles the plan step end event.
pub fn on_plan_step_end(chat_store: &mut ChatStore, step_id: String, success: bool) {
    if let Some(ref mut tracker) = chat_store.plan_tracker {
        for step in &mut tracker.steps {
            if step.id == step_id {
                step.status = if success {
                    crate::ui::types::PlanStepStatus::Success
                } else {
                    crate::ui::types::PlanStepStatus::Failed
                };
                break;
            }
        }
    }
}

/// Handles the plan step skipped event.
pub fn on_plan_step_skipped(chat_store: &mut ChatStore, step_id: String) {
    if let Some(ref mut tracker) = chat_store.plan_tracker {
        for step in &mut tracker.steps {
            if step.id == step_id {
                step.status = crate::ui::types::PlanStepStatus::Skipped;
                break;
            }
        }
    }
}

/// Handles the web page fetched event.
pub fn on_web_page_fetched(
    ui_store: &mut crate::stores::UiStore,
    title: String,
    url: String,
    content: String,
) {
    ui_store.preview_item = Some(crate::ui::types::PreviewItem::WebPage {
        title,
        url,
        content,
    });
}
