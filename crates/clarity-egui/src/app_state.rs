use crate::settings::GuiSettings;
use std::path::PathBuf;
use parking_lot::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Clone, Debug)]
pub struct LlmBinding {
    pub provider: String,
    pub local_model_path: String,
}

pub struct AppState {
    pub agent: clarity_core::Agent,
    pub llm_binding: Mutex<Option<LlmBinding>>,
    pub network_available: AtomicBool,
    pub llm_load_lock: tokio::sync::Mutex<()>,
    pub cached_settings: Mutex<GuiSettings>,
    pub prewarm_error: Mutex<Option<String>>,
    #[allow(dead_code)]
    pub initialized: AtomicBool,
    pub task_store: clarity_core::background::TaskStore,
}

impl Default for AppState {
    fn default() -> Self {
        let registry = clarity_core::ToolRegistry::with_builtin_tools();
        let agent = clarity_core::Agent::new(registry);
        let task_dir = dirs::data_dir()
            .map(|d| d.join("clarity").join("bg_tasks"))
            .unwrap_or_else(|| PathBuf::from("."));
        Self {
            agent,
            llm_binding: Mutex::new(None),
            network_available: AtomicBool::new(true),
            llm_load_lock: tokio::sync::Mutex::new(()),
            cached_settings: Mutex::new(GuiSettings::load()),
            prewarm_error: Mutex::new(None),
            initialized: AtomicBool::new(false),
            task_store: clarity_core::background::TaskStore::new(task_dir),
        }
    }
}

fn binding_matches(binding: &Option<LlmBinding>, provider: &str, path: &str) -> bool {
    matches!(binding, Some(b) if b.provider == provider && b.local_model_path == path)
}

pub async fn ensure_llm(state: &AppState) -> Result<(), String> {
    let settings = {
        let guard = state.cached_settings.lock();
        guard.clone()
    };

    let network_available = state.network_available.load(std::sync::atomic::Ordering::Relaxed);
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

    {
        let guard = state.llm_binding.lock();
        if binding_matches(&guard, &desired_provider, &desired_path) && state.agent.llm().is_some()
        {
            return Ok(());
        }
    }

    let _load_guard = state.llm_load_lock.lock().await;

    {
        let guard = state.llm_binding.lock();
        if binding_matches(&guard, &desired_provider, &desired_path) && state.agent.llm().is_some()
        {
            return Ok(());
        }
    }

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

    let mut guard = state.llm_binding.lock();
    *guard = Some(LlmBinding {
        provider: desired_provider,
        local_model_path: desired_path,
    });

    Ok(())
}

pub async fn reload_llm(state: &AppState) -> Result<(), String> {
    {
        let mut binding = state.llm_binding.lock();
        *binding = None;
    }
    ensure_llm(state).await.map_err(|e| e.to_string())
}

pub async fn check_network(probe: &str) -> bool {
    matches!(
        tokio::time::timeout(
            std::time::Duration::from_secs(3),
            tokio::net::TcpStream::connect(probe),
        )
        .await,
        Ok(Ok(_))
    )
}

pub async fn prewarm_llm(state: &AppState) -> Result<(), String> {
    ensure_llm(state).await
}
