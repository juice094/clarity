use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};
use clarity_core::AgentError;

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

/// Ensure the agent has an LLM configured, initializing from GuiSettings if needed.
async fn ensure_llm(agent: &clarity_core::Agent) -> Result<(), String> {
    if agent.llm().is_some() {
        return Ok(());
    }

    let settings = crate::commands::settings::GuiSettings::load();
    let provider = settings.provider.as_str();

    let llm: Arc<dyn clarity_core::llm::LlmProvider> = match provider {
        "local" => {
            let model_path = settings
                .local_model_path
                .or_else(|| {
                    clarity_core::llm::resolve_local_model_path()
                        .map(|p| p.to_string_lossy().into_owned())
                })
                .ok_or_else(|| {
                    "No local model configured. Place .gguf in ~/models/ or set CLARITY_LOCAL_MODEL_PATH.".to_string()
                })?;

            let config = clarity_core::llm::LocalGgufConfig::new(&model_path)
                .with_tokenizer_repo("Qwen/Qwen2.5-7B-Instruct");
            let provider = clarity_core::llm::LocalGgufProvider::new(config)
                .await
                .map_err(|e| format!("Failed to load local model: {}", e))?;
            Arc::new(provider)
        }
        _ => {
            // Try named provider first, fallback to auto-detect
            match clarity_core::llm::LlmFactory::create_arc(provider).await {
                Ok(llm) => llm,
                Err(e) => {
                    tracing::warn!(
                        "Failed to create provider '{}': {}, falling back to auto",
                        provider,
                        e
                    );
                    clarity_core::llm::LlmFactory::auto_arc()
                        .await
                        .map_err(|e| format!("LLM initialization failed: {}", e))?
                }
            }
        }
    };

    agent.set_llm(llm);
    Ok(())
}

/// Run a single-turn agent query (non-streaming).
///
/// If no LLM is configured, attempts to auto-configure from GuiSettings
/// (provider + local_model_path) or falls back to environment variables.
#[tauri::command]
pub async fn agent_run(
    query: String,
    state: State<'_, crate::AppState>,
) -> Result<String, String> {
    ensure_llm(&state.agent).await?;

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
    ensure_llm(&state.agent).await?;

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
