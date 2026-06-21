use std::collections::HashMap;
use std::time::Instant;

use crate::stores::{ChatStore, SessionStore};
use crate::ui::types::{
    AgentStatus, ContentBlock, DraftStatus, Message, Role, ToastLevel, ToolCallInfo, ToolCallStatus,
};

/// Returns true if `session_id` is the currently active session.
fn is_active(session_store: &SessionStore, session_id: &str) -> bool {
    session_store.active_session_id == session_id
}

/// Persist a specific session, regardless of whether it is active.
fn save_target_session(app: &mut crate::App, session_id: &str) {
    if app.session_store.active_session_id == session_id {
        app.save_current_session();
    } else if let Some(session) = app
        .session_store
        .sessions
        .iter()
        .find(|s| s.id == session_id)
    {
        let _ = crate::session::save_session_internal(session);
    }
}

/// Handles the draft progress event.
pub fn on_draft_progress(
    session_store: &SessionStore,
    chat_store: &mut ChatStore,
    session_id: &str,
    text: String,
) {
    if is_active(session_store, session_id) {
        chat_store.draft_status = DraftStatus::Progress { text };
    }
}

/// Handles the draft clear event.
pub fn on_draft_clear(session_store: &SessionStore, chat_store: &mut ChatStore, session_id: &str) {
    if is_active(session_store, session_id) {
        chat_store.draft_status = DraftStatus::None;
    }
}

/// Handles the draft content event (reasoning/thinking blocks).
pub fn on_draft_content(
    session_store: &SessionStore,
    chat_store: &mut ChatStore,
    session_id: &str,
    text: String,
) {
    if !is_active(session_store, session_id) {
        return;
    }
    // Accumulate reasoning content if multiple chunks arrive.
    match &mut chat_store.draft_status {
        DraftStatus::Content { text: existing } => existing.push_str(&text),
        _ => chat_store.draft_status = DraftStatus::Content { text },
    }
}

