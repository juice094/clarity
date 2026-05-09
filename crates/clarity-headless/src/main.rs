//! Headless CLI for Clarity Agent
//!
//! Provides a pure terminal entry-point for running the Clarity agent
//! without TUI or GUI. Suitable for scripts, CI/CD, and automation.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
#[cfg(feature = "local-llm")]
use clarity_llm::{LocalGgufConfig, LocalGgufProvider};
use clarity_core::{
    agent::{
        jumpy::{
            predictor::{
                HistoricalPredictor, HybridPredictor, LlmAdapter, LlmAugmentedPredictor,
                OutcomePredictor, SkillObservation,
            },
            state::JumpyState,
        },
        AgentConfig, TokenUsage,
    },
    approval::ApprovalMode,
    Agent, ToolRegistry,
};
use clarity_llm::{AnthropicLlm, DeepSeekProvider, KimiLlm, OllamaProvider, OpenAiCompatibleLlm};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

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
                    eprintln!("Warning: LLM provider unavailable ({}). Falling back to historical predictor.", e);
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
