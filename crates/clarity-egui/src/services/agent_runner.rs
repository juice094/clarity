use std::sync::Arc;
use std::time::Instant;

use crate::App;
use crate::app_state::ensure_llm;
use crate::services::wire_dispatcher::dispatch_wire_message;
use crate::session::now_millis;
use crate::ui::types::*;

impl App {
    /// Execute a shell command directly (!cmd), bypassing the LLM entirely.
    pub(crate) fn execute_shell_direct(&mut self, cmd: String) {
        let tx = self.ui_tx.clone();
        let working_dir = self.state.agent.config().working_dir.clone();

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
                        command: cmd,
                        output: combined,
                        exit_code,
                    });
                }
                Err(e) => {
                    let _ = tx.send(UiEvent::ShellResult {
                        command: cmd,
                        output: format!("Failed to execute: {}", e),
                        exit_code: -1,
                    });
                }
            }
        });
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
        let provider_chat_only = self
            .settings_store
            .provider_registry
            .get(&selected_provider)
            .map(|p| p.is_chat_only())
            .unwrap_or(false);

        // When an OpenClaw remote bot is selected AND the active session is a Claw
        // session, route the main chat message through the WebSocket gateway.
        // Chat/Work sessions must keep using the local agent even if a Claw bot
        // happens to be selected, avoiding mode confusion.
        if self.is_claw_active() && is_claw_session {
            // Guard against rapid Enter presses while a Claw turn is already in
            // flight. Unlike the local agent we cannot cancel a remote turn, so
            // we simply drop the duplicate send.
            if !is_turn_idle_for_send(&self.view_state.turn) {
                return;
            }
            self.send_claw();
            return;
        }

        // Chat-only providers (e.g. deepseek-device) are intentionally unavailable
        // in Work sessions because they do not support workspace tools.
        if is_work_session && provider_chat_only {
            self.push_toast(
                "The selected provider is only available in Chat sessions.",
                ToastLevel::Warn,
            );
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
            if let Ok(content) = std::fs::read_to_string(&att.path) {
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
        self.view_state.turn = clarity_core::ui::TurnState::Loading;
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
            self.runtime.spawn(async move {
                if let Err(e) = ensure_llm(&state).await {
                    if let Err(err) = tx.send(UiEvent::Error(e.to_string())) {
                        tracing::warn!("Failed to send Error: {}", err);
                    }
                    return;
                }
                let registry = state.agent.registry().clone();
                let working_dir = state.agent.config().working_dir.clone();
                let llm = match state.agent.llm() {
                    Some(llm) => llm,
                    None => {
                        if let Err(err) = tx.send(UiEvent::Error("No LLM configured".to_string())) {
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
                        if let Err(e) = tx.send(UiEvent::Chunk(content)) {
                            tracing::warn!("Failed to send Chunk: {}", e);
                        }
                        if let Err(e) = tx.send(UiEvent::Done) {
                            tracing::warn!("Failed to send Done: {}", e);
                        }
                    }
                    Err(e) => {
                        if let Err(err) = tx.send(UiEvent::Error(format!(
                            "Subagent /{} failed: {}",
                            agent_type_string, e
                        ))) {
                            tracing::warn!("Failed to send Error: {}", err);
                        }
                    }
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

            // Retrieve relevant long-term memories and enrich the query.
            let enriched_query = if let Some(store) = state.memory_store.get() {
                match store.search_fulltext(&query, 5).await {
                    Ok(facts) if !facts.is_empty() => {
                        let memory_context = facts
                            .iter()
                            .map(|f| format!("- {}", f.fact))
                            .collect::<Vec<_>>()
                            .join("\n");
                        tracing::debug!(
                            "Injecting {} relevant memory facts into query",
                            facts.len()
                        );
                        format!(
                            "{}\n\n[Relevant memories from past conversations]\n{}",
                            query, memory_context
                        )
                    }
                    _ => query,
                }
            } else {
                query
            };

            // Fire-and-forget: save the user query as a memory fact.
            if let Some(store) = state.memory_store.get() {
                let store = store.clone();
                let q = enriched_query.clone();
                tokio::spawn(async move {
                    if let Err(e) = store
                        .save_fact(
                            &q,
                            &["session".to_string(), "user_query".to_string()],
                            None,
                            None,
                        )
                        .await
                    {
                        tracing::debug!("Failed to save memory fact: {}", e);
                    }
                });
            }

            let wire = Arc::new(clarity_wire::Wire::new());
            let agent = state.agent.clone().with_wire(wire.clone());

            let tx_wire = tx.clone();
            tokio::spawn(async move {
                let mut wire_ui = wire.ui_side(false);
                while let Some(msg) = wire_ui.recv().await {
                    dispatch_wire_message(msg, &tx_wire);
                }
            });

            let result = agent.run_streaming(&enriched_query).await;

            match result {
                Ok(final_response) => {
                    // Fire-and-forget: save a turn summary to long-term memory.
                    if let Some(store) = state.memory_store.get() {
                        let store = store.clone();
                        let summary = format!(
                            "Turn summary —\nUser: {}\nAgent: {}",
                            enriched_query, final_response
                        );
                        tokio::spawn(async move {
                            if let Err(e) = store
                                .save_fact(
                                    &summary,
                                    &["session".to_string(), "turn".to_string()],
                                    None,
                                    None,
                                )
                                .await
                            {
                                tracing::debug!("Failed to save turn memory: {}", e);
                            }
                        });
                    }
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

        let mut full_message = text.clone();
        for att in &self.chat_store.attachments {
            if let Ok(content) = std::fs::read_to_string(&att.path) {
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
        self.chat_store.input.clear();
        self.session_store
            .drafts
            .remove(&self.session_store.active_session_id);
        self.view_state.turn = clarity_core::ui::TurnState::Loading;
        self.chat_store.agent_status = AgentStatus::Busy;
        self.chat_store.tool_calls.clear();

        let ws = match self.claw_ws {
            Some(ref ws) => ws.clone(),
            None => {
                let _ = self.ui_tx.send(UiEvent::Error(
                    "OpenClaw connection is not available".into(),
                ));
                return;
            }
        };

        // Resolve the session key and target device from the active session's
        // Claw context when available; fall back to the active bot for non-Claw
        // sessions so existing flows keep working.
        let (session_key, target_bot_id) = self
            .session_store
            .active_session()
            .map(|s| match &s.context {
                SessionContext::Claw {
                    role,
                    session_key,
                    affinity,
                } => {
                    let target = self.device_state.pick_instance(role, affinity);
                    if let Some(ref bot) = target {
                        // Make sure the active bot matches the picked instance
                        // before the message goes out.
                        if self.ui_store.active_bot_id != bot.id {
                            self.ui_store.active_bot_id = bot.id.clone();
                        }
                        self.device_state.set_last_picked(role, &bot.id);
                        (session_key.clone(), bot.id.clone())
                    } else {
                        (session_key.clone(), self.ui_store.active_bot_id.clone())
                    }
                }
                _ => (
                    "agent:main:main".to_string(),
                    self.ui_store.active_bot_id.clone(),
                ),
            })
            .unwrap_or_else(|| {
                (
                    "agent:main:main".to_string(),
                    self.ui_store.active_bot_id.clone(),
                )
            });

        // ponytail: in Stage 2 we route through the resolved target. The main
        // loop's connection manager will reconnect if the active bot changed.
        if target_bot_id.is_empty() {
            let _ = self
                .ui_tx
                .send(UiEvent::Error("No Claw device available".into()));
            return;
        }

        // Responses arrive asynchronously via claw_ws.drain(); the main loop
        // translates them into UiEvent::Chunk / UiEvent::Done and the chat
        // handlers finalize the assistant message.
        ws.send_chat(&session_key, &full_message, self.claw_ws_uses_sessions_send);
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
}
