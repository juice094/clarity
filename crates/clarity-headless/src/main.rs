#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, missing_docs)
)]
//! Headless CLI for Clarity Agent
//!
//! Provides a pure terminal entry-point for running the Clarity agent
//! without TUI or GUI. Suitable for scripts, CI/CD, and automation.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use clarity_core::{
    Agent, ToolRegistry,
    agent::{
        AgentConfig, TokenUsage,
        jumpy::{
            predictor::{
                HistoricalPredictor, HybridPredictor, LlmAdapter, LlmAugmentedPredictor,
                OutcomePredictor, SkillObservation,
            },
            state::JumpyState,
        },
    },
    approval::ApprovalMode,
    config::Config,
};
use clarity_llm::{AnthropicLlm, DeepSeekProvider, KimiLlm, OllamaProvider, OpenAiCompatibleLlm};
#[cfg(feature = "local-llm")]
use clarity_llm::{LocalGgufConfig, LocalGgufProvider};
use futures_util::StreamExt;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

mod threads;

/// Headless mode CLI arguments
#[derive(Parser, Debug)]
#[command(
    name = "clarity-headless",
    version,
    about = "Headless CLI for Clarity Agent"
)]
struct Args {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand, Debug, Clone)]
enum Command {
    /// Run the agent with a prompt
    Run(RunArgs),
    /// Jumpy World Model — predict outcome of a skill without executing it
    Jumpy(JumpyArgs),
    /// Validate configuration and print a health report
    Health(HealthArgs),
    /// Manage V2 threads and rollouts
    Threads(threads::ThreadsArgs),
    /// Run the KimiClaw ACP bridge to relay cloud messages to local Gateway
    AcpBridge(AcpBridgeArgs),
    /// Pair this Claw instance with a local Kimi Desktop OpenClaw Gateway
    #[command(name = "openclaw-pair")]
    OpenClawPair(OpenClawPairArgs),
}

#[derive(Parser, Debug, Clone)]
struct RunArgs {
    /// User prompt / instruction
    #[arg(short, long)]
    prompt: Option<String>,

    /// Read prompt from file
    #[arg(short, long)]
    file: Option<String>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "markdown")]
    output: OutputFormat,

    /// LLM provider to use
    #[arg(short = 'P', long, value_enum, default_value = "openai")]
    provider: ProviderType,

    /// Model name (overrides env var default)
    #[arg(short, long)]
    model: Option<String>,

    /// API key (or read from env var)
    #[arg(long)]
    api_key: Option<String>,

    /// Approval mode
    #[arg(short, long, value_enum, default_value = "interactive")]
    approval: ApprovalModeArg,

    /// Max iterations
    #[arg(long, default_value = "10")]
    max_iterations: usize,

    /// Enable plan mode
    #[arg(long)]
    plan: bool,

    /// Working directory
    #[arg(long)]
    working_dir: Option<String>,
}

#[derive(Parser, Debug, Clone)]
struct JumpyArgs {
    /// Skill ID to predict outcome for
    #[arg(short, long)]
    skill: String,

    /// Parameters as JSON string
    #[arg(short, long, default_value = "{}")]
    params: String,

    /// Read parameters from a JSON file (takes precedence over --params)
    #[arg(short = 'f', long)]
    params_file: Option<PathBuf>,

    /// Similarity threshold for HistoricalPredictor [0.0, 1.0]
    #[arg(short = 'T', long)]
    threshold: Option<f32>,

    /// Predictor type
    #[arg(short = 't', long, value_enum, default_value = "llm")]
    predictor: PredictorType,

    /// Historical observations file (JSON array of SkillObservation)
    #[arg(short = 'd', long)]
    observations: Option<PathBuf>,

    /// Commitment level ∈ [0.0, 1.0]
    #[arg(short, long, default_value = "0.9")]
    commitment: f32,

    /// Current state description (or read from stdin)
    #[arg(short = 'S', long)]
    state: Option<String>,

    /// LLM provider for LLM-augmented prediction
    #[arg(short = 'P', long, value_enum, default_value = "openai")]
    provider: ProviderType,

    /// Model name (overrides env var default)
    #[arg(short, long)]
    model: Option<String>,

    /// API key (or read from env var)
    #[arg(long)]
    api_key: Option<String>,

    /// Output format
    #[arg(short, long, value_enum, default_value = "json")]
    output: OutputFormat,
}

