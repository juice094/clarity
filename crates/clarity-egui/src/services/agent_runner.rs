use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use crate::App;
use crate::app_state::ensure_llm;
use crate::services::wire_dispatcher::dispatch_wire_message;
use crate::session::now_millis;
use crate::ui::types::*;

/// Maximum text bytes to read from a user-attached file.
///
/// ponytail: hard cap prevents a huge attachment from blowing the LLM request
/// body. The user can still reference the filename; upgrade path is streaming
/// or chunked upload for genuinely large files.
const MAX_ATTACHMENT_BYTES: usize = 100_000;

/// Find the largest valid UTF-8 boundary at or before `byte_idx`.
///
/// ponytail: std `str::floor_char_boundary` is stable since 1.91; we are on
/// MSRV 1.85, so keep this tiny helper until we bump Rust.
fn floor_char_boundary(text: &str, byte_idx: usize) -> usize {
    let byte_idx = byte_idx.min(text.len());
    let mut idx = byte_idx;
    while idx > 0 && text.as_bytes()[idx] & 0b1100_0000 == 0b1000_0000 {
        idx -= 1;
    }
    idx
}

/// Read a text attachment, truncating to [`MAX_ATTACHMENT_BYTES`] if needed.
fn read_attachment_text(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    if content.len() <= MAX_ATTACHMENT_BYTES {
        Some(content)
    } else {
        let split = floor_char_boundary(&content, MAX_ATTACHMENT_BYTES);
        Some(format!("{}...[truncated]", &content[..split]))
    }
}

