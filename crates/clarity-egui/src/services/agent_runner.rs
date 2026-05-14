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
        // Clear any stale plan tracker / snapshot hint from a previous turn.
        self.chat_store.plan_tracker = None;
        self.chat_store.last_snapshot = None;

        // If currently streaming, steer: cancel the current turn and queue the
        // message for immediate send when the cancellation completes.
        if self.chat_store.is_loading {
            // Debounce: ignore rapid consecutive Enter presses while stopping.
            if self.chat_store.stopping {
                return;
            }
            self.chat_store.stopping = true;
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
            let enriched_query = if let Some(ref store) = state.memory_store {
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
            if let Some(ref store) = state.memory_store {
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
                        clarity_wire::WireMessage::PlanStepSkipped { step_id } => {
                            Some(UiEvent::PlanStepSkipped { step_id })
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
                .run_streaming(&enriched_query, move |chunk: &str| {
                    if let Err(e) = tx_chunk.send(UiEvent::Chunk(chunk.to_string())) {
                        tracing::warn!("Failed to send Chunk: {}", e);
                    }
                })
                .await;

            match result {
                Ok(final_response) => {
                    // Fire-and-forget: save a turn summary to long-term memory.
                    if let Some(ref store) = state.memory_store {
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
}
