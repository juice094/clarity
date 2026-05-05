use std::time::Instant;

use crate::stores::{ChatStore, SessionStore};
use crate::ui::types::{
    AgentStatus, ContentBlock, Message, Role, ToastLevel, ToolCallInfo, ToolCallStatus,
};

// TODO: decompose App dependency
pub fn on_done(app: &mut crate::App) {
    app.chat_store.is_loading = false;
    app.chat_store.agent_status = AgentStatus::Online;
    app.chat_store.stopping = false;
    app.state.agent.reset();
    app.save_current_session();
    // Auto-send any queued message.
    if let Some((text, attachments)) = app.chat_store.pending_send.take() {
        app.chat_store.input = text;
        app.chat_store.attachments = attachments;
        app.send();
    }
}

// TODO: decompose App dependency
pub fn on_error(app: &mut crate::App, msg: String) {
    app.chat_store.is_loading = false;
    app.chat_store.agent_status = AgentStatus::Online;
    app.chat_store.stopping = false;
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
        };
        m.prepare();
        session.messages.push(m);
    }
}

pub fn on_chunk(session_store: &mut SessionStore, text: String) {
    if let Some(session) = session_store.active_session_mut() {
        if let Some(last) = session.messages.last_mut() {
            if last.role == Role::Agent {
                last.content.push_str(&text);
                if let Some(ContentBlock::Text { text: ref mut t }) = last.blocks.last_mut() {
                    t.push_str(&text);
                } else {
                    last.blocks.push(ContentBlock::Text { text: text.clone() });
                }
                last.prepare();
                return;
            }
        }
        let mut msg = Message {
            role: Role::Agent,
            content: text.clone(),
            blocks: vec![ContentBlock::Text { text }],
            timestamp: Instant::now(),
            parsed: vec![],
            cached_height: None,
            is_error: false,
        };
        msg.prepare();
        session.messages.push(msg);
    }
}

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
                last.prepare();
                return;
            }
        }
        let mut msg = Message {
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
        };
        msg.prepare();
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
        let mut msg = Message {
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
        };
        msg.prepare();
        session.messages.push(msg);
    }
}

pub fn on_compaction_begin(chat_store: &mut ChatStore) {
    chat_store.compacting = true;
}

pub fn on_compaction_end(chat_store: &mut ChatStore) {
    chat_store.compacting = false;
}

pub fn on_usage(
    chat_store: &mut ChatStore,
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
) {
    chat_store.last_usage = Some((prompt_tokens, completion_tokens, total_tokens));
}

pub fn on_plan_ready(chat_store: &mut ChatStore, plan: clarity_core::agent::Plan) {
    chat_store.is_loading = false;
    chat_store.agent_status = AgentStatus::Online;
    chat_store.pending_plan = Some(plan);
}

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
