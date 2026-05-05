pub mod chat;
pub mod settings;
pub mod subagent;
pub mod system;
pub mod task;

use crate::ui::types::UiEvent;
use crate::App;

pub fn process_events(app: &mut App) {
    while let Ok(event) = app.ui_rx.try_recv() {
        match event {
            UiEvent::Chunk(text) => chat::on_chunk(&mut app.session_store, text),
            UiEvent::ToolStart {
                id,
                name,
                arguments,
            } => {
                chat::on_tool_start(
                    &mut app.session_store,
                    &mut app.chat_store,
                    id,
                    name,
                    arguments,
                );
            }
            UiEvent::ToolResult { id, result } => {
                let name = app
                    .chat_store
                    .tool_calls
                    .iter()
                    .find(|t| t.id == id)
                    .map(|t| t.name.clone())
                    .unwrap_or_else(|| "tool".to_string());
                chat::on_tool_result(
                    &mut app.session_store,
                    &mut app.chat_store,
                    id,
                    name,
                    result,
                );
            }
            UiEvent::StepBegin { tool_name } => {
                system::on_step_begin(tool_name);
            }
            UiEvent::CompactionBegin => chat::on_compaction_begin(&mut app.chat_store),
            UiEvent::CompactionEnd => chat::on_compaction_end(&mut app.chat_store),
            UiEvent::Done => chat::on_done(app),
            UiEvent::Error(msg) => chat::on_error(app, msg),
            UiEvent::Fallback { fallback, reason } => {
                system::on_fallback(&mut app.ui_store, fallback, reason);
            }
            UiEvent::TaskList(tasks) => task::on_task_list(&mut app.task_store, tasks),
            UiEvent::SubAgentBatch(batch_id, status) => {
                subagent::on_subagent_batch(&mut app.subagent_store, batch_id, status);
            }
            UiEvent::Usage {
                prompt_tokens,
                completion_tokens,
                total_tokens,
            } => chat::on_usage(
                &mut app.chat_store,
                prompt_tokens,
                completion_tokens,
                total_tokens,
            ),
            UiEvent::PlanReady(plan) => chat::on_plan_ready(&mut app.chat_store, plan),
            UiEvent::PlanStepBegin { step_id, tool_name } => {
                chat::on_plan_step_begin(&mut app.chat_store, step_id, tool_name);
            }
            UiEvent::PlanStepEnd { step_id, success } => {
                chat::on_plan_step_end(&mut app.chat_store, step_id, success);
            }
            UiEvent::ProviderTestResult {
                provider_id,
                success,
                error,
            } => settings::on_provider_test_result(
                &mut app.settings_store,
                &mut app.ui_store,
                provider_id,
                success,
                error,
            ),
            UiEvent::ProviderModelList {
                provider_id,
                models,
            } => settings::on_provider_model_list(
                &mut app.settings_store,
                &mut app.ui_store,
                provider_id,
                models,
            ),
            UiEvent::WebPageFetched {
                title,
                url,
                content,
            } => {
                chat::on_web_page_fetched(&mut app.ui_store, title, url, content);
            }
            UiEvent::ResolveApproval { req_id, response } => {
                system::on_resolve_approval(
                    app.state.mode_aware_approval_runtime.inner().clone(),
                    &app.runtime,
                    req_id,
                    response,
                );
            }
            UiEvent::McpReloaded {
                success,
                tools,
                message,
            } => {
                app.mcp_store.connected_tools = tools;
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
                app.settings_store.kimi_code_login_state = match state.as_str() {
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
                app.settings_store.kimi_code_login_state = if success {
                    crate::stores::KimiCodeLoginState::Success
                } else {
                    crate::stores::KimiCodeLoginState::Error(message.clone())
                };
                let level = if success {
                    crate::ui::types::ToastLevel::Info
                } else {
                    crate::ui::types::ToastLevel::Error
                };
                app.push_toast(message, level);
                if success {
                    app.settings_store.settings_edit.provider = provider_id.clone();
                    // Pick the first model from the provider's model list if currently empty
                    if app.settings_store.settings_edit.model.is_empty() {
                        if let Some(prov) = app.settings_store.provider_registry.get(&provider_id) {
                            if !prov.models.is_empty() {
                                app.settings_store.settings_edit.model = prov.models[0].clone();
                            }
                        }
                    }
                    app.save_settings_and_reload();
                }
            }
            UiEvent::SubagentStage { agent_id, name } => {
                subagent::on_subagent_stage(&mut app.subagent_store, agent_id, name);
            }
            UiEvent::SubagentOutput { agent_id, text } => {
                subagent::on_subagent_output(&mut app.subagent_store, agent_id, text);
            }
            UiEvent::SubagentStatus { agent_id, agent_type, status } => {
                subagent::on_subagent_status(&mut app.subagent_store, agent_id, agent_type, status);
            }
            UiEvent::SubagentComplete { agent_id, success } => {
                subagent::on_subagent_complete(&mut app.subagent_store, agent_id, success);
            }
            UiEvent::SubagentProgress { agent_id, steps, max_steps } => {
                subagent::on_subagent_progress(&mut app.subagent_store, agent_id, steps, max_steps);
            }
            UiEvent::GatewayHealth(status) => {
                app.chat_store.gateway_status = status;
            }
        }
    }
}
