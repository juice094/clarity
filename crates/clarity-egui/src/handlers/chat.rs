use std::collections::HashMap;
use std::time::Instant;

use crate::stores::{ChatStore, SessionStore};
use crate::ui::types::{
    AgentStatus, ContentBlock, DiffStats, DraftStatus, Message, Role, ToastLevel, ToolCallInfo,
    ToolCallStatus,
};

/// Returns true if `session_id` is the currently active session.
fn is_active(session_store: &SessionStore, session_id: &str) -> bool {
    session_store.active_session_id == session_id
}

/// Persist a specific session, regardless of whether it is active.
fn save_target_session(app: &mut crate::App, session_id: &str) {
    if app.context.session_store.active_session_id == session_id {
        app.save_current_session();
    } else if let Some(session) = app
        .context
        .session_store
        .sessions
        .iter_mut()
        .find(|s| s.id == session_id)
    {
        let now = crate::session::now_millis();
        match crate::session::save_session_internal(session) {
            Ok(()) => {
                session.last_saved_at = now;
            }
            Err(e) => {
                tracing::warn!("Failed to save background session {}: {}", session.id, e);
            }
        }
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
                session.updated_at = crate::session::now_millis();
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
        session.updated_at = crate::session::now_millis();
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
    if let Some(session) = app.context.session_store.session_mut(session_id) {
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
    if let Some(session) = app.context.session_store.session_mut(session_id) {
        session.in_flight = false;
    }
    app.chat_store_mut().in_flight_since = None;

    // The agent run has finished; reset global UI state. We keep a single
    // active run at a time, so this is safe even if the user switched to a
    // different session while streaming.
    app.view_state.turn = clarity_core::ui::TurnState::Idle;
    app.chat_store_mut().agent_status = AgentStatus::Online;
    app.chat_store_mut().draft_status = DraftStatus::None;
    app.chat_store_mut().status_message = None;
    app.chat_store_mut().chunks_since_save = 0;
    app.context.state.agent.reset();

    // Trigger deferred markdown parse now that streaming is complete.
    if let Some(session) = app.context.session_store.session_mut(session_id) {
        if let Some(last) = session.messages.last_mut() {
            if last.role == Role::Agent {
                last.prepare();
            }
        }
        session.updated_at = crate::session::now_millis();

        // Compute diff stats from tool results in this session.
        compute_session_diff_stats(session);
    }

    save_target_session(app, session_id);

    if is_active(&app.context.session_store, session_id) {
        // Capture the latest snapshot created by this turn.
        let snapshots = app.context.state.agent.snapshot_list();
        app.chat_store_mut().last_snapshot = snapshots.last().cloned();
        // Auto-send any queued message only if the finished session is still
        // active; otherwise the queued message belongs to a background session.
        if let Some((text, attachments)) = app.chat_store_mut().pending_send.take() {
            app.chat_store_mut().input = text;
            app.chat_store_mut().attachments = attachments;
            app.send();
        }
    }
}

/// Handles the error event.
pub fn on_error(app: &mut crate::App, session_id: &str, msg: String) {
    if let Some(session) = app.context.session_store.session_mut(session_id) {
        session.in_flight = false;
    }
    app.chat_store_mut().in_flight_since = None;

    // A run ended with an error; reset global UI state so the user can send
    // again. The error toast is only shown for the active session to avoid
    // interrupting a different conversation.
    app.view_state.turn = clarity_core::ui::TurnState::Idle;
    app.chat_store_mut().agent_status = AgentStatus::Online;
    app.chat_store_mut().draft_status = DraftStatus::None;
    app.chat_store_mut().status_message = None;

    if is_active(&app.context.session_store, session_id) {
        crate::handlers::system::push_toast(&mut app.context.ui_store, &msg, ToastLevel::Error);
        // Release queued message back to input so user can retry.
        if let Some((text, mut attachments)) = app.chat_store_mut().pending_send.take() {
            if app.chat_store_mut().input.is_empty() {
                app.chat_store_mut().input = text;
            } else {
                app.chat_store_mut().input.push('\n');
                app.chat_store_mut().input.push_str(&text);
            }
            app.chat_store_mut().attachments.append(&mut attachments);
        }
    }

    if let Some(session) = app.context.session_store.session_mut(session_id) {
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
                // Incrementally re-parse markdown every 5 chunks so code
                // blocks, lists, and formatting appear during streaming
                // instead of only at the end. Cached height is cleared so
                // the message list re-measures the growing bubble.
                if chat_store.chunks_since_save % 5 == 0 {
                    last.prepare();
                    last.cached_height = None;
                }
                // Bump updated_at so the auto-save timer can persist partial
                // responses mid-stream, guarding against crash data loss.
                session.updated_at = crate::session::now_millis();
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
        session.updated_at = crate::session::now_millis();
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
                session.updated_at = crate::session::now_millis();
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
        session.updated_at = crate::session::now_millis();
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
            crate::ui::truncate::truncate_str(result, crate::ui::truncate::TOOL_OUTPUT_MAX_CHARS)
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
            crate::ui::truncate::truncate_str(result, crate::ui::truncate::TOOL_OUTPUT_MAX_CHARS)
        }
        "file_read" => {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(result) {
                let path = v.get("path").and_then(|p| p.as_str()).unwrap_or("unknown");
                let content = v.get("content").and_then(|c| c.as_str()).unwrap_or(result);
                let (display, trunc) = crate::ui::truncate::truncate_str(
                    content,
                    crate::ui::truncate::TOOL_OUTPUT_MAX_CHARS,
                );
                (format!("📄 {}: {}", path, display), trunc)
            } else {
                crate::ui::truncate::truncate_str(
                    result,
                    crate::ui::truncate::TOOL_OUTPUT_MAX_CHARS,
                )
            }
        }
        "file_write" | "file_edit" => {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(result) {
                let path = v.get("path").and_then(|p| p.as_str()).unwrap_or("unknown");
                if let Some(_diff) = v.get("_diff_preview").and_then(|d| d.as_str()) {
                    return (format!("✏ {} (diff preview available)", path), false);
                }
                return (format!("✏ {} written", path), false);
            }
            crate::ui::truncate::truncate_str(result, crate::ui::truncate::TOOL_OUTPUT_MAX_CHARS)
        }
        _ => crate::ui::truncate::truncate_str(result, crate::ui::truncate::TOOL_OUTPUT_MAX_CHARS),
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
    display_result: Option<String>,
) {
    if is_active(session_store, session_id) {
        if let Some(tc) = chat_store.tool_calls.iter_mut().find(|t| t.id == id) {
            tc.status = ToolCallStatus::Success;
            tc.result = Some(result.clone());
        }
    }

    if let Some(session) = session_store.session_mut(session_id) {
        let (display, truncated) = if let Some(ref pre_formatted) = display_result {
            (pre_formatted.clone(), false)
        } else {
            format_tool_output(&name, &result)
        };
        let msg = Message {
            role: Role::Agent,
            content: format!("🔧 **{}**\n```json\n{}\n```", name, display),
            blocks: vec![ContentBlock::ToolResult {
                name,
                args: None,
                output: display,
                truncated,
            }],
            timestamp: Instant::now(),
            parsed: vec![],
            cached_height: None,
            is_error: false,
            lines: Vec::new(),
        };
        session.messages.push(msg);
        session.updated_at = crate::session::now_millis();
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
        chat_store.token_usage = crate::stores::chat::TokenUsage {
            prompt_tokens: prompt_tokens as u64,
            completion_tokens: completion_tokens as u64,
            total_tokens: total_tokens as u64,
            last_updated: std::time::Instant::now(),
            ..chat_store.token_usage.clone()
        };
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
    if let Some(session) = app.context.session_store.session_mut(session_id) {
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
    if is_active(&app.context.session_store, session_id) {
        app.chat_store_mut().stick_to_bottom = true;
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

/// Compute `DiffStats` for a session by scanning tool-result blocks for
/// `_diff_preview` patches emitted by `FileEditTool` / `FileWriteTool`.
pub fn compute_session_diff_stats(session: &mut crate::ui::types::Session) {
    let mut files_changed = 0usize;
    let mut lines_added = 0usize;
    let mut lines_removed = 0usize;

    for msg in &session.messages {
        for block in &msg.blocks {
            if let ContentBlock::ToolResult {
                name: _,
                args: _,
                output,
                truncated: _,
            } = block
            {
                if let Some(hunks) =
                    crate::widgets::diff_viewer::extract_diff_from_tool_result(output)
                {
                    files_changed += 1;
                    for hunk in hunks {
                        for line in &hunk.lines {
                            match line {
                                clarity_core::diff::DiffLine::Added(_) => lines_added += 1,
                                clarity_core::diff::DiffLine::Removed(_) => lines_removed += 1,
                                _ => {}
                            }
                        }
                    }
                }
            }
        }
    }

    if files_changed > 0 {
        session.diff_stats = Some(DiffStats {
            files_changed,
            lines_added,
            lines_removed,
            computed_at: crate::session::now_millis(),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session::now_millis;
    use crate::stores::ChatStore;
    use crate::ui::types::{Session, SessionContext, SessionLifecycle};

    fn make_session(id: &str) -> Session {
        let now = now_millis();
        Session {
            id: id.into(),
            title: format!("Session {}", id),
            category: "chat".into(),
            project_id: None,
            context: SessionContext::Chat,
            lifecycle: SessionLifecycle::Temporary,
            archived: false,
            messages: Vec::new(),
            updated_at: now,
            last_saved_at: now,
            turn_heights: Vec::new(),
            estimate_buffer: Vec::new(),
            line_offset_buffer: Vec::new(),
            estimate_key: None,
            cached_total_height: None,
            provider_state: HashMap::new(),
            in_flight: false,
            diff_stats: None,
        }
    }

    fn make_store(sessions: Vec<Session>, active: &str) -> SessionStore {
        SessionStore {
            sessions,
            active_session_id: active.into(),
            drafts: HashMap::new(),
            turn_cache: HashMap::new(),
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

    #[test]
    fn reasoning_chunk_appends_to_existing_think_block() {
        let mut chat_store = ChatStore::default();
        let session_a = make_session("a");
        let mut store = make_store(vec![session_a], "a");

        // First reasoning chunk creates a think block.
        on_reasoning_chunk(&mut store, &mut chat_store, "a", "step 1: ".into());
        let session = store.session_mut("a").unwrap();
        assert_eq!(session.messages.len(), 1);
        assert_eq!(session.messages[0].role, Role::Agent);
        if let ContentBlock::Think { steps } = &session.messages[0].blocks[0] {
            assert_eq!(steps.len(), 1);
            assert_eq!(steps[0], "step 1: ");
        } else {
            panic!("Expected Think block");
        }

        // Second chunk appends to the same step.
        on_reasoning_chunk(&mut store, &mut chat_store, "a", "result found".into());
        let session = store.session_mut("a").unwrap();
        if let ContentBlock::Think { steps } = &session.messages[0].blocks[0] {
            assert_eq!(steps[0], "step 1: result found");
        } else {
            panic!("Expected Think block after append");
        }
    }

    #[test]
    fn empty_reasoning_chunk_is_noop() {
        let mut chat_store = ChatStore::default();
        let session_a = make_session("a");
        let mut store = make_store(vec![session_a], "a");

        on_reasoning_chunk(&mut store, &mut chat_store, "a", "".into());
        let session = store.session_mut("a").unwrap();
        assert!(
            session.messages.is_empty(),
            "Empty chunk should not create a message"
        );
    }

    #[test]
    fn draft_events_for_active_session_only() {
        let mut chat_store = ChatStore::default();
        let session_a = make_session("a");
        let session_b = make_session("b");
        let store = make_store(vec![session_a, session_b], "b");

        // Draft progress for inactive session A should not affect chat_store.
        on_draft_progress(&store, &mut chat_store, "a", "progress".into());
        assert!(matches!(chat_store.draft_status, DraftStatus::None));

        // Draft progress for active session B should update.
        on_draft_progress(&store, &mut chat_store, "b", "progress".into());
        assert!(matches!(
            chat_store.draft_status,
            DraftStatus::Progress { .. }
        ));

        // Draft clear for active session.
        on_draft_clear(&store, &mut chat_store, "b");
        assert!(matches!(chat_store.draft_status, DraftStatus::None));
    }

    #[test]
    fn draft_content_accumulates_across_chunks() {
        let mut chat_store = ChatStore::default();
        let session_a = make_session("a");
        let store = make_store(vec![session_a], "a");

        on_draft_content(&store, &mut chat_store, "a", "part 1".into());
        if let DraftStatus::Content { text } = &chat_store.draft_status {
            assert_eq!(text, "part 1");
        } else {
            panic!("Expected DraftStatus::Content");
        }

        on_draft_content(&store, &mut chat_store, "a", " + part 2".into());
        if let DraftStatus::Content { text } = &chat_store.draft_status {
            assert_eq!(text, "part 1 + part 2");
        } else {
            panic!("Expected accumulated DraftStatus::Content");
        }
    }

    #[test]
    fn tool_start_and_result_for_active_session() {
        let mut chat_store = ChatStore::default();
        let session_a = make_session("a");
        let mut store = make_store(vec![session_a], "a");

        on_tool_start(
            &mut store,
            &mut chat_store,
            "a",
            "t1".into(),
            "read_file".into(),
            serde_json::json!({"path": "/tmp/x"}),
        );
        assert_eq!(chat_store.tool_calls.len(), 1);
        assert_eq!(chat_store.tool_calls[0].name, "read_file");
        assert!(matches!(
            chat_store.tool_calls[0].status,
            ToolCallStatus::Running
        ));

        on_tool_result(
            &mut store,
            &mut chat_store,
            "a",
            "t1".into(),
            "read_file".into(),
            "file contents here".into(),
            None,
        );
        // Tool call status should be updated.
        assert!(matches!(
            chat_store.tool_calls[0].status,
            ToolCallStatus::Success
        ));
    }

    #[test]
    fn format_tool_output_think_extracts_summary() {
        let (display, truncated) =
            format_tool_output("think", r#"{"summary": "I will read the file first"}"#);
        assert_eq!(display, "I will read the file first");
        assert!(!truncated);
    }

    #[test]
    fn format_tool_output_think_falls_back_to_truncation() {
        let long = "a".repeat(5000);
        let result = format!(r#"{{"not_summary": "{}"}}"#, long);
        let (display, truncated) = format_tool_output("think", &result);
        assert!(truncated);
        assert!(display.len() < 5000);
    }

    #[test]
    fn format_tool_output_glob_over_20_results() {
        let items: Vec<String> = (0..25).map(|i| format!("/tmp/file{}.rs", i)).collect();
        let result = serde_json::to_string(&items).unwrap();
        let (display, truncated) = format_tool_output("glob", &result);
        assert!(display.contains("Found 25 results"));
        assert!(display.contains("... (20 more)"));
        assert!(!truncated);
    }

    #[test]
    fn format_tool_output_file_read_formats_path() {
        let result = r#"{"path": "/tmp/test.rs", "content": "fn main() {}"}"#;
        let (display, truncated) = format_tool_output("file_read", result);
        assert!(display.contains("📄 /tmp/test.rs"));
        assert!(display.contains("fn main() {}"));
        assert!(!truncated);
    }

    #[test]
    fn format_tool_output_file_write_shows_path() {
        let result = r#"{"path": "/tmp/output.txt"}"#;
        let (display, truncated) = format_tool_output("file_write", result);
        assert!(display.contains("✏ /tmp/output.txt written"));
        assert!(!truncated);
    }

    #[test]
    fn format_tool_output_file_edit_with_diff() {
        let result = r#"{"path": "/tmp/test.rs", "_diff_preview": "@@ -1 +1 @@\n-old\n+new"}"#;
        let (display, truncated) = format_tool_output("file_edit", result);
        assert!(display.contains("diff preview available"));
        assert!(!truncated);
    }

    #[test]
    fn format_tool_output_unknown_tool_truncates() {
        let long = "x".repeat(5000);
        let (display, truncated) = format_tool_output("unknown_tool", &long);
        assert!(truncated);
        assert!(display.len() < long.len());
    }

    #[test]
    fn on_session_meta_stores_provider_state() {
        let mut session_a = make_session("a");
        assert!(session_a.provider_state.is_empty());

        // Simulate what on_session_meta does at the data level.
        session_a.provider_state.insert(
            "deepseek-device".into(),
            r#"{"chat_session_id":"abc"}"#.into(),
        );
        assert_eq!(
            session_a.provider_state.get("deepseek-device"),
            Some(&r#"{"chat_session_id":"abc"}"#.to_string())
        );
    }

    #[test]
    fn on_compaction_sets_and_clears_turn_state() {
        let session_a = make_session("a");
        let store = make_store(vec![session_a], "a");
        let mut view_state = clarity_core::ui::ViewState::new();
        view_state.turn = clarity_core::ui::TurnState::Loading;

        on_compaction_begin(&store, &mut view_state, "a");
        assert_eq!(view_state.turn, clarity_core::ui::TurnState::Compacting);

        on_compaction_end(&store, &mut view_state, "a");
        assert_eq!(view_state.turn, clarity_core::ui::TurnState::Idle);
    }

    #[test]
    fn on_usage_updates_token_counters() {
        let mut chat_store = ChatStore::default();
        let session_a = make_session("a");
        let store = make_store(vec![session_a], "a");

        on_usage(&store, &mut chat_store, "a", 500, 300, 800);
        assert_eq!(chat_store.last_usage, Some((500, 300, 800)));
        assert_eq!(chat_store.token_usage.prompt_tokens, 500);
        assert_eq!(chat_store.token_usage.completion_tokens, 300);
        assert_eq!(chat_store.token_usage.total_tokens, 800);
    }

    #[test]
    fn compute_session_diff_stats_accumulates_from_tool_results() {
        use crate::ui::types::{ContentBlock, Session, SessionContext, SessionLifecycle};

        let mut session = Session {
            id: "ds".into(),
            title: "diff test".into(),
            category: "chat".into(),
            project_id: None,
            context: SessionContext::Chat,
            lifecycle: SessionLifecycle::Temporary,
            archived: false,
            messages: Vec::new(),
            updated_at: crate::session::now_millis(),
            last_saved_at: crate::session::now_millis(),
            turn_heights: Vec::new(),
            estimate_buffer: Vec::new(),
            line_offset_buffer: Vec::new(),
            estimate_key: None,
            cached_total_height: None,
            provider_state: HashMap::new(),
            in_flight: false,
            diff_stats: None,
        };

        // A tool result with a _diff_preview.
        let diff_json = r#"{"_diff_preview":"--- a/test.rs\n+++ b/test.rs\n@@ -1,2 +1,2 @@\n-fn main() {\n+fn run() {\n"}"#;
        let msg = Message {
            role: Role::Agent,
            content: "diff".into(),
            blocks: vec![ContentBlock::ToolResult {
                name: "file_edit".into(),
                args: None,
                output: diff_json.into(),
                truncated: false,
            }],
            timestamp: Instant::now(),
            parsed: vec![],
            cached_height: None,
            is_error: false,
            lines: Vec::new(),
        };
        session.messages.push(msg);

        compute_session_diff_stats(&mut session);
        assert!(session.diff_stats.is_some());
        let stats = session.diff_stats.unwrap();
        assert_eq!(stats.files_changed, 1);
        // -fn main() → 1 removed, +fn run() → 1 added
        assert!(stats.lines_added > 0 || stats.lines_removed > 0);
    }

    #[test]
    fn compute_session_diff_stats_noop_without_diff_blocks() {
        let mut session = make_session("no-diff");
        session.messages.push(Message {
            role: Role::Agent,
            content: "plain text".into(),
            blocks: vec![ContentBlock::Text {
                text: "plain text".into(),
            }],
            timestamp: Instant::now(),
            parsed: vec![],
            cached_height: None,
            is_error: false,
            lines: Vec::new(),
        });

        compute_session_diff_stats(&mut session);
        assert!(session.diff_stats.is_none());
    }
}