#[derive(Parser, Debug, Clone)]
struct HealthArgs {
    /// Output format
    #[arg(short, long, value_enum, default_value = "json")]
    output: OutputFormat,
}

#[derive(Parser, Debug, Clone)]
struct AcpBridgeArgs {
    /// Local backend to forward cloud messages to
    #[arg(short, long, value_enum, default_value = "auto")]
    local_backend: LocalBackendArg,

    /// Local Clarity Gateway URL
    #[arg(short, long, default_value = "http://127.0.0.1:18790")]
    gateway_url: String,

    /// Directory containing `openclaw.json` with the KimiClaw bridge config
    #[arg(short, long)]
    openclaw_home: Option<PathBuf>,
}

#[derive(Parser, Debug, Clone)]
struct OpenClawPairArgs {
    /// Local OpenClaw Gateway URL
    #[arg(short, long, default_value = "http://127.0.0.1:18679")]
    gateway_url: String,

    /// Admin token for the local OpenClaw Gateway
    #[arg(short, long)]
    token: String,
}

#[derive(Debug, Clone, ValueEnum, PartialEq)]
enum LocalBackendArg {
    /// Auto-detect: probe the native Clarity Gateway `/ws`; fall back to OpenClaw
    /// if it is unreachable.
    Auto,
    /// Original Clarity Gateway WebSocket.
    Gateway,
    /// Kimi Desktop local OpenClaw Gateway.
    Openclaw,
}

#[derive(Debug, Clone, ValueEnum, PartialEq)]
enum OutputFormat {
    Markdown,
    Json,
}

#[derive(Debug, Clone, ValueEnum, PartialEq)]
enum ProviderType {
    Openai,
    Anthropic,
    Deepseek,
    Ollama,
    Kimi,
    #[cfg(feature = "local-llm")]
    Local,
}

#[derive(Debug, Clone, ValueEnum, PartialEq)]
enum ApprovalModeArg {
    Interactive,
    Yolo,
    Plan,
}

#[derive(Debug, Clone, ValueEnum, PartialEq)]
enum PredictorType {
    Llm,
    Historical,
    Hybrid,
}

fn main() -> Result<()> {
    let runtime = tokio::runtime::Runtime::new().context("Failed to create Tokio runtime")?;
    runtime.block_on(async_main())
}

async fn async_main() -> Result<()> {
    clarity_core::logging::init();

    let args = Args::parse();

    match args.command {
        Command::Run(run_args) => run_command(run_args).await,
        Command::Jumpy(jumpy_args) => jumpy_command(jumpy_args).await,
        Command::Health(health_args) => health_command(health_args).await,
        Command::Threads(threads_args) => threads::run(threads_args).await,
        Command::AcpBridge(acp_args) => acp_bridge_command(acp_args).await,
        Command::OpenClawPair(pair_args) => openclaw_pair_command(pair_args).await,
    }
}

