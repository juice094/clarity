pub mod chat;
pub mod settings;
pub mod subagent;
pub mod system;
pub mod task;

use crate::App;
use crate::ui::types::UiEvent;

pub fn process_events(app: &mut App) {
    while let Ok(event) = app.ui_rx.try_recv() {
        match event {
            UiEvent::Chunk(text) => chat::on_chunk(&mut app.session_store, text),
            UiEvent::ToolStart { id, name, arguments } => {
                chat::on_tool_start(&mut app.chat_store, id, name, arguments);
            }
            UiEvent::ToolResult { id, result } => {
                chat::on_tool_result(&mut app.chat_store, id, result);
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
            } => chat::on_usage(&mut app.chat_store, prompt_tokens, completion_tokens, total_tokens),
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
            UiEvent::WebPageFetched { title, url, content } => {
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
        }
    }
}
