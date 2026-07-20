pub mod chat;
pub mod cron;
pub mod session;
pub mod settings;
pub mod subagent;
pub mod system;
pub mod task;
pub mod team;

use crate::App;
use crate::stores::console::{ConsoleEntry, ConsoleLevel};
use crate::ui::types::UiEvent;

/// Dispatches queued UI events to handlers.
pub fn process_events(app: &mut App) {
    while let Ok(event) = app.ui_rx.try_recv() {
        match event {
            UiEvent::Chunk { session_id, text } => {
                {
                    let (session_store, chat_store) = app.chat_session_both_mut();
                    chat::on_chunk(session_store, chat_store, &session_id, text);
                }
                // Incrementally persist the session during long streams so a
                // crash before UiEvent::Done does not lose the entire response.
                const CHUNKS_PER_SAVE: usize = 10;
                if app.chat_store().chunks_since_save >= CHUNKS_PER_SAVE {
                    app.save_current_session();
                    app.chat_store_mut().chunks_since_save = 0;
                }
            }
            UiEvent::ToolStart {
                session_id,
                id,
                name,
                arguments,
            } => {
                let (session_store, chat_store) = app.chat_session_both_mut();
                chat::on_tool_start(session_store, chat_store, &session_id, id, name, arguments);
            }
            UiEvent::ToolResult {
                session_id,
                id,
                result,
                display_result,
            } => {
                let name = app
                    .chat_store()
                    .tool_calls
                    .iter()
                    .find(|t| t.id == id)
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| "tool".to_string());
                {
                    let (session_store, chat_store) = app.chat_session_both_mut();
                    chat::on_tool_result(
                        session_store,
                        chat_store,
                        &session_id,
                        id.clone(),
                        name.clone(),
                        result.clone(),
                        display_result.clone(),
                    );
                }
                // Push to console log.
                let is_error = result.contains("error") || result.contains("Error");
                app.context.console_store.push(ConsoleEntry {
                    timestamp: std::time::Instant::now(),
                    level: if is_error {
                        ConsoleLevel::Error
                    } else {
                        ConsoleLevel::ToolOutput
                    },
                    source: name,
                    message: result,
                    truncated: false,
                    source_pid: None,
                    ansi_styled: None,
                });
            }
            UiEvent::ToolCallProgress {
                session_id: _,
                index,
                name,
                arguments_so_far,
            } => {
                let label = if name.is_empty() {
                    format!("tool #{} assembling…", index)
                } else {
                    format!(
                        "⚙ {} #{} ({})",
                        name,
                        index,
                        crate::ui::truncate::truncate(&arguments_so_far, 80)
                    )
                };
                app.context.console_store.push(ConsoleEntry {
                    timestamp: std::time::Instant::now(),
                    level: ConsoleLevel::Status,
                    source: if name.is_empty() {
                        "tool".into()
                    } else {
                        name.clone()
                    },
                    message: label,
                    truncated: false,
                    source_pid: None,
                    ansi_styled: None,
                });
            }
            UiEvent::StepBegin {
                session_id,
                tool_name,
            } => {
                {
                    let (session_store, chat_store) = app.chat_session_mut();
                    let msg = format!("🔧 正在执行: {}…", tool_name);
                    chat::on_status_update(session_store, chat_store, &session_id, msg.clone());
                }
                let msg = format!("🔧 正在执行: {}…", tool_name);
                app.context.console_store.push(ConsoleEntry {
                    timestamp: std::time::Instant::now(),
                    level: ConsoleLevel::Status,
                    source: tool_name,
                    message: msg,
                    truncated: false,
                    source_pid: None,
                    ansi_styled: None,
                });
            }
            UiEvent::CompactionBegin { session_id } => {
                chat::on_compaction_begin(
                    &app.context.session_store,
                    &mut app.view_state,
                    &session_id,
                );
            }
            UiEvent::CompactionEnd { session_id } => {
                chat::on_compaction_end(
                    &app.context.session_store,
                    &mut app.view_state,
                    &session_id,
                );
            }
            UiEvent::DraftProgress { session_id, text } => {
                let (session_store, chat_store) = app.chat_session_mut();
                chat::on_draft_progress(session_store, chat_store, &session_id, text);
            }
            UiEvent::DraftClear { session_id } => {
                let (session_store, chat_store) = app.chat_session_mut();
                chat::on_draft_clear(session_store, chat_store, &session_id);
            }
            UiEvent::DraftContent { session_id, text } => {
                let (session_store, chat_store) = app.chat_session_mut();
                chat::on_draft_content(session_store, chat_store, &session_id, text);
            }
            UiEvent::ReasoningChunk { session_id, text } => {
                let (session_store, chat_store) = app.chat_session_both_mut();
                chat::on_reasoning_chunk(session_store, chat_store, &session_id, text);
            }
            UiEvent::TurnStart {
                session_id,
                user_input,
            } => {
                {
                    let (session_store, chat_store) = app.chat_session_mut();
                    chat::on_turn_start(session_store, chat_store, &session_id, user_input.clone());
                }
                app.context.console_store.push(ConsoleEntry {
                    timestamp: std::time::Instant::now(),
                    level: ConsoleLevel::Info,
                    source: "agent".into(),
                    message: format!("Turn started — {}", user_input),
                    truncated: false,
                    source_pid: None,
                    ansi_styled: None,
                });
            }
            UiEvent::TurnEnd { session_id } => chat::on_turn_end(app, &session_id),
            UiEvent::StatusUpdate {
                session_id,
                message,
            } => {
                app.context.console_store.push(ConsoleEntry {
                    timestamp: std::time::Instant::now(),
                    level: ConsoleLevel::Status,
                    source: "agent".into(),
                    message: message.clone(),
                    truncated: false,
                    source_pid: None,
                    ansi_styled: None,
                });
                {
                    let (session_store, chat_store) = app.chat_session_mut();
                    chat::on_status_update(session_store, chat_store, &session_id, message);
                }
            }
            UiEvent::ViewStateUpdate { session_id, turn } => {
                if app.context.session_store.active_session_id == session_id {
                    if let Some(turn) = turn {
                        app.view_state.turn = turn;
                    }
                }
            }
            UiEvent::ThreadActive { thread_id, .. } => {
                session::on_thread_active(&mut app.context.session_store, thread_id);
            }
            UiEvent::ThreadList { threads } => {
                session::on_thread_list(&mut app.context.session_store, threads);
            }
            UiEvent::ThreadCreated { session } => {
                session::on_thread_created(&mut app.context.session_store, session);
            }
            UiEvent::ThreadUpdated {
                thread_id,
                title,
                archived,
            } => {
                session::on_thread_updated(
                    &mut app.context.session_store,
                    thread_id,
                    title,
                    archived,
                );
            }
            UiEvent::ThreadDeleted { thread_id } => {
                session::on_thread_deleted(&mut app.context.session_store, thread_id);
            }
            UiEvent::SessionMeta {
                session_id,
                provider_state,
            } => {
                chat::on_session_meta(app, &session_id, provider_state);
            }
            UiEvent::Done { session_id } => chat::on_done(app, &session_id),
            UiEvent::Error {
                session_id,
                message,
            } => chat::on_error(app, &session_id, message),
            UiEvent::Fallback { fallback, reason } => {
                system::on_fallback(&mut app.context.ui_store, fallback, reason);
            }
            UiEvent::TaskList(tasks) => task::on_task_list(app.task_store_mut(), tasks),
            UiEvent::SubAgentBatch(batch_id, status) => {
                subagent::on_subagent_batch(app.subagent_store_mut(), batch_id, status);
            }
            UiEvent::Usage {
                session_id,
                prompt_tokens,
                completion_tokens,
                total_tokens,
            } => {
                let (session_store, chat_store) = app.chat_session_mut();
                chat::on_usage(
                    session_store,
                    chat_store,
                    &session_id,
                    prompt_tokens,
                    completion_tokens,
                    total_tokens,
                );
            }
            UiEvent::PlanReady(plan) => {
                let (chat_store, view_state) = app.chat_and_view_state_mut();
                chat::on_plan_ready(chat_store, view_state, plan);
            }
            UiEvent::PlanStepBegin {
                session_id,
                step_id,
                tool_name,
            } => {
                let (session_store, chat_store) = app.chat_session_mut();
                chat::on_plan_step_begin(
                    session_store,
                    chat_store,
                    &session_id,
                    step_id,
                    tool_name,
                );
            }
            UiEvent::PlanStepEnd {
                session_id,
                step_id,
                success,
            } => {
                let (session_store, chat_store) = app.chat_session_mut();
                chat::on_plan_step_end(session_store, chat_store, &session_id, step_id, success);
            }
            UiEvent::PlanSkip { step_id } => {
                let agent = app.context.state.agent.clone();
                app.context.runtime.spawn(async move {
                    if let Err(e) = agent.skip_plan_step(&step_id).await {
                        tracing::warn!("Failed to skip plan step {}: {}", step_id, e);
                    }
                });
            }
            UiEvent::PlanRetry { step_id } => {
                let agent = app.context.state.agent.clone();
                app.context.runtime.spawn(async move {
                    if let Err(e) = agent.retry_plan_step(&step_id).await {
                        tracing::warn!("Failed to retry plan step {}: {}", step_id, e);
                    }
                });
            }
            UiEvent::PlanStepSkipped {
                session_id,
                step_id,
            } => {
                let (session_store, chat_store) = app.chat_session_mut();
                chat::on_plan_step_skipped(session_store, chat_store, &session_id, step_id);
            }
            UiEvent::CronList(tasks) => {
                cron::on_cron_list(app.cron_store_mut(), tasks);
            }
            UiEvent::ProviderTestResult {
                provider_id,
                success,
                error,
            } => {
                let (settings_store, ui_store) = app.settings_and_ui_mut();
                settings::on_provider_test_result(
                    settings_store,
                    ui_store,
                    provider_id,
                    success,
                    error,
                );
            }
            UiEvent::ProviderModelList {
                provider_id,
                models,
            } => {
                let (settings_store, ui_store) = app.settings_and_ui_mut();
                settings::on_provider_model_list(settings_store, ui_store, provider_id, models);
            }
            UiEvent::WebPageFetched {
                title,
                url,
                content,
            } => {
                chat::on_web_page_fetched(&mut app.context.ui_store, title, url, content);
            }
            UiEvent::ResolveApproval { req_id, response } => {
                system::on_resolve_approval(
                    app.context
                        .state
                        .mode_aware_approval_runtime
                        .inner()
                        .clone(),
                    &app.context.runtime,
                    req_id,
                    response,
                );
            }
            UiEvent::McpReloaded {
                success,
                tools,
                message,
            } => {
                app.context.mcp_store.connected_tools = tools;
                if !message.is_empty() {
                    let level = if success {
                        crate::ui::types::ToastLevel::Info
                    } else {
                        crate::ui::types::ToastLevel::Error
                    };
                    app.push_toast(message, level);
                }
            }
            UiEvent::KimiCodeLoginStateUpdate {
                state,
                user_code,
                url,
                error,
            } => {
                app.settings_store_mut().kimi_code_login_state = match state.as_str() {
                    "waiting" => crate::stores::KimiCodeLoginState::Waiting {
                        user_code: user_code.unwrap_or_default(),
                        verification_uri: url.clone().unwrap_or_default(),
                        verification_uri_complete: url.unwrap_or_default(),
                    },
                    "polling" => crate::stores::KimiCodeLoginState::Polling,
                    "success" => crate::stores::KimiCodeLoginState::Success,
                    "error" => crate::stores::KimiCodeLoginState::Error(error.unwrap_or_default()),
                    _ => crate::stores::KimiCodeLoginState::Idle,
                };
            }
            UiEvent::KimiCodeLoginResult {
                success,
                message,
                provider_id,
            } => {
                {
                    let settings_store = app.settings_store_mut();
                    settings_store.kimi_code_login_state = if success {
                        crate::stores::KimiCodeLoginState::Success
                    } else {
                        crate::stores::KimiCodeLoginState::Error(message.clone())
                    };
                }
                let level = if success {
                    crate::ui::types::ToastLevel::Info
                } else {
                    crate::ui::types::ToastLevel::Error
                };
                app.push_toast(message, level);
                if success {
                    let model = {
                        let settings_store = app.settings_store();
                        if settings_store.settings_edit.model.is_empty() {
                            settings_store
                                .provider_registry
                                .get(&provider_id)
                                .and_then(|prov| prov.models.first().cloned())
                        } else {
                            None
                        }
                    };
                    let settings_store = app.settings_store_mut();
                    settings_store.settings_edit.provider = provider_id.clone();
                    if let Some(model) = model {
                        settings_store.settings_edit.model = model;
                    }
                    app.save_settings_and_reload();
                }
            }
            UiEvent::SubagentStage { agent_id, name } => {
                subagent::on_subagent_stage(app.subagent_store_mut(), agent_id, name);
            }
            UiEvent::SubagentOutput { agent_id, text } => {
                subagent::on_subagent_output(app.subagent_store_mut(), agent_id, text);
            }
            UiEvent::SubagentStatus {
                agent_id,
                agent_type,
                status,
            } => {
                subagent::on_subagent_status(
                    app.subagent_store_mut(),
                    agent_id,
                    agent_type,
                    status,
                );
            }
            UiEvent::SubagentComplete { agent_id, success } => {
                subagent::on_subagent_complete(app.subagent_store_mut(), agent_id, success);
            }
            UiEvent::SubagentProgress {
                agent_id,
                steps,
                max_steps,
            } => {
                subagent::on_subagent_progress(
                    app.subagent_store_mut(),
                    agent_id,
                    steps,
                    max_steps,
                );
            }
            UiEvent::GatewayHealth(status) => {
                app.chat_store_mut().gateway_status = status;
            }
            UiEvent::SnapshotRestored { id, success, error } => {
                app.view_state.turn = clarity_core::ui::TurnState::Idle;
                if success {
                    app.push_toast(
                        format!("Workspace restored to snapshot #{}", id),
                        crate::ui::types::ToastLevel::Info,
                    );
                } else {
                    app.push_toast(
                        format!(
                            "Restore failed: {}",
                            error.unwrap_or_else(|| "unknown error".to_string())
                        ),
                        crate::ui::types::ToastLevel::Error,
                    );
                }
            }
            UiEvent::TaskResultLoaded { task_id, result } => {
                if app.task_store().viewing_task_id.as_ref() == Some(&task_id) {
                    app.task_store_mut().viewing_task_result = Some(result);
                }
            }
            UiEvent::ShellResult {
                session_id,
                command,
                output,
                exit_code,
            } => {
                chat::on_shell_result(app, &session_id, command, output, exit_code);
            }
            UiEvent::KnowledgeLoaded { path, result } => {
                app.context.knowledge_store.loading = false;
                app.context.knowledge_store.bundle_path = path;
                match result {
                    Ok(bundle) => {
                        app.context.knowledge_store.set_bundle(bundle);
                    }
                    Err(err) => {
                        app.context.knowledge_store.error = Some(err);
                        app.context.knowledge_store.bundle = None;
                    }
                }
            }
            UiEvent::KnowledgeVaultEvents(events) => {
                if let Some(ref field) = app.context.knowledge_store.field {
                    if let Err(e) = field.apply_watcher_events(&events) {
                        tracing::warn!("Failed to apply vault watcher events {:?}: {}", events, e);
                    }
                }
            }
        }
    }
}