impl App {
    /// Execute a shell command directly (!cmd), bypassing the LLM entirely.
    pub(crate) fn execute_shell_direct(&mut self, cmd: String) {
        let tx = self.ui_tx.clone();
        let working_dir = self.state.agent.config().working_dir.clone();
        let session_id = self.session_store.active_session_id.clone();

        // Add the command as a user message immediately.
        if let Some(session) = self.session_store.active_session_mut() {
            let mut msg = Message {
                role: Role::User,
                content: format!("!{}", cmd),
                blocks: vec![],
                timestamp: Instant::now(),
                parsed: vec![],
                cached_height: None,
                is_error: false,
                lines: Vec::new(),
            };
            msg.prepare();
            session.messages.push(msg);
            session.updated_at = crate::session::now_millis();
        }
        self.save_current_session();
        self.chat_store.stick_to_bottom = true;

        self.runtime.spawn(async move {
            let (shell, arg) = if cfg!(target_os = "windows") {
                ("powershell", "-Command")
            } else {
                ("bash", "-c")
            };

            let result = tokio::process::Command::new(shell)
                .arg(arg)
                .arg(&cmd)
                .current_dir(&working_dir)
                .output()
                .await;

            match result {
                Ok(output) => {
                    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
                    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
                    let exit_code = output.status.code().unwrap_or(-1);
                    let combined = if stderr.is_empty() {
                        stdout
                    } else {
                        format!("{}\n[stderr]\n{}", stdout, stderr)
                    };
                    let _ = tx.send(UiEvent::ShellResult {
                        session_id: session_id.clone(),
                        command: cmd,
                        output: combined,
                        exit_code,
                    });
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::ShellResult {
                        session_id: session_id.clone(),
                        command: cmd,
                        output: format!("Failed to execute: {}", e),
                        exit_code: -1,
                    });
                }
            }
        });
    }

    /// Upgrade the active Chat session to a Work session bound to the currently
    /// selected project (if any).
    pub(crate) fn upgrade_active_session_to_work(&mut self) {
        let workspace_id = self.project_store.selected_project_id.clone();
        let has_workspace = workspace_id
            .as_ref()
            .and_then(|pid| {
                self.project_store
                    .projects
                    .iter()
                    .find(|p| &p.id == pid)
                    .map(|p| p.has_workspace)
            })
            .unwrap_or(true);

        let context = SessionContext::Work {
            workspace_id: workspace_id.clone(),
            has_workspace,
        };

        if let Some(session) = self.session_store.active_session_mut() {
            session.context = context;
            session.project_id = workspace_id;
            session.updated_at = crate::session::now_millis();
        }

        self.save_current_session();
        self.push_toast("Session upgraded to Work mode.", ToastLevel::Info);
    }

    /// Send.
    pub(crate) fn send(&mut self) {
        let text = self.chat_store.input.trim().to_string();
        if text.is_empty() && self.chat_store.attachments.is_empty() {
            return;
        }

        let active_context = self
            .session_store
            .active_session()
            .map(|s| s.context.clone());
        let is_claw_session = matches!(active_context, Some(SessionContext::Claw { .. }));
        let is_work_session = matches!(active_context, Some(SessionContext::Work { .. }));
        let selected_provider = self.settings_store.settings_edit.provider.clone();
        let provider_tools_available = self
            .settings_store
            .provider_registry
            .get(&selected_provider)
            .map(|p| p.supports_tools())
            .unwrap_or(true);

        // When the active session is a Claw session, route the main chat message
        // through the WebSocket gateway. Chat/Work sessions must keep using the
        // local agent even if a Claw bot happens to be selected, avoiding mode
        // confusion.
        if is_claw_session {
            // Guard against rapid Enter presses while a Claw turn is already in
            // flight. Unlike the local agent we cannot cancel a remote turn, so
            // we simply drop the duplicate send.
            if !is_turn_idle_for_send(&self.view_state.turn) {
                return;
            }
            self.send_claw();
            return;
        }

        // Work sessions require a provider that can drive workspace tools.
        if is_work_session && !provider_tools_available {
            self.push_toast(
                "The selected provider is only available in Chat sessions.",
                ToastLevel::Warn,
            );
            return;
        }

        // Command: /work — upgrade the current Chat session to a Work session.
        if text.trim() == "/work" {
            if !provider_tools_available {
                self.push_toast(
                    "The selected provider does not support workspace tools.",
                    ToastLevel::Warn,
                );
                self.chat_store.input.clear();
                return;
            }
            self.upgrade_active_session_to_work();
            self.chat_store.input.clear();
            self.chat_store.attachments.clear();
            return;
        }

        // Clear any stale plan tracker / snapshot hint from a previous turn.
        self.chat_store.plan_tracker = None;
        self.chat_store.last_snapshot = None;

        // If currently streaming, steer: cancel the current turn and queue the
        // message for immediate send when the cancellation completes.
        if self.view_state.turn == clarity_core::ui::TurnState::Loading {
            // Debounce: ignore rapid consecutive Enter presses while stopping.
            if self.view_state.turn == clarity_core::ui::TurnState::Stopping {
                return;
            }
            self.view_state.turn = clarity_core::ui::TurnState::Stopping;
            self.stop();
            self.chat_store.pending_send =
                Some((text, std::mem::take(&mut self.chat_store.attachments)));
            self.chat_store.input.clear();
            return;
        }

        let mut full_message = text.clone();
        for att in &self.chat_store.attachments {
            if let Some(content) = read_attachment_text(&att.path) {
                full_message.push_str(&format!("\n\n[File: {}]\n```\n{}\n```", att.name, content));
            } else {
                full_message.push_str(&format!("\n\n[File: {} (binary or unreadable)]", att.name));
            }
        }
        self.chat_store.attachments.clear();

        if let Some(session) = self.session_store.active_session_mut() {
            let mut msg = Message {
                role: Role::User,
                content: full_message.clone(),
                blocks: vec![],
                timestamp: Instant::now(),
                parsed: vec![],
                cached_height: None,
                is_error: false,
                lines: Vec::new(),
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
        let session_id = self.session_store.active_session_id.clone();
        if let Some(session) = self.session_store.session_mut(&session_id) {
            session.in_flight = true;
        }
        self.view_state.turn = clarity_core::ui::TurnState::Loading;
        self.chat_store.agent_status = AgentStatus::Busy;
        self.chat_store.tool_calls.clear();

        let state = self.state.clone();
        let tx = self.ui_tx.clone();
        let query = full_message;
        let provider_id = selected_provider.clone();
        let restored_state = self
            .session_store
            .active_session()
            .and_then(|s| s.provider_state.get(&provider_id).cloned());

        // Plan mode command: /plan <query>
        if let Some(plan_query) = text.strip_prefix("/plan ") {
            let plan_query = plan_query.to_string();
            let plan_session_id = session_id.clone();
            self.runtime.spawn(async move {
                if let Err(e) = ensure_llm(&state).await {
                    if let Err(err) = tx.send(UiEvent::Error {
                        session_id: plan_session_id,
                        message: e.to_string(),
                    }) {
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
                        if let Err(err) = tx.send(UiEvent::Error {
                            session_id: plan_session_id.clone(),
                            message: format!("Plan generation failed: {}", e),
                        }) {
                            tracing::warn!("Failed to send Error: {}", err);
                        }
                    }
                }
                if let Err(e) = tx.send(UiEvent::Done {
                    session_id: plan_session_id,
                }) {
                    tracing::warn!("Failed to send Done: {}", e);
                }
            });
            return;
        }

        // Subagent shortcuts: /coder <query> and /explore <query>
        let subagent_prefix = if text.starts_with("/coder ") {
            Some(("coder", "/coder "))
        } else if text.starts_with("/explore ") {
            Some(("explore", "/explore "))
        } else {
            None
        };

        if let Some((agent_type, prefix)) = subagent_prefix {
            let subagent_prompt = query.strip_prefix(prefix).unwrap_or(&query).to_string();
            let agent_type_string = agent_type.to_string();
            let subagent_session_id = session_id.clone();
            self.runtime.spawn(async move {
                if let Err(e) = ensure_llm(&state).await {
                    if let Err(err) = tx.send(UiEvent::Error {
                        session_id: subagent_session_id,
                        message: e.to_string(),
                    }) {
                        tracing::warn!("Failed to send Error: {}", err);
                    }
                    return;
                }
                let registry = state.agent.registry().clone();
                let working_dir = state.agent.config().working_dir.clone();
                let llm = match state.agent.llm() {
                    Some(llm) => llm,
                    None => {
                        if let Err(err) = tx.send(UiEvent::Error {
                            session_id: subagent_session_id,
                            message: "No LLM configured".to_string(),
                        }) {
                            tracing::warn!("Failed to send Error: {}", err);
                        }
                        return;
                    }
                };
                let context_dir = dirs::data_dir()
                    .map(|d| d.join("clarity").join("subagents"))
                    .unwrap_or_else(|| working_dir.join("subagents"));

                // ── IS-1 Sprint 30: progress channel for live UI tracking ──
                let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel(128);
                let ui_tx2 = tx.clone();
                let _recv_handle = tokio::spawn(async move {
                    use clarity_contract::subagent::SubagentProgressEvent;
                    while let Some(event) = progress_rx.recv().await {
                        match event {
                            SubagentProgressEvent::Stage { agent_id, name } => {
                                let _ = ui_tx2.send(UiEvent::SubagentStage { agent_id, name });
                            }
                            SubagentProgressEvent::Output { agent_id, text } => {
                                let _ = ui_tx2.send(UiEvent::SubagentOutput { agent_id, text });
                            }
                            SubagentProgressEvent::StatusChange {
                                agent_id,
                                agent_type,
                                status,
                            } => {
                                let status_str = match status {
                                    clarity_contract::subagent::SubagentStatus::Idle => "Idle",
                                    clarity_contract::subagent::SubagentStatus::Running => "Running",
                                    clarity_contract::subagent::SubagentStatus::Completed => {
                                        "Completed"
                                    }
                                    clarity_contract::subagent::SubagentStatus::Failed => "Failed",
                                }
                                .to_string();
                                let _ = ui_tx2.send(UiEvent::SubagentStatus {
                                    agent_id: agent_id.clone(),
                                    agent_type,
                                    status: status_str.clone(),
                                });
                                if status == clarity_contract::subagent::SubagentStatus::Completed
                                    || status == clarity_contract::subagent::SubagentStatus::Failed
                                {
                                    let _ = ui_tx2.send(UiEvent::SubagentComplete {
                                        agent_id,
                                        success: status
                                            == clarity_contract::subagent::SubagentStatus::Completed,
                                    });
                                }
                            }
                            SubagentProgressEvent::Progress {
                                agent_id,
                                steps,
                                max_steps,
                            } => {
                                let _ = ui_tx2.send(UiEvent::SubagentProgress {
                                    agent_id,
                                    steps,
                                    max_steps,
                                });
                            }
                        }
                    }
                });

                let runner = clarity_subagents::SubagentRunner::new(
                    registry,
                    &working_dir,
                    &context_dir,
                )
                .with_llm(llm)
                .with_progress_tx(progress_tx);
                let mut store = clarity_subagents::SubagentStore::new(&context_dir);
                let spec =
                    clarity_contract::subagent::RunSpec::new(&subagent_prompt, &subagent_prompt)
                        .with_type(&agent_type_string);
                match runner.run(spec, &mut store, None).await {
                    Ok(result) => {
                        let content = format!(
                            "🤖 **{}** subagent result\n\n{}",
                            agent_type_string, result.summary
                        );
                        if let Err(e) = tx.send(UiEvent::Chunk {
                            session_id: subagent_session_id.clone(),
                            text: content,
                        }) {
                            tracing::warn!("Failed to send Chunk: {}", e);
                        }
                        if let Err(e) = tx.send(UiEvent::Done {
                            session_id: subagent_session_id,
                        }) {
                            tracing::warn!("Failed to send Done: {}", e);
                        }
                    }
                    Err(e) => {
                        if let Err(err) = tx.send(UiEvent::Error {
                            session_id: subagent_session_id,
                            message: format!("Subagent /{} failed: {}", agent_type_string, e),
                        }) {
                            tracing::warn!("Failed to send Error: {}", err);
                        }
                    }
                }
            });
            return;
        }

        self.runtime.spawn(async move {
            if let Err(e) = ensure_llm(&state).await {
                if let Err(err) = tx.send(UiEvent::Error {
                    session_id: session_id.clone(),
                    message: e.to_string(),
                }) {
                    tracing::warn!("Failed to send Error: {}", err);
                }
                return;
            }

            // The LLM already receives the full conversation history from the agent,
            // and stateful providers (e.g. deepseek-device) keep server-side context.
            // Injecting full-text memory facts here duplicates that history and causes
            // noisy, token-heavy prompts, so we pass the query through unchanged.
            let enriched_query = query;

            // Restore provider-side session state for stateful providers (e.g.
            // deepseek-device) so a restart can continue the same server-side session.
            if let Some(llm) = state.agent.llm() {
                if let Some(ref blob) = restored_state {
                    llm.restore_provider_state(blob);
                }
            }

            let wire = Arc::new(clarity_wire::Wire::new());
            let agent = state.agent.clone().with_wire(wire.clone());

            let tx_wire = tx.clone();
            let wire_session_id = session_id.clone();
            // Subscribe the UI receiver *before* starting generation. The agent
            // sends wire messages synchronously into a tokio broadcast channel;
            // if no receiver exists yet, the first events (TurnBegin, ContentPart)
            // are dropped with "no receivers" and the chat UI never leaves the
            // typing indicator.
            // ponytail: the receiver handle is created here and moved into the
            // drain task so subscription happens in the spawning task, not inside
            // the spawned future where scheduling delay would race with streaming.
            let wire_ui = wire.ui_side(false);
            tokio::spawn(async move {
                let mut wire_ui = wire_ui;
                while let Some(msg) = wire_ui.recv().await {
                    dispatch_wire_message(msg, &wire_session_id, &tx_wire);
                }
            });

            let result = agent.run_streaming(&enriched_query).await;

            match result {
                Ok(_final_response) => {
                    // Capture any provider-side session state (e.g. deepseek-device's
                    // chat_session_id) so the next restart can resume the same
                    // server-side session. Send the updated blob back to the UI thread
                    // to be persisted in session.json.
                    if let Some(llm) = state.agent.llm() {
                        if let Some(blob) = llm.capture_provider_state() {
                            let mut provider_state = HashMap::new();
                            provider_state.insert(provider_id.clone(), blob);
                            if let Err(e) = tx.send(UiEvent::SessionMeta {
                                session_id: session_id.clone(),
                                provider_state,
                            }) {
                                tracing::warn!("Failed to send SessionMeta: {}", e);
                            }
                        }
                    }
                    if let Err(e) = tx.send(UiEvent::Done {
                        session_id: session_id.clone(),
                    }) {
                        tracing::warn!("Failed to send Done: {}", e);
                    }
                }
                Err(clarity_core::AgentError::Cancelled) => {
                    if let Err(e) = tx.send(UiEvent::Done {
                        session_id: session_id.clone(),
                    }) {
                        tracing::warn!("Failed to send Done: {}", e);
                    }
                }
                Err(e) => {
                    if let Err(err) = tx.send(UiEvent::Error {
                        session_id: session_id.clone(),
                        message: format!("Agent error: {}", e),
                    }) {
                        tracing::warn!("Failed to send Error: {}", err);
                    }
                }
            }
        });
    }

    /// Send the current composer input through the active OpenClaw WebSocket.
    ///
    /// This mirrors the local-agent send path: the user message is appended to
    /// the active session, the turn state becomes Loading, and the message is
    /// forwarded to the remote Gateway. Responses arrive asynchronously via
    /// `claw_ws.drain()` and are translated into `UiEvent`s in the main loop.
    pub(crate) fn send_claw(&mut self) {
        if !is_turn_idle_for_send(&self.view_state.turn) {
            return;
        }

        let text = self.chat_store.input.trim().to_string();
        if text.is_empty() && self.chat_store.attachments.is_empty() {
            return;
        }

        let session_id = self.session_store.active_session_id.clone();

        let mut full_message = text.clone();
        for att in &self.chat_store.attachments {
            if let Some(content) = read_attachment_text(&att.path) {
                full_message.push_str(&format!("\n\n[File: {}]\n```\n{}\n```", att.name, content));
            } else {
                full_message.push_str(&format!("\n\n[File: {} (binary or unreadable)]", att.name));
            }
        }
        self.chat_store.attachments.clear();

        if let Some(session) = self.session_store.active_session_mut() {
            let mut msg = Message {
                role: Role::User,
                content: full_message.clone(),
                blocks: vec![],
                timestamp: Instant::now(),
                parsed: vec![],
                cached_height: None,
                is_error: false,
                lines: Vec::new(),
            };
            msg.prepare();
            session.messages.push(msg);
            session.updated_at = crate::session::now_millis();
            if session.title.starts_with("New ") {
                let trimmed = text.trim();
                session.title = if trimmed.chars().count() > 20 {
                    format!("{}…", trimmed.chars().take(20).collect::<String>())
                } else {
                    trimmed.to_string()
                };
            }
        }
        if let Some(session) = self.session_store.session_mut(&session_id) {
            session.in_flight = true;
        }
        self.chat_store.input.clear();
        self.session_store
            .drafts
            .remove(&self.session_store.active_session_id);
        self.view_state.turn = clarity_core::ui::TurnState::Loading;
        self.chat_store.agent_status = AgentStatus::Busy;
        self.chat_store.tool_calls.clear();
        self.chat_store.claw_in_flight_session_id = Some(session_id.clone());

        let ws = match self.claw_ws {
            Some(ref ws) => ws.clone(),
            None => {
                if let Some(session) = self.session_store.session_mut(&session_id) {
                    session.in_flight = false;
                }
                self.chat_store.claw_in_flight_session_id = None;
                self.view_state.turn = clarity_core::ui::TurnState::Idle;
                let _ = self.ui_tx.send(UiEvent::Error {
                    session_id,
                    message: "OpenClaw connection is not available".into(),
                });
                return;
            }
        };

        // Resolve the session key from the active session's Claw context. The
        // target device is managed by the connection loop in main.rs; sending
        // only needs the session key. Non-Claw sessions must not reach this
        // path.
        let session_key = match self.session_store.active_session() {
            Some(Session {
                context:
                    SessionContext::Claw {
                        role,
                        session_key,
                        affinity,
                    },
                ..
            }) => {
                let target = self.device_state.pick_instance(role, affinity);
                match target {
                    Some(bot) => {
                        self.device_state.set_last_picked(role, &bot.id);
                        session_key.clone()
                    }
                    None => {
                        if let Some(session) = self.session_store.session_mut(&session_id) {
                            session.in_flight = false;
                        }
                        self.chat_store.claw_in_flight_session_id = None;
                        self.view_state.turn = clarity_core::ui::TurnState::Idle;
                        let _ = self.ui_tx.send(UiEvent::Error {
                            session_id,
                            message: "No Claw device available".into(),
                        });
                        return;
                    }
                }
            }
            _ => {
                if let Some(session) = self.session_store.session_mut(&session_id) {
                    session.in_flight = false;
                }
                self.chat_store.claw_in_flight_session_id = None;
                self.view_state.turn = clarity_core::ui::TurnState::Idle;
                let _ = self.ui_tx.send(UiEvent::Error {
                    session_id,
                    message: "Not a Claw session".into(),
                });
                return;
            }
        };

        // Responses arrive asynchronously via claw_ws.drain(); the main loop
        // translates them into UiEvent::Chunk / UiEvent::Done and the chat
        // handlers finalize the assistant message.
        ws.send_chat(&session_key, &full_message);
    }
}

/// Return true only when the turn state is safe to start a new user send.
///
/// This helper is intentionally a free function so it can be unit-tested
/// without constructing a full [`App`].
pub(crate) fn is_turn_idle_for_send(turn: &clarity_core::ui::TurnState) -> bool {
    matches!(turn, clarity_core::ui::TurnState::Idle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_turn_guard_allows_idle_only() {
        use clarity_core::ui::TurnState;
        assert!(is_turn_idle_for_send(&TurnState::Idle));
        assert!(!is_turn_idle_for_send(&TurnState::Loading));
        assert!(!is_turn_idle_for_send(&TurnState::Stopping));
        assert!(!is_turn_idle_for_send(&TurnState::Compacting));
        assert!(!is_turn_idle_for_send(&TurnState::Restoring));
    }

    #[test]
    fn test_attachment_text_truncates_to_cap() {
        let path = std::env::temp_dir().join(format!(
            "clarity-attachment-test-{}.txt",
            std::process::id()
        ));
        let huge = "x".repeat(MAX_ATTACHMENT_BYTES + 1_000);
        std::fs::write(&path, &huge).unwrap();

        let result = read_attachment_text(&path).unwrap();
        assert!(result.len() <= MAX_ATTACHMENT_BYTES + "...[truncated]".len());
        assert!(result.ends_with("...[truncated]"));

        std::fs::remove_file(&path).unwrap();
    }
}
