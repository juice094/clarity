use clarity_core::AgentError;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, State};

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

/// Check whether the currently bound LLM matches the desired configuration.
fn binding_matches(binding: &Option<crate::LlmBinding>, provider: &str, path: &str) -> bool {
    matches!(binding, Some(b) if b.provider == provider && b.local_model_path == path)
}

/// Ensure the agent has an LLM configured, initializing from GuiSettings if needed.
///
/// This function also handles:
/// - **Configuration drift**: If the user changes provider or local_model_path in Settings,
///   the next request will automatically rebind to the new LLM.
/// - **Offline fallback**: When `network_available` is `false` and the preferred provider
///   is not local, it transparently switches to the local GGUF model.
/// - **Concurrent-load safety**: Multiple simultaneous requests (or the network monitor)
///   cannot trigger redundant model loads thanks to `llm_load_lock`.
pub async fn ensure_llm(state: &crate::AppState) -> Result<(), String> {
    use std::sync::atomic::Ordering;

    let settings = {
        let guard = state.cached_settings.lock().unwrap();
        guard.clone()
    };
    let network_available = state.network_available.load(Ordering::Relaxed);

    // Determine which provider we should be using right now
    let desired_provider = if !network_available && settings.provider != "local" {
        tracing::info!(
            "Network unavailable (preferred={}); falling back to local",
            settings.provider
        );
        "local".to_string()
    } else {
        settings.provider.clone()
    };

    let desired_path = if desired_provider == "local" {
        settings
            .local_model_path
            .clone()
            .or_else(|| {
                clarity_core::llm::resolve_local_model_path()
                    .map(|p| p.to_string_lossy().into_owned())
            })
            .unwrap_or_default()
    } else {
        String::new()
    };

    // Fast path: check without blocking other async tasks
    {
        let guard = state.llm_binding.lock().unwrap();
        if binding_matches(&guard, &desired_provider, &desired_path) && state.agent.llm().is_some()
        {
            return Ok(());
        }
    }

    // Slow path: serialize actual loading so only one task loads the model.
    let _load_guard = state.llm_load_lock.lock().await;

    // Re-check after acquiring the lock — another task may have loaded
    // the desired model while we were waiting.
    {
        let guard = state.llm_binding.lock().unwrap();
        if binding_matches(&guard, &desired_provider, &desired_path) && state.agent.llm().is_some()
        {
            return Ok(());
        }
    }

    // Load the desired provider
    let llm: Arc<dyn clarity_core::llm::LlmProvider> = match desired_provider.as_str() {
        "local" => {
            if desired_path.is_empty() {
                return Err(
                    "No local model configured. Place .gguf in ~/models/ or set CLARITY_LOCAL_MODEL_PATH.".to_string(),
                );
            }
            let model_path = std::path::PathBuf::from(&desired_path);
            let sibling_tokenizer = model_path.with_file_name("tokenizer.json");

            let mut config = clarity_core::llm::LocalGgufConfig::new(&desired_path)
                .with_tokenizer_repo("Qwen/Qwen2.5-7B-Instruct");

            // If a tokenizer.json sits next to the model, use it locally so
            // offline fallback works even when HuggingFace is unreachable.
            if sibling_tokenizer.exists() {
                if let Ok(meta) = std::fs::metadata(&sibling_tokenizer) {
                    if meta.len() < 1024 {
                        return Err(format!(
                            "Tokenizer file {} seems corrupted (size {} bytes). \
                             Please re-download a valid tokenizer.json.",
                            sibling_tokenizer.display(),
                            meta.len()
                        ));
                    }
                }
                tracing::info!("Using local tokenizer at {}", sibling_tokenizer.display());
                config = config.with_tokenizer_path(&sibling_tokenizer);
            }

            let provider = clarity_core::llm::LocalGgufProvider::new(config)
                .await
                .map_err(|e| format!("Failed to load local model: {}", e))?;
            Arc::new(provider)
        }
        _ => {
            let api_key = settings.api_key.as_deref().unwrap_or("");
            match clarity_core::llm::LlmFactory::create_with_key_arc(
                &desired_provider,
                api_key,
                &settings.model,
            ) {
                Ok(llm) => llm,
                Err(e) => {
                    // Fallback: try env-var based creation if GUI key is empty
                    if api_key.is_empty() {
                        match clarity_core::llm::LlmFactory::create_arc(&desired_provider).await {
                            Ok(llm) => llm,
                            Err(_) => {
                                return Err(format!(
                                    "Provider '{}' requires an API key. \
                                     Please open Settings and enter your key.",
                                    desired_provider
                                ));
                            }
                        }
                    } else {
                        return Err(format!(
                            "Failed to create provider '{}': {}. \
                             Please check your API key and network connection.",
                            desired_provider, e
                        ));
                    }
                }
            }
        }
    };

    state.agent.set_llm(llm);

    let mut guard = state.llm_binding.lock().unwrap();
    *guard = Some(crate::LlmBinding {
        provider: desired_provider,
        local_model_path: desired_path,
    });

    Ok(())
}

/// Run a single-turn agent query (non-streaming).
///
/// If no LLM is configured, attempts to auto-configure from GuiSettings
/// (provider + local_model_path) or falls back to environment variables.
#[tauri::command]
pub async fn agent_run(query: String, state: State<'_, crate::AppState>) -> Result<String, String> {
    ensure_llm(&state).await?;

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
    ensure_llm(&state).await?;

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

/// Return the last prewarm error, if any, so the frontend can display it
/// after mount (startup events may have been missed).
#[tauri::command]
pub fn get_prewarm_status(state: State<crate::AppState>) -> Option<String> {
    state.prewarm_error.lock().unwrap().clone()
}
