//! Headless CLI for Clarity Agent
//!
//! Provides a pure terminal entry-point for running the Clarity agent
//! without TUI or GUI. Suitable for scripts, CI/CD, and automation.

use anyhow::{Context, Result};
use clap::{Parser, ValueEnum};
use clarity_core::{
    Agent, ToolRegistry,
    agent::{AgentConfig, TokenUsage},
    approval::ApprovalMode,
    llm::{AnthropicLlm, DeepSeekProvider, KimiLlm, OllamaProvider, OpenAiCompatibleLlm},
};
#[cfg(feature = "local-llm")]
use clarity_core::llm::{LocalGgufConfig, LocalGgufProvider};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

/// Headless mode CLI arguments
#[derive(Parser, Debug)]
#[command(name = "clarity-headless", version, about = "Headless CLI for Clarity Agent")]
struct Args {
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

fn main() -> Result<()> {
    let runtime = tokio::runtime::Runtime::new().context("Failed to create Tokio runtime")?;
    runtime.block_on(async_main())
}

async fn async_main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let args = Args::parse();

    let prompt = read_prompt(&args).context("Failed to read prompt")?;

    let provider = build_provider(&args).await.context("Failed to build LLM provider")?;

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

fn read_prompt(args: &Args) -> Result<String> {
    match (&args.prompt, &args.file) {
        (Some(p), _) => Ok(p.clone()),
        (None, Some(path)) => std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read prompt file: {}", path)),
        (None, None) => anyhow::bail!("Either --prompt or --file must be provided"),
    }
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

async fn build_provider(args: &Args) -> Result<Arc<dyn clarity_core::agent::LlmProvider>> {
    let provider: Arc<dyn clarity_core::agent::LlmProvider> = match args.provider {
        ProviderType::Openai => {
            let api_key = args
                .api_key
                .clone()
                .or_else(|| std::env::var("OPENAI_API_KEY").ok())
                .context("OPENAI_API_KEY not set and --api-key not provided")?;
            let base_url = env_or("OPENAI_BASE_URL", "https://api.openai.com/v1");
            let model = args
                .model
                .clone()
                .unwrap_or_else(|| env_or("OPENAI_MODEL", "gpt-4o"));
            Arc::new(OpenAiCompatibleLlm::new(api_key, base_url, model))
        }
        ProviderType::Deepseek => {
            let api_key = args
                .api_key
                .clone()
                .or_else(|| std::env::var("DEEPSEEK_API_KEY").ok())
                .context("DEEPSEEK_API_KEY not set and --api-key not provided")?;
            let base_url = env_or("DEEPSEEK_BASE_URL", "https://api.deepseek.com/v1");
            let model = args
                .model
                .clone()
                .unwrap_or_else(|| env_or("DEEPSEEK_MODEL", "deepseek-chat"));
            Arc::new(DeepSeekProvider::new(api_key, base_url, model))
        }
        ProviderType::Anthropic => {
            let api_key = args
                .api_key
                .clone()
                .or_else(|| std::env::var("ANTHROPIC_AUTH_TOKEN").ok())
                .context("ANTHROPIC_AUTH_TOKEN not set and --api-key not provided")?;
            let base_url = env_or("ANTHROPIC_BASE_URL", "https://api.anthropic.com");
            let model = args
                .model
                .clone()
                .unwrap_or_else(|| env_or("ANTHROPIC_MODEL", "claude-3-sonnet-20240229"));
            Arc::new(AnthropicLlm::new(api_key, base_url, model))
        }
        ProviderType::Kimi => {
            let api_key = args
                .api_key
                .clone()
                .or_else(|| std::env::var("KIMI_API_KEY").ok())
                .context("KIMI_API_KEY not set and --api-key not provided")?;
            let base_url = env_or("KIMI_BASE_URL", "https://api.moonshot.ai/v1");
            let model = args
                .model
                .clone()
                .unwrap_or_else(|| env_or("KIMI_MODEL", "kimi-k2-07132k"));
            Arc::new(KimiLlm::new(api_key, base_url, model))
        }
        ProviderType::Ollama => {
            let base_url = env_or("OLLAMA_HOST", "http://localhost:11434");
            let model = args
                .model
                .clone()
                .unwrap_or_else(|| env_or("OLLAMA_MODEL", "llama3"));
            Arc::new(OllamaProvider::new(base_url, model))
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
            let mut config = LocalGgufConfig::new(model_path);
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
    fn test_args_parse_prompt() {
        let args = Args::try_parse_from(["clarity-headless", "--prompt", "hello"]).unwrap();
        assert_eq!(args.prompt, Some("hello".to_string()));
        assert_eq!(args.output, OutputFormat::Markdown);
        assert_eq!(args.provider, ProviderType::Openai);
    }

    #[test]
    fn test_args_parse_file_json() {
        let args =
            Args::try_parse_from(["clarity-headless", "--file", "test.md", "--output", "json"])
                .unwrap();
        assert_eq!(args.file, Some("test.md".to_string()));
        assert_eq!(args.output, OutputFormat::Json);
    }

    #[test]
    fn test_args_parse_approval_yolo() {
        let args =
            Args::try_parse_from(["clarity-headless", "--approval", "yolo", "--prompt", "hi"])
                .unwrap();
        assert_eq!(args.approval, ApprovalModeArg::Yolo);
    }

    #[test]
    fn test_args_parse_provider_kimi() {
        let args =
            Args::try_parse_from(["clarity-headless", "--provider", "kimi", "--prompt", "hi"])
                .unwrap();
        assert_eq!(args.provider, ProviderType::Kimi);
    }

    #[test]
    #[cfg(feature = "local-llm")]
    fn test_args_parse_provider_local() {
        let args =
            Args::try_parse_from(["clarity-headless", "--provider", "local", "--prompt", "hi"])
                .unwrap();
        assert_eq!(args.provider, ProviderType::Local);
    }

    #[test]
    fn test_args_missing_prompt_and_file() {
        let result = Args::try_parse_from(["clarity-headless"]);
        assert!(result.is_ok()); // clap doesn't enforce this; we enforce at runtime
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
