use std::sync::Arc;
use tauri::State;
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

/// Run a single-turn agent query.
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