async fn run_command(args: RunArgs) -> Result<()> {
    let prompt = read_prompt(&args).context("Failed to read prompt")?;

    let provider = build_provider(
        args.provider,
        args.model.as_deref(),
        args.api_key.as_deref(),
    )
    .await
    .context("Failed to build LLM provider")?;

    let approval_mode = if args.plan {
        ApprovalMode::Plan
    } else {
        match args.approval {
            ApprovalModeArg::Interactive => ApprovalMode::Interactive,
            ApprovalModeArg::Yolo => ApprovalMode::Yolo,
            ApprovalModeArg::Plan => ApprovalMode::Plan,
        }
    };

    let working_dir = args
        .working_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let config = AgentConfig::new()
        .with_max_iterations(args.max_iterations)
        .with_working_dir(working_dir);

    let registry = ToolRegistry::with_builtin_tools();
    let agent = Agent::with_config(registry, config)
        .with_llm(provider)
        .with_approval_mode(approval_mode);

    let start = Instant::now();
    let result = agent.run(&prompt).await;
    let duration = start.elapsed();

    let usage = agent.get_session_usage();

    match args.output {
        OutputFormat::Markdown => match result {
            Ok(response) => println!("{}", response),
            Err(e) => {
                eprintln!("Error: {}", e);
                std::process::exit(1);
            }
        },
        OutputFormat::Json => {
            let json = build_json_response(result, usage, duration.as_millis() as u64);
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

async fn jumpy_command(args: JumpyArgs) -> Result<()> {
    // 0. Resolve params (file takes precedence)
    let params = if let Some(path) = args.params_file {
        std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read params file: {}", path.display()))?
    } else {
        args.params
    };

    // 1. Read current state
    let current = if let Some(state_desc) = args.state {
        JumpyState::from_query(&state_desc)
    } else {
        use std::io::IsTerminal;
        let stdin = std::io::stdin();
        if stdin.is_terminal() {
            anyhow::bail!("Either --state must be provided, or pipe state description via stdin");
        }
        let buf =
            std::io::read_to_string(stdin.lock()).context("Failed to read state from stdin")?;
        if buf.trim().is_empty() {
            anyhow::bail!("Stdin state input is empty");
        }
        JumpyState::from_query(&buf)
    };

    // 2. Build predictor
    let predictor: Box<dyn OutcomePredictor> = match args.predictor {
        PredictorType::Llm => {
            let provider = build_provider(
                args.provider,
                args.model.as_deref(),
                args.api_key.as_deref(),
            )
            .await
            .context("Failed to build LLM provider")?;
            let adapted = LlmAdapter::new(provider);
            Box::new(LlmAugmentedPredictor::new(Arc::new(adapted)))
        }
        PredictorType::Historical => {
            let mut historical = HistoricalPredictor::new();
            if let Some(t) = args.threshold {
                historical = historical.with_threshold(t);
            }
            if let Some(path) = args.observations {
                let data = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read observations: {}", path.display()))?;
                let observations: Vec<SkillObservation> =
                    serde_json::from_str(&data).context("Failed to parse observations as JSON")?;
                historical.observe_batch(observations);
            }
            Box::new(historical)
        }
        PredictorType::Hybrid => {
            let mut historical = HistoricalPredictor::new();
            if let Some(t) = args.threshold {
                historical = historical.with_threshold(t);
            }
            if let Some(path) = args.observations {
                let data = std::fs::read_to_string(&path)
                    .with_context(|| format!("Failed to read observations: {}", path.display()))?;
                let observations: Vec<SkillObservation> =
                    serde_json::from_str(&data).context("Failed to parse observations as JSON")?;
                historical.observe_batch(observations);
            }
            match build_provider(
                args.provider,
                args.model.as_deref(),
                args.api_key.as_deref(),
            )
            .await
            {
                Ok(provider) => {
                    let adapted = LlmAdapter::new(provider);
                    let llm = LlmAugmentedPredictor::new(Arc::new(adapted));
                    Box::new(HybridPredictor::new(historical, llm))
                }
                Err(e) => {
                    eprintln!(
                        "Warning: LLM provider unavailable ({}). Falling back to historical predictor.",
                        e
                    );
                    Box::new(historical)
                }
            }
        }
    };

    // 3. Predict
    let predicted = predictor
        .predict(
            &args.skill,
            &params,
            &current,
            args.commitment.clamp(0.0, 1.0),
        )
        .await
        .map_err(|e| anyhow::anyhow!("Prediction failed: {}", e))?;

    // 4. Output
    match args.output {
        OutputFormat::Markdown => {
            println!("# Jumpy Prediction\n");
            println!("**Skill**: `{}`", args.skill);
            println!("**Parameters**: `{}`", params);
            println!("**Commitment**: {:.2}", args.commitment);
            println!("\n## Predicted State\n");
            println!("- **Tags**: {:?}", predicted.tags);
            println!("- **Progress**: {:.2}", predicted.progress);
            println!("- **Context Summary**: {}", predicted.context_summary);
            println!("- **Active Files**: {:?}", predicted.active_files);
            println!("- **Memory**: {:?}", predicted.memory);
        }
        OutputFormat::Json => {
            let json = json!({
                "success": true,
                "skill": args.skill,
                "params": params,
                "commitment": args.commitment,
                "predicted_state": predicted,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }
    }

    Ok(())
}

async fn health_command(args: HealthArgs) -> Result<()> {
    let (_, health) = Config::load_with_health().context("Failed to load configuration")?;

    match args.output {
        OutputFormat::Json => {
            let json = health
                .to_json()
                .context("Failed to serialize health report")?;
            println!("{}", json);
        }
        OutputFormat::Markdown => {
            println!("# Config Health Report");
            println!();
            println!("- **Healthy**: {}", health.is_healthy());
            println!("- **Issues**: {}", health.issues().len());
            println!("- **Layers**: {}", health.layers().len());
            if !health.issues().is_empty() {
                println!();
                println!("## Issues");
                for issue in health.issues() {
                    println!("- {:?}", issue);
                }
            }
        }
    }

    if health.is_healthy() {
        Ok(())
    } else {
        anyhow::bail!("Configuration health check failed");
    }
}

/// Probe whether the native Clarity Gateway WebSocket endpoint is reachable.
///
/// Opens a short-lived connection, waits for the `welcome` frame, and closes.
/// Returns `true` only if the endpoint responds with a valid welcome message.
async fn native_gateway_reachable(gateway_url: &str) -> bool {
    let ws_url = clarity_claw::gateway_ws_url(gateway_url);
    match tokio::time::timeout(
        std::time::Duration::from_secs(3),
        tokio_tungstenite::connect_async(&ws_url),
    )
    .await
    {
        Ok(Ok((mut stream, _))) => {
            let welcome =
                tokio::time::timeout(std::time::Duration::from_secs(2), stream.next()).await;
            let _ = stream.close(None).await;
            matches!(
                welcome,
                Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text))))
                    if text.contains("\"type\":\"welcome\"")
            )
        }
        _ => false,
    }
}

async fn acp_bridge_command(args: AcpBridgeArgs) -> Result<()> {
    let openclaw_home = args.openclaw_home.clone().unwrap_or_else(|| {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".kimi_openclaw")
    });

    let (config, configured_gateway_url) =
        clarity_claw::acp_bridge::load_acp_config_and_gateway_url(&openclaw_home).with_context(
            || format!("Failed to load ACP bridge config from {:?}", openclaw_home),
        )?;

    let state_path = openclaw_home.join("acp-bridge-state.json");
    let gateway_url = if args.gateway_url == "http://127.0.0.1:18790" {
        // When the user did not override the default, prefer the Gateway URL
        // stored in openclaw.json so we match the local KimiClaw setup.
        configured_gateway_url
    } else {
        args.gateway_url
    };

    let (gateway_url, local_backend) = match args.local_backend {
        LocalBackendArg::Gateway => (
            gateway_url,
            clarity_claw::acp_bridge::LocalBackend::ClarityGateway,
        ),
        LocalBackendArg::Openclaw => {
            let oc_url = clarity_claw::openclaw_ws_url(&gateway_url);
            (
                oc_url,
                clarity_claw::acp_bridge::LocalBackend::OpenClawGateway,
            )
        }
        LocalBackendArg::Auto => {
            // Auto-detect: prefer the native Clarity Gateway WebSocket endpoint
            // because it is the canonical protocol and supports the full feature
            // set (role-context sync, wire messages, metrics). Fall back to the
            // OpenClaw-compatible endpoint only when the native endpoint is
            // unreachable, so Clarity can continue serving Claw traffic even
            // after Kimi Desktop is removed.
            if native_gateway_reachable(&gateway_url).await {
                (
                    gateway_url,
                    clarity_claw::acp_bridge::LocalBackend::ClarityGateway,
                )
            } else {
                tracing::info!(
                    gateway_url = %gateway_url,
                    "Native Gateway /ws unreachable, falling back to /openclaw/ws"
                );
                let oc_url = clarity_claw::openclaw_ws_url(&gateway_url);
                (
                    oc_url,
                    clarity_claw::acp_bridge::LocalBackend::OpenClawGateway,
                )
            }
        }
    };

    tracing::info!(
        gateway_url = %gateway_url,
        acp_url = %config.url,
        backend = ?local_backend,
        state_path = %state_path.display(),
        "Starting ACP bridge"
    );

    let (shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    let (status_tx, mut status_rx) =
        tokio::sync::watch::channel(clarity_claw::acp_bridge::AcpBridgeStatus::default());

    // Listen for Ctrl+C / SIGINT and request a clean bridge shutdown.
    let shutdown_tx_clone = shutdown_tx.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            let _ = shutdown_tx_clone.send(());
        }
    });

    // Periodically log bridge health so users can verify the relay is alive.
    tokio::spawn(async move {
        loop {
            let status = status_rx.borrow_and_update().clone();
            if status.connected || status.last_error.is_some() {
                tracing::info!(
                    connected = status.connected,
                    chat_id = ?status.chat_id,
                    reconnect_count = status.reconnect_count,
                    last_error = ?status.last_error,
                    "ACP bridge status"
                );
            }
            if status_rx.changed().await.is_err() {
                break;
            }
        }
    });

    let result = clarity_claw::acp_bridge::run_acp_gateway_bridge_with_options(
        &config,
        &gateway_url,
        local_backend,
        shutdown_rx,
        clarity_contract::retry::RetryConfig::default(),
        Some(state_path.as_path()),
        Some(status_tx),
    )
    .await
    .context("ACP bridge failed");

    drop(shutdown_tx);
    result
}