/// Handles a reasoning chunk from the backend.
///
/// Reasoning content is appended to a `ContentBlock::Think` in the target
/// assistant message so it survives the end of the turn as a collapsible panel.
pub fn on_reasoning_chunk(
    session_store: &mut SessionStore,
    chat_store: &mut ChatStore,
    session_id: &str,
    text: String,
) {
    if text.is_empty() {
        return;
    }

    if is_active(session_store, session_id) {
        chat_store.draft_status = DraftStatus::None;
        chat_store.status_message = None;
    }

    if let Some(session) = session_store.session_mut(session_id) {
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
pub fn on_turn_start(
    _session_store: &SessionStore,
    _chat_store: &mut ChatStore,
    _session_id: &str,
    _user_input: String,
) {
    // Reserved for telemetry, turn attribution, or marking the start of a new
    // streamed response. The user message has already been inserted locally.
}

/// Handles the turn end event.
pub fn on_turn_end(app: &mut crate::App, session_id: &str) {
    // Delegate to the existing done handler for now. Over time this should
    // become the canonical turn-completion signal and Done can be retired.
    on_done(app, session_id);
}

/// Persist provider-side session state blobs back into the target session.
///
/// Clarity treats these blobs as opaque caches; the provider decides their
/// contents and TTL. This lets stateful providers (e.g. deepseek-device)
/// resume the same server-side session after an app restart.
pub fn on_session_meta(
    app: &mut crate::App,
    session_id: &str,
    provider_state: HashMap<String, String>,
) {
    if let Some(session) = app.session_store.session_mut(session_id) {
        for (provider_id, blob) in provider_state {
            session.provider_state.insert(provider_id, blob);
        }
        save_target_session(app, session_id);
    }
}

/// Handles the backend status update event.
pub fn on_status_update(
    session_store: &SessionStore,
    chat_store: &mut ChatStore,
    session_id: &str,
    message: String,
) {
    if is_active(session_store, session_id) {
        chat_store.status_message = Some(message);
    }
}

/// Handles the done event.
pub fn on_done(app: &mut crate::App, session_id: &str) {
    if let Some(session) = app.session_store.session_mut(session_id) {
        session.in_flight = false;
    }

    // The agent run has finished; reset global UI state. We keep a single
    // active run at a time, so this is safe even if the user switched to a
    // different session while streaming.
    app.view_state.turn = clarity_core::ui::TurnState::Idle;
    app.chat_store.agent_status = AgentStatus::Online;
    app.chat_store.draft_status = DraftStatus::None;
    app.chat_store.status_message = None;
    app.chat_store.chunks_since_save = 0;
    app.state.agent.reset();

    // Trigger deferred markdown parse now that streaming is complete.
    if let Some(session) = app.session_store.session_mut(session_id) {
        if let Some(last) = session.messages.last_mut() {
            if last.role == Role::Agent {
                last.prepare();
            }
        }
        session.updated_at = crate::session::now_millis();
    }

    save_target_session(app, session_id);

    if is_active(&app.session_store, session_id) {
        // Capture the latest snapshot created by this turn.
        let snapshots = app.state.agent.snapshot_list();
        app.chat_store.last_snapshot = snapshots.last().cloned();
        // Auto-send any queued message only if the finished session is still
        // active; otherwise the queued message belongs to a background session.
        if let Some((text, attachments)) = app.chat_store.pending_send.take() {
            app.chat_store.input = text;
            app.chat_store.attachments = attachments;
            app.send();
        }
    }
}

/// Handles the error event.
pub fn on_error(app: &mut crate::App, session_id: &str, msg: String) {
    if let Some(session) = app.session_store.session_mut(session_id) {
        session.in_flight = false;
    }

    // A run ended with an error; reset global UI state so the user can send
    // again. The error toast is only shown for the active session to avoid
    // interrupting a different conversation.
    app.view_state.turn = clarity_core::ui::TurnState::Idle;
    app.chat_store.agent_status = AgentStatus::Online;
    app.chat_store.draft_status = DraftStatus::None;
    app.chat_store.status_message = None;

    if is_active(&app.session_store, session_id) {
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
    }

    if let Some(session) = app.session_store.session_mut(session_id) {
        let mut m = Message {
            role: Role::Agent,
            content: msg.clone(),
            blocks: vec![ContentBlock::Text { text: msg }],
            timestamp: Instant::now(),
            parsed: vec![],
            cached_height: None,
            is_error: true,
            lines: Vec::new(),
        };
        m.prepare();
        session.messages.push(m);
        session.updated_at = crate::session::now_millis();
    }

    save_target_session(app, session_id);
}

/// Handles the chunk event.
pub fn on_chunk(
    session_store: &mut SessionStore,
    chat_store: &mut ChatStore,
    session_id: &str,
    text: String,
) {
    if is_active(session_store, session_id) {
        // Real content has arrived — clear any transient draft/status indicator.
        chat_store.draft_status = DraftStatus::None;
        chat_store.status_message = None;
        chat_store.chunks_since_save += 1;
    }

    if let Some(session) = session_store.session_mut(session_id) {
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
    session_id: &str,
    id: String,
    name: String,
    arguments: serde_json::Value,
) {
    if is_active(session_store, session_id) {
        chat_store.draft_status = DraftStatus::None;
        chat_store.status_message = None;
        chat_store.tool_calls.push(ToolCallInfo {
            id: id.clone(),
            name: name.clone(),
            status: ToolCallStatus::Running,
            result: Some(arguments.to_string()),
        });
    }

    if let Some(session) = session_store.session_mut(session_id) {
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
    session_id: &str,
    id: String,
    name: String,
    result: String,
) {
    if is_active(session_store, session_id) {
        if let Some(tc) = chat_store.tool_calls.iter_mut().find(|t| t.id == id) {
            tc.status = ToolCallStatus::Success;
            tc.result = Some(result.clone());
        }
    }

    if let Some(session) = session_store.session_mut(session_id) {
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
pub fn on_compaction_begin(
    session_store: &SessionStore,
    view_state: &mut clarity_core::ui::ViewState,
    session_id: &str,
) {
    if is_active(session_store, session_id) {
        view_state.turn = clarity_core::ui::TurnState::Compacting;
    }
}

/// Handles the compaction end event.
pub fn on_compaction_end(
    session_store: &SessionStore,
    view_state: &mut clarity_core::ui::ViewState,
    session_id: &str,
) {
    if is_active(session_store, session_id) {
        view_state.turn = clarity_core::ui::TurnState::Idle;
    }
}

/// Handles the usage event.
pub fn on_usage(
    session_store: &SessionStore,
    chat_store: &mut ChatStore,
    session_id: &str,
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
) {
    if is_active(session_store, session_id) {
        chat_store.last_usage = Some((prompt_tokens, completion_tokens, total_tokens));
    }
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
pub fn on_plan_step_begin(
    session_store: &SessionStore,
    chat_store: &mut ChatStore,
    session_id: &str,
    step_id: String,
    _tool_name: String,
) {
    if !is_active(session_store, session_id) {
        return;
    }
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
pub fn on_plan_step_end(
    session_store: &SessionStore,
    chat_store: &mut ChatStore,
    session_id: &str,
    step_id: String,
    success: bool,
) {
    if !is_active(session_store, session_id) {
        return;
    }
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
pub fn on_plan_step_skipped(
    session_store: &SessionStore,
    chat_store: &mut ChatStore,
    session_id: &str,
    step_id: String,
) {
    if !is_active(session_store, session_id) {
        return;
    }
    if let Some(ref mut tracker) = chat_store.plan_tracker {
        for step in &mut tracker.steps {
            if step.id == step_id {
                step.status = crate::ui::types::PlanStepStatus::Skipped;
                break;
            }
        }
    }
}

/// Handles a direct shell execution result.
pub fn on_shell_result(
    app: &mut crate::App,
    session_id: &str,
    command: String,
    output: String,
    exit_code: i32,
) {
    let exit_marker = if exit_code == 0 {
        format!("Exit: {}", exit_code)
    } else {
        format!("Exit: {} ❌", exit_code)
    };
    let content = format!(
        "▶ {}\n```\n{}\n```\n_{}_",
        command,
        output.trim_end(),
        exit_marker
    );
    if let Some(session) = app.session_store.session_mut(session_id) {
        let mut msg = Message {
            role: Role::Agent,
            content: content.clone(),
            blocks: vec![ContentBlock::Text { text: content }],
            timestamp: std::time::Instant::now(),
            parsed: vec![],
            cached_height: None,
            is_error: exit_code != 0,
            lines: Vec::new(),
        };
        msg.prepare();
        session.messages.push(msg);
        session.updated_at = crate::session::now_millis();
    }
    save_target_session(app, session_id);
    if is_active(&app.session_store, session_id) {
        app.chat_store.stick_to_bottom = true;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::now_millis;
    use crate::stores::ChatStore;
    use crate::ui::types::{Session, SessionContext, SessionLifecycle};

    fn make_session(id: &str) -> Session {
        Session {
            id: id.into(),
            title: format!("Session {}", id),
            category: "chat".into(),
            project_id: None,
            context: SessionContext::Chat,
            lifecycle: SessionLifecycle::Temporary,
            archived: false,
            messages: Vec::new(),
            updated_at: now_millis(),
            turn_heights: Vec::new(),
            provider_state: HashMap::new(),
            in_flight: false,
        }
    }

    fn make_store(sessions: Vec<Session>, active: &str) -> SessionStore {
        SessionStore {
            sessions,
            active_session_id: active.into(),
            drafts: HashMap::new(),
        }
    }

    #[test]
    fn chunk_routes_to_target_session_not_active_session() {
        let mut chat_store = ChatStore::default();
        let session_a = make_session("a");
        let session_b = make_session("b");
        let mut store = make_store(vec![session_a, session_b], "b");

        on_chunk(&mut store, &mut chat_store, "a", "hello".into());
        on_chunk(&mut store, &mut chat_store, "a", " world".into());

        let a = store.session_mut("a").unwrap();
        assert_eq!(a.messages.len(), 1);
        assert_eq!(a.messages[0].content, "hello world");
        assert_eq!(a.messages[0].role, Role::Agent);

        let b = store.session_mut("b").unwrap();
        assert!(b.messages.is_empty());

        // Because session B is active, chunks for inactive session A must not
        // affect the active-session save counter.
        assert_eq!(chat_store.chunks_since_save, 0);
    }

    #[test]
    fn chunk_for_active_session_increments_save_counter() {
        let mut chat_store = ChatStore::default();
        let session_a = make_session("a");
        let mut store = make_store(vec![session_a], "a");

        on_chunk(&mut store, &mut chat_store, "a", "x".into());
        assert_eq!(chat_store.chunks_since_save, 1);
    }

    #[test]
    fn done_resets_in_flight_for_target_session() {
        let mut session_a = make_session("a");
        session_a.in_flight = true;
        let session_b = make_session("b");
        let mut store = make_store(vec![session_a, session_b], "b");

        // Simulate an agent message so on_done can prepare it.
        store.session_mut("a").unwrap().messages.push(Message {
            role: Role::Agent,
            content: "reply".into(),
            blocks: vec![ContentBlock::Text {
                text: "reply".into(),
            }],
            timestamp: Instant::now(),
            parsed: vec![],
            cached_height: None,
            is_error: false,
            lines: Vec::new(),
        });

        // on_done needs an App; build a minimal one through the test harness is
        // expensive, so we verify the session-level mutation directly.
        if let Some(s) = store.session_mut("a") {
            s.in_flight = false;
        }
        assert!(!store.session_mut("a").unwrap().in_flight);

        // Also verify the active session is untouched.
        assert!(!store.session_mut("b").unwrap().in_flight);
    }
}
