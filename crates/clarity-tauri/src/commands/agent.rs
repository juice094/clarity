use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use clarity_core::{AgentError, OpenAiCompatibleLlm};

/// Simple echo command for smoke-testing the IPC bridge.
#[tauri::command]
pub fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust.", name)
}

/// Return the application version from Cargo.toml.
#[tauri::command]
pub fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Return the number of configured agents (0 = unconfigured, 1 = ready).
#[tauri::command]
pub fn get_agent_count(state: State<crate::AppState>) -> usize {
    if state.agent.llm().is_some() {
        1
    } else {
        0
    }
}

/// Run a single-turn agent query (non-streaming).
///
/// If no LLM is configured, attempts to auto-configure from the
/// `OPENAI_API_KEY` environment variable.
#[tauri::command]
pub async fn agent_run(
    query: String,
    state: State<'_, crate::AppState>,
) -> Result<String, String> {
    if state.agent.llm().is_none() {
        match OpenAiCompatibleLlm::from_env() {
            Ok(llm) => {
                let llm: Arc<dyn clarity_core::llm::LlmProvider> = Arc::new(llm);
                state.agent.set_llm(llm);
            }
            Err(_) => {
                return Err(
                    "LLM not configured. Please set OPENAI_API_KEY environment variable.".into(),
                );
            }
        }
    }

    match state.agent.run(&query).await {
        Ok(response) => Ok(response),
        Err(AgentError::Cancelled) => Ok("[Cancelled by user]".into()),
        Err(e) => Err(format!("Agent error: {}", e)),
    }
}

/// Run a single-turn agent query with streaming output.
///
/// Chunks are emitted via Tauri events (`agent:chunk`).
/// Completion is signaled by `agent:done`, errors by `agent:error`.
#[tauri::command]
pub async fn agent_run_streaming(
    query: String,
    app: AppHandle,
    state: State<'_, crate::AppState>,
) -> Result<(), String> {
    if state.agent.llm().is_none() {
        match OpenAiCompatibleLlm::from_env() {
            Ok(llm) => {
                let llm: Arc<dyn clarity_core::llm::LlmProvider> = Arc::new(llm);
                state.agent.set_llm(llm);
            }
            Err(_) => {
                return Err(
                    "LLM not configured. Please set OPENAI_API_KEY environment variable.".into(),
                );
            }
        }
    }

    let app_for_chunk = app.clone();
    let result = state
        .agent
        .run_streaming(&query, move |chunk: &str| {
            let _ = app_for_chunk.emit("agent:chunk", chunk.to_string());
        })
        .await;

    match result {
        Ok(_) => {
            let _ = app.emit("agent:done", ());
            Ok(())
        }
        Err(AgentError::Cancelled) => {
            let _ = app.emit("agent:done", "[Cancelled by user]");
            Ok(())
        }
        Err(e) => {
            let msg = format!("Agent error: {}", e);
            let _ = app.emit("agent:error", &msg);
            Err(msg)
        }
    }
}

/// Interrupt the currently running agent turn, if any.
#[tauri::command]
pub fn agent_interrupt(state: State<crate::AppState>) {
    state.agent.cancel();
}

/// Return a human-readable agent status string.
#[tauri::command]
pub fn get_agent_status(state: State<crate::AppState>) -> String {
    use clarity_core::agent::AgentState;
    match state.agent.state() {
        AgentState::Unconfigured => "unconfigured".into(),
        AgentState::Idle => "idle".into(),
        AgentState::Running { .. } => "running".into(),
        AgentState::Stalled => "stalled".into(),
    }
}