async fn openclaw_pair_command(args: OpenClawPairArgs) -> Result<()> {
    use clarity_claw::{
        device::{DeviceIdentity, PairedToken, save_paired_token},
        openclaw_gateway::{OpenClawDeviceApi, client::OpenClawGatewayClient},
    };

    let device = DeviceIdentity::load_or_generate()
        .map_err(|e| anyhow::anyhow!("Failed to load or generate Claw device identity: {e}"))?;
    let ws_url = clarity_claw::openclaw_ws_url(&args.gateway_url);

    tracing::info!(
        gateway_url = %args.gateway_url,
        device_id = %device.device_id(),
        "Requesting OpenClaw device pairing"
    );

    let client = OpenClawGatewayClient::connect(&ws_url, &args.token)
        .await
        .context("Failed to connect to OpenClaw Gateway")?;

    // Wait for handshake before issuing the pairing request.
    for _ in 0..60 {
        if client.hello_ok().is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
    if client.hello_ok().is_none() {
        anyhow::bail!("OpenClaw Gateway handshake timed out");
    }

    let result = client
        .pair_request(&device)
        .await
        .context("device.pair.request failed")?;

    tracing::info!(
        device_id = %result.device_id,
        approved = result.approved,
        scopes = ?result.scopes,
        "OpenClaw pairing result received"
    );

    if !result.approved {
        anyhow::bail!(
            "Pairing request for {} is pending approval in the Gateway UI",
            result.device_id
        );
    }

    let record = PairedToken {
        gateway_url: args.gateway_url,
        token: args.token,
        device_token: result.token,
        role: "operator".to_string(),
        scopes: result.scopes,
        paired_at_ms: chrono::Utc::now().timestamp_millis(),
    };
    save_paired_token(&record).map_err(|e| anyhow::anyhow!("Failed to save paired token: {e}"))?;

    println!("OpenClaw pairing approved and saved.");
    println!("Device id: {}", result.device_id);
    println!("Scopes: {:?}", record.scopes);
    Ok(())
}

fn read_prompt(args: &RunArgs) -> Result<String> {
    match (&args.prompt, &args.file) {
        (Some(p), _) => Ok(p.clone()),
        (None, Some(path)) => std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read prompt file: {}", path)),
        (None, None) => {
            use std::io::IsTerminal;
            let stdin = std::io::stdin();
            if stdin.is_terminal() {
                anyhow::bail!("Either --prompt, --file must be provided, or pipe input via stdin");
            }
            let buf = std::io::read_to_string(stdin.lock())
                .context("Failed to read prompt from stdin")?;
            if buf.trim().is_empty() {
                anyhow::bail!("Stdin input is empty");
            }
            Ok(buf)
        }
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

async fn build_provider(
    provider_type: ProviderType,
    model: Option<&str>,
    api_key: Option<&str>,
) -> Result<Arc<dyn clarity_core::agent::LlmProvider>> {
    let provider: Arc<dyn clarity_core::agent::LlmProvider> = match provider_type {
        ProviderType::Openai => {
            let key = api_key
                .map(|s| s.to_string())
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .context("OPENAI_API_KEY not set and --api-key not provided")?;
            let base_url = env_or("OPENAI_BASE_URL", "https://api.openai.com/v1");
            let m = model
                .map(|s| s.to_string())
                .unwrap_or_else(|| env_or("OPENAI_MODEL", "gpt-4o"));
            Arc::new(OpenAiCompatibleLlm::new(key, base_url, m))
        }
        ProviderType::Deepseek => {
            let key = api_key
                .map(|s| s.to_string())
                .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
                .context("DEEPSEEK_API_KEY not set and --api-key not provided")?;
            let base_url = env_or("DEEPSEEK_BASE_URL", "https://api.deepseek.com/v1");
            let m = model
                .map(|s| s.to_string())
                .unwrap_or_else(|| env_or("DEEPSEEK_MODEL", "deepseek-chat"));
            Arc::new(DeepSeekProvider::new(key, base_url, m))
        }
        ProviderType::Anthropic => {
            let key = api_key
                .map(|s| s.to_string())
                .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
                .context("ANTHROPIC_AUTH_TOKEN not set and --api-key not provided")?;
            let base_url = env_or("ANTHROPIC_BASE_URL", "https://api.anthropic.com");
            let m = model
                .map(|s| s.to_string())
                .unwrap_or_else(|| env_or("ANTHROPIC_MODEL", "claude-3-sonnet-20240229"));
            Arc::new(AnthropicLlm::new(key, base_url, m))
        }
        ProviderType::Kimi => {
            let key = api_key
                .map(|s| s.to_string())
                .or_else(|| std::env::var("KIMI_API_KEY").ok())
                .context("KIMI_API_KEY not set and --api-key not provided")?;
            let base_url = env_or("KIMI_BASE_URL", "https://api.moonshot.ai/v1");
            let m = model
                .map(|s| s.to_string())
                .unwrap_or_else(|| env_or("KIMI_MODEL", "kimi-k2-07132k"));
            Arc::new(KimiLlm::new(key, base_url, m))
        }
        ProviderType::Ollama => {
            let base_url = env_or("OLLAMA_HOST", "http://localhost:11434");
            let m = model
                .map(|s| s.to_string())
                .unwrap_or_else(|| env_or("OLLAMA_MODEL", "llama3"));
            Arc::new(OllamaProvider::new(base_url, m))
        }
        #[cfg(feature = "local-llm")]
        ProviderType::Local => {
            let model_path = std::env::var("CLARITY_LOCAL_MODEL_PATH")
                .map(PathBuf::from)
                .ok()
                .or_else(|| {
                    if let Some(home) = dirs::home_dir() {
                        let models_dir = home.join("models");
                        if let Ok(entries) = std::fs::read_dir(&models_dir) {
                            let mut ggufs: Vec<_> = entries
                                .filter_map(|e| e.ok())
                                .filter(|e| {
                                    e.path()
                                        .extension()
                                        .and_then(|ext| ext.to_str())
                                        .map(|ext| ext.eq_ignore_ascii_case("gguf"))
                                        .unwrap_or(false)
                                })
                                .map(|e| e.path())
                                .collect();
                            ggufs.sort();
                            return ggufs.into_iter().next();
                        }
                    }
                    None
                })
                .context("No local model found. Set CLARITY_LOCAL_MODEL_PATH or place a .gguf file in ~/models/")?;
            let tokenizer_repo = std::env::var("CLARITY_LOCAL_TOKENIZER_REPO").ok();
            let mut config = LocalGgufConfig::new(model_path)?;
            if let Some(repo) = tokenizer_repo {
                config = config.with_tokenizer_repo(repo);
            }
            let provider = LocalGgufProvider::new(config).await?;
            Arc::new(provider)
        }
    };
    Ok(provider)
}

fn build_json_response(
    result: Result<String, clarity_core::AgentError>,
    usage: TokenUsage,
    duration_ms: u64,
) -> serde_json::Value {
    match result {
        Ok(response) => json!({
            "success": true,
            "response": response,
            "usage": {
                "prompt_tokens": usage.prompt_tokens,
                "completion_tokens": usage.completion_tokens,
                "total_tokens": usage.total_tokens,
            },
            "duration_ms": duration_ms,
            "tool_calls": [],
        }),
        Err(e) => json!({
            "success": false,
            "response": null,
            "error": e.to_string(),
            "usage": {
                "prompt_tokens": usage.prompt_tokens,
                "completion_tokens": usage.completion_tokens,
                "total_tokens": usage.total_tokens,
            },
            "duration_ms": duration_ms,
            "tool_calls": [],
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_args_parse_run_prompt() {
        let args = Args::try_parse_from(["clarity-headless", "run", "--prompt", "hello"]).unwrap();
        match args.command {
            Command::Run(run_args) => {
                assert_eq!(run_args.prompt, Some("hello".to_string()));
                assert_eq!(run_args.output, OutputFormat::Markdown);
                assert_eq!(run_args.provider, ProviderType::Openai);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_args_parse_run_file_json() {
        let args = Args::try_parse_from([
            "clarity-headless",
            "run",
            "--file",
            "test.md",
            "--output",
            "json",
        ])
        .unwrap();
        match args.command {
            Command::Run(run_args) => {
                assert_eq!(run_args.file, Some("test.md".to_string()));
                assert_eq!(run_args.output, OutputFormat::Json);
            }
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_args_parse_run_approval_yolo() {
        let args = Args::try_parse_from([
            "clarity-headless",
            "run",
            "--approval",
            "yolo",
            "--prompt",
            "hi",
        ])
        .unwrap();
        match args.command {
            Command::Run(run_args) => assert_eq!(run_args.approval, ApprovalModeArg::Yolo),
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_args_parse_run_provider_kimi() {
        let args = Args::try_parse_from([
            "clarity-headless",
            "run",
            "--provider",
            "kimi",
            "--prompt",
            "hi",
        ])
        .unwrap();
        match args.command {
            Command::Run(run_args) => assert_eq!(run_args.provider, ProviderType::Kimi),
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    #[cfg(feature = "local-llm")]
    fn test_args_parse_run_provider_local() {
        let args = Args::try_parse_from([
            "clarity-headless",
            "run",
            "--provider",
            "local",
            "--prompt",
            "hi",
        ])
        .unwrap();
        match args.command {
            Command::Run(run_args) => assert_eq!(run_args.provider, ProviderType::Local),
            _ => panic!("Expected Run command"),
        }
    }

    #[test]
    fn test_args_parse_jumpy_skill() {
        let args = Args::try_parse_from([
            "clarity-headless",
            "jumpy",
            "--skill",
            "test-skill",
            "--params",
            "{\"key\":\"value\"}",
            "--state",
            "test state",
        ])
        .unwrap();
        match args.command {
            Command::Jumpy(jumpy_args) => {
                assert_eq!(jumpy_args.skill, "test-skill");
                assert_eq!(jumpy_args.params, "{\"key\":\"value\"}");
                assert_eq!(jumpy_args.state, Some("test state".to_string()));
                assert_eq!(jumpy_args.predictor, PredictorType::Llm);
                assert_eq!(jumpy_args.commitment, 0.9);
                assert_eq!(jumpy_args.output, OutputFormat::Json);
            }
            _ => panic!("Expected Jumpy command"),
        }
    }

    #[test]
    fn test_args_parse_jumpy_hybrid() {
        let args = Args::try_parse_from([
            "clarity-headless",
            "jumpy",
            "--skill",
            "test",
            "--predictor",
            "hybrid",
            "--observations",
            "obs.json",
            "--state",
            "input",
        ])
        .unwrap();
        match args.command {
            Command::Jumpy(jumpy_args) => {
                assert_eq!(jumpy_args.predictor, PredictorType::Hybrid);
                assert_eq!(jumpy_args.observations, Some(PathBuf::from("obs.json")));
            }
            _ => panic!("Expected Jumpy command"),
        }
    }

    #[test]
    fn test_args_parse_jumpy_params_file() {
        let args = Args::try_parse_from([
            "clarity-headless",
            "jumpy",
            "--skill",
            "test-skill",
            "--params-file",
            "params.json",
            "--threshold",
            "0.5",
        ])
        .unwrap();
        match args.command {
            Command::Jumpy(jumpy_args) => {
                assert_eq!(jumpy_args.skill, "test-skill");
                assert_eq!(jumpy_args.params_file, Some(PathBuf::from("params.json")));
                assert_eq!(jumpy_args.threshold, Some(0.5));
            }
            _ => panic!("Expected Jumpy command"),
        }
    }

    #[test]
    fn test_args_parse_health_default_json() {
        let args = Args::try_parse_from(["clarity-headless", "health"]).unwrap();
        match args.command {
            Command::Health(health_args) => {
                assert_eq!(health_args.output, OutputFormat::Json);
            }
            _ => panic!("Expected Health command"),
        }
    }

    #[test]
    fn test_args_parse_health_markdown() {
        let args =
            Args::try_parse_from(["clarity-headless", "health", "--output", "markdown"]).unwrap();
        match args.command {
            Command::Health(health_args) => {
                assert_eq!(health_args.output, OutputFormat::Markdown);
            }
            _ => panic!("Expected Health command"),
        }
    }

    #[test]
    fn test_args_parse_acp_bridge_defaults() {
        let args = Args::try_parse_from(["clarity-headless", "acp-bridge"]).unwrap();
        match args.command {
            Command::AcpBridge(acp_args) => {
                assert_eq!(acp_args.gateway_url, "http://127.0.0.1:18790");
                assert_eq!(acp_args.openclaw_home, None);
                assert_eq!(acp_args.local_backend, LocalBackendArg::Auto);
            }
            _ => panic!("Expected AcpBridge command"),
        }
    }

    #[test]
    fn test_args_parse_acp_bridge_custom() {
        let args = Args::try_parse_from([
            "clarity-headless",
            "acp-bridge",
            "--gateway-url",
            "http://custom:8080",
            "--openclaw-home",
            "/tmp/oc",
        ])
        .unwrap();
        match args.command {
            Command::AcpBridge(acp_args) => {
                assert_eq!(acp_args.gateway_url, "http://custom:8080");
                assert_eq!(acp_args.openclaw_home, Some(PathBuf::from("/tmp/oc")));
                assert_eq!(acp_args.local_backend, LocalBackendArg::Auto);
            }
            _ => panic!("Expected AcpBridge command"),
        }
    }

    #[test]
    fn test_args_parse_acp_bridge_local_backend_gateway() {
        let args = Args::try_parse_from([
            "clarity-headless",
            "acp-bridge",
            "--local-backend",
            "gateway",
        ])
        .unwrap();
        match args.command {
            Command::AcpBridge(acp_args) => {
                assert_eq!(acp_args.local_backend, LocalBackendArg::Gateway);
            }
            _ => panic!("Expected AcpBridge command"),
        }
    }

    #[test]
    fn test_args_parse_acp_bridge_local_backend_openclaw() {
        let args = Args::try_parse_from([
            "clarity-headless",
            "acp-bridge",
            "--local-backend",
            "openclaw",
        ])
        .unwrap();
        match args.command {
            Command::AcpBridge(acp_args) => {
                assert_eq!(acp_args.local_backend, LocalBackendArg::Openclaw);
            }
            _ => panic!("Expected AcpBridge command"),
        }
    }

    #[test]
    fn test_args_parse_openclaw_pair_defaults() {
        let args = Args::try_parse_from([
            "clarity-headless",
            "openclaw-pair",
            "--token",
            "admin-token",
        ])
        .unwrap();
        match args.command {
            Command::OpenClawPair(pair_args) => {
                assert_eq!(pair_args.gateway_url, "http://127.0.0.1:18679");
                assert_eq!(pair_args.token, "admin-token");
            }
            _ => panic!("Expected OpenClawPair command"),
        }
    }

    #[test]
    fn test_args_parse_openclaw_pair_custom() {
        let args = Args::try_parse_from([
            "clarity-headless",
            "openclaw-pair",
            "--gateway-url",
            "http://custom:18679",
            "--token",
            "custom-token",
        ])
        .unwrap();
        match args.command {
            Command::OpenClawPair(pair_args) => {
                assert_eq!(pair_args.gateway_url, "http://custom:18679");
                assert_eq!(pair_args.token, "custom-token");
            }
            _ => panic!("Expected OpenClawPair command"),
        }
    }

    #[test]
    fn test_json_response_success() {
        let usage = TokenUsage {
            prompt_tokens: 10,
            completion_tokens: 20,
            total_tokens: 30,
        };
        let json = build_json_response(Ok("Hello".to_string()), usage, 100);
        assert_eq!(json["success"], true);
        assert_eq!(json["response"], "Hello");
        assert_eq!(json["usage"]["total_tokens"], 30);
        assert_eq!(json["duration_ms"], 100);
    }

    #[test]
    fn test_json_response_error() {
        let usage = TokenUsage {
            prompt_tokens: 5,
            completion_tokens: 0,
            total_tokens: 5,
        };
        let err = clarity_core::AgentError::Llm("test error".into());
        let expected = err.to_string();
        let json = build_json_response(Err(err), usage, 50);
        assert_eq!(json["success"], false);
        assert_eq!(json["error"], expected);
    }
}
