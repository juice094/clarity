#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        missing_docs,
        unsafe_code
    )
)]
//! Binary entry point for the Clarity Gateway server.
use std::sync::Arc;
use tracing::{error, info, warn};

use clarity_gateway::{channels, server};

use channels::{
    ChannelConfig, ChannelManager, discord::DiscordChannel, slack::SlackChannel,
    telegram::TelegramChannel, webhook::WebhookChannel, wechat::WeChatGatewayChannel,
};
use clarity_core::agent::{Agent, AgentConfig, MockLlm};
use clarity_core::background::BackgroundTaskManager;
use clarity_core::background::agent_executor::DefaultAgentTaskExecutor;
use clarity_core::mcp::{McpClientBuilder, McpRegistry, config::McpConfig, register_mcp_tools};
use clarity_core::memory::{
    LlmProviderBridge, MemoryTicker, PersistentMemoryStore, SharedMemoryTicker,
};
use clarity_core::registry::ToolRegistry;
use clarity_llm::LlmFactory;
use std::path::PathBuf;

/// 从环境变量加载渠道配置
fn load_channel_configs() -> (
    ChannelConfig,
    ChannelConfig,
    ChannelConfig,
    ChannelConfig,
    ChannelConfig,
) {
    // Telegram 配置
    let telegram_config = if std::env::var("TELEGRAM_ENABLED").unwrap_or_default() == "true" {
        ChannelConfig::new()
            .enabled()
            .with_token(std::env::var("TELEGRAM_BOT_TOKEN").unwrap_or_default())
    } else {
        ChannelConfig::new()
    };

    // Discord 配置
    let discord_config = if std::env::var("DISCORD_ENABLED").unwrap_or_default() == "true" {
        ChannelConfig::new()
            .enabled()
            .with_token(std::env::var("DISCORD_BOT_TOKEN").unwrap_or_default())
    } else {
        ChannelConfig::new()
    };

    // Webhook 配置
    let webhook_config = if std::env::var("WEBHOOK_ENABLED").unwrap_or_default() == "true" {
        ChannelConfig::new()
            .enabled()
            .with_webhook_url(std::env::var("WEBHOOK_URL").unwrap_or_default())
            .with_webhook_secret(std::env::var("WEBHOOK_SECRET").unwrap_or_default())
    } else {
        ChannelConfig::new()
    };

    // Slack 配置
    let slack_config = if std::env::var("SLACK_ENABLED").unwrap_or_default() == "true" {
        ChannelConfig::new()
            .enabled()
            .with_token(std::env::var("SLACK_BOT_TOKEN").unwrap_or_default())
            .with_extra(serde_json::json!({
                "app_token": std::env::var("SLACK_APP_TOKEN").unwrap_or_default(),
                "signing_secret": std::env::var("SLACK_SIGNING_SECRET").unwrap_or_default()
            }))
    } else {
        ChannelConfig::new()
    };

    // WeChat 配置
    let wechat_config = if std::env::var("WECHAT_ENABLED").unwrap_or_default() == "true" {
        let allowed_users: Vec<String> = std::env::var("WECHAT_ALLOWED_USERS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        let mut extra = serde_json::json!({
            "alias": std::env::var("WECHAT_ALIAS").unwrap_or_else(|_| "default".to_string()),
            "data_dir": std::env::var("WECHAT_DATA_DIR").unwrap_or_default(),
        });

        if !allowed_users.is_empty() {
            extra["allowed_users"] = serde_json::json!(allowed_users);
        }

        ChannelConfig::new().enabled().with_extra(extra)
    } else {
        ChannelConfig::new()
    };

    (
        telegram_config,
        discord_config,
        webhook_config,
        slack_config,
        wechat_config,
    )
}

/// 创建并配置 Agent
async fn create_agent() -> anyhow::Result<Arc<Agent>> {
    info!("Creating Agent with built-in tools...");

    // 加载 TOML 配置（默认 → ~/.config/clarity/ → .clarity.toml → 环境变量覆盖）
    // 然后将配置中的凭证导出到 provider 特定的环境变量，供 LlmFactory::auto() 读取
    if let Ok(config) = clarity_core::config::Config::load() {
        config.export_to_env();
    }

    // 创建工具注册表
    let registry = ToolRegistry::with_builtin_tools();

    // 配置 Agent（window 入口：方法论驱动，或被外部 agent workspace 覆盖）
    let window_context = r#"# Methodology
You are a methodological query assistant. When answering:
1. Clarify the scope of the question first
2. Reason step by step, showing your thinking
3. Cite sources when possible; distinguish facts from speculation
4. Explicitly state when you are uncertain"#;

    let mut config = AgentConfig::new()
        .with_max_iterations(10)
        .with_read_only(false)
        .with_entry_context(window_context);

    // 如果存在外部 agent workspace，加载其 agent.yaml 作为人格/记忆入口
    let agent_workspace = std::env::var("CLARITY_AGENT_WORKSPACE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".kimi_openclaw")
                .join("workspace")
        });

    if agent_workspace.join("agent.yaml").exists() {
        match clarity_core::agent::definition::load_agent_definition(&agent_workspace) {
            Ok(def) => {
                info!("Loaded agent definition from {}", agent_workspace.display());
                if let Err(e) = clarity_core::agent::definition::apply_to_config(&def, &mut config)
                {
                    warn!("Failed to apply agent definition: {}", e);
                }
                // Workspace 内的相对路径（SOUL.md / MEMORY.md 等）应以工作区为基准
                config = config.with_working_dir(&agent_workspace);
            }
            Err(e) => {
                warn!("Agent workspace agent.yaml found but failed to load: {}", e);
            }
        }
    } else {
        info!(
            "No agent workspace agent.yaml found at {}, using default methodology",
            agent_workspace.display()
        );
    }

    // 使用加密 registry 中的 active alias 作为 model_alias，覆盖 agent.yaml 中的默认值，
    // 确保 Agent 实际调用的模型与当前选中的 LLM provider 一致。
    if let Some(alias) = clarity_gateway::handlers::config::load_active_alias().await
        && !alias.is_empty()
    {
        config.model_alias = Some(alias.clone());
        info!(
            "Overriding model alias from active alias registry: {}",
            alias
        );
    }

    // 创建持久化记忆存储
    let clarity_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".clarity");
    let memory_db = clarity_dir.join("memory.db");
    let _ = tokio::fs::create_dir_all(&clarity_dir).await;

    let memory_store = Arc::new(match PersistentMemoryStore::new_auto(&memory_db).await {
        Ok(store) => store,
        Err(e) => {
            warn!(
                "Failed to create persistent memory store: {}, using in-memory",
                e
            );
            PersistentMemoryStore::new_in_memory().map_err(|inner| {
                anyhow::anyhow!("Failed to create in-memory memory store: {}", inner)
            })?
        }
    });

    let compiled_dir = clarity_dir.join("compiled");
    let _ = tokio::fs::create_dir_all(&compiled_dir).await;
    let memory_ticker = SharedMemoryTicker::new(MemoryTicker::new(&compiled_dir, Some(5)));

    // 1. Try MCP LLM server (explicit user config)
    let (llm, provider_label): (Arc<dyn clarity_llm::api::LlmProvider>, String) =
        if let Some(mcp_llm) = load_llm_mcp().await {
            let label = if let Ok(cmd) = std::env::var("CLARITY_MCP_LLM_COMMAND") {
                format!("mcp:{}", cmd)
            } else {
                "mcp:unknown".to_string()
            };
            (mcp_llm, label)
        } else if let Ok(mesh) = clarity_llm::mesh::MeshLlmProvider::from_env().await {
            if !mesh.provider_names().is_empty() {
                let names = mesh.provider_names();
                info!("LLM mesh loaded with providers: {:?}", names);
                let label = format!("mesh:{}", names.join(","));
                (Arc::new(mesh), label)
            } else {
                let single = load_llm_single().await;
                (single, "single".to_string())
            }
        } else {
            let single = load_llm_single().await;
            (single, "single".to_string())
        };

    // 创建 Agent
    let mut agent = Agent::with_config(registry, config)
        .with_memory(memory_store.clone())
        .with_memory_ticker(memory_ticker)
        .with_llm(llm.clone());
    agent.set_provider_label(provider_label);

    // 允许通过环境变量覆盖 Agent 最大上下文窗口
    if let Ok(val) = std::env::var("CLARITY_MAX_CONTEXT_TOKENS") {
        match val.parse::<usize>() {
            Ok(max_tokens) => {
                agent = agent.with_max_context_tokens(max_tokens);
                info!("Agent max context tokens set to {}", max_tokens);
            }
            Err(_) => {
                warn!(
                    "CLARITY_MAX_CONTEXT_TOKENS is not a valid usize: {}, ignoring",
                    val
                );
            }
        }
    }

    // Headless Gateway 默认使用 yolo 审批模式，避免 channel 回复因等待人工确认而卡死。
    // 可通过 CLARITY_APPROVAL_MODE=interactive|smart|plan|yolo 覆盖。
    let approval_mode = match std::env::var("CLARITY_APPROVAL_MODE")
        .unwrap_or_else(|_| "yolo".to_string())
        .to_lowercase()
        .as_str()
    {
        "interactive" => clarity_core::approval::ApprovalMode::Interactive,
        "smart" => clarity_core::approval::ApprovalMode::Smart,
        "plan" => clarity_core::approval::ApprovalMode::Plan,
        _ => clarity_core::approval::ApprovalMode::Yolo,
    };
    agent = agent.with_approval_mode(approval_mode);
    info!("Gateway approval mode set to {:?}", approval_mode);

    // 注入 SubagentOrchestrator（SubagentManager）
    let subagent_ctx = clarity_dir.join("subagent_context");
    let _ = tokio::fs::create_dir_all(&subagent_ctx).await;
    let orchestrator = Arc::new(clarity_subagents::SubagentManager::new(
        agent.registry().clone(),
        &clarity_dir,
        &subagent_ctx,
    ));
    agent = agent.with_orchestrator(orchestrator);

    // 设置 MemoryCompiler callback（OpenHanako 四级编译管道）
    let sessions_dir = clarity_dir.join("sessions");
    let _ = tokio::fs::create_dir_all(&sessions_dir).await;
    match clarity_memory::SessionStore::new(&sessions_dir) {
        Ok(session_store) => {
            let compiler = clarity_memory::MemoryCompiler::new(
                memory_store.inner().clone(),
                session_store,
                Arc::new(LlmProviderBridge::new(llm)),
                clarity_memory::CompileConfig::default(),
            );
            let compiler = Arc::new(tokio::sync::Mutex::new(compiler));
            let compiled_dir_clone = compiled_dir.clone();
            agent
                .set_memory_compile_callback(move || {
                    let compiler = compiler.clone();
                    let compiled_dir = compiled_dir_clone.clone();
                    async move {
                        let mut compiler = compiler.lock().await;
                        compiler.compile_all(&compiled_dir).await
                    }
                })
                .await;
            info!("Memory compiler callback registered");
        }
        Err(e) => {
            warn!(
                "Failed to create session store: {}, memory compiler disabled",
                e
            );
        }
    }

    Ok(Arc::new(agent))
}

/// Try connecting to an MCP LLM server via stdio.
/// Controlled by `CLARITY_MCP_LLM_COMMAND` and `CLARITY_MCP_LLM_ARGS`.
async fn load_llm_mcp() -> Option<Arc<dyn clarity_llm::api::LlmProvider>> {
    let command = std::env::var("CLARITY_MCP_LLM_COMMAND").ok()?;
    let args = std::env::var("CLARITY_MCP_LLM_ARGS")
        .unwrap_or_default()
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>();

    info!("Attempting MCP LLM connection: {} {:?}", command, args);
    match clarity_mcp::McpLlmProvider::connect_stdio(&command, &args).await {
        Ok(provider) => {
            info!("MCP LLM provider connected successfully");
            Some(Arc::new(provider))
        }
        Err(e) => {
            warn!("Failed to connect MCP LLM provider: {}", e);
            None
        }
    }
}

/// Load a single LLM provider: active alias from encrypted registry → auto-detection fallback.
async fn load_llm_single() -> Arc<dyn clarity_llm::api::LlmProvider> {
    if let Some(provider) = clarity_gateway::handlers::config::load_active_provider().await {
        info!("LLM provider loaded from encrypted alias registry (ReliableProvider wrapped)");
        provider
    } else {
        load_llm_fallback().await
    }
}

/// Fallback LLM provider auto-detection.
async fn load_llm_fallback() -> Arc<dyn clarity_llm::api::LlmProvider> {
    match LlmFactory::auto().await {
        Ok(llm) => {
            info!("LLM provider initialized successfully");
            Arc::from(llm)
        }
        Err(e) => {
            warn!("Failed to create LLM provider: {}", e);
            warn!("Agent will use mock responses (MockLlm)");
            Arc::new(MockLlm)
        }
    }
}

/// Load MCP configuration from default path or env override, and register available MCP tools.
async fn load_and_register_mcp_tools(agent: &Agent) {
    // Determine config path: env override > default config dir > local fallback
    let config_result = if let Ok(env_path) = std::env::var("CLARITY_MCP_CONFIG_PATH") {
        McpConfig::load(&env_path)
    } else {
        McpConfig::load_default().or_else(|_| {
            let local = PathBuf::from("mcp.json");
            if local.exists() {
                McpConfig::load(&local)
            } else {
                let local_hidden = PathBuf::from(".clarity").join("mcp.json");
                if local_hidden.exists() {
                    McpConfig::load(&local_hidden)
                } else {
                    Err(anyhow::anyhow!("No MCP config found"))
                }
            }
        })
    };

    let config = match config_result {
        Ok(cfg) => cfg,
        Err(e) => {
            info!("MCP config not found or failed to load: {}", e);
            return;
        }
    };

    if config.servers.is_empty() {
        info!("MCP config loaded but no servers configured");
        return;
    }

    let mut mcp_registry = McpRegistry::new();
    for (name, entry) in &config.servers {
        if entry.disabled {
            info!("MCP server '{}' is disabled, skipping", name);
            continue;
        }
        let client = McpClientBuilder::from_mcp_entry(name, entry);
        mcp_registry.register(name, client);
    }

    match tokio::time::timeout(
        std::time::Duration::from_secs(10),
        mcp_registry.connect_all(),
    )
    .await
    {
        Ok(Ok(())) => {}
        Ok(Err(e)) => {
            warn!("Failed to connect to one or more MCP servers: {}", e);
            return;
        }
        Err(_) => {
            warn!("MCP server connection timed out after 10s; continuing with built-in tools only");
            return;
        }
    }

    let registry = agent.registry();
    if let Err(e) = register_mcp_tools(&mcp_registry, registry).await {
        warn!("Failed to register MCP tools: {}", e);
    } else {
        let names = registry.list_tools().unwrap_or_default();
        let mcp_names: Vec<_> = names.into_iter().filter(|n| n.contains('_')).collect();
        info!("Registered MCP tools: {:?}", mcp_names);
    }
}

#[tokio::main]
async fn main() {
    // 初始化日志（自动脱敏）
    clarity_core::logging::init_with_default("clarity_gateway=debug,tower_http=debug");

    info!("🚀 Clarity Gateway starting...");

    // 创建 Agent
    let agent: Arc<Agent> = match create_agent().await {
        Ok(agent) => agent,
        Err(e) => {
            error!("Failed to create agent: {}", e);
            std::process::exit(1);
        }
    };

    // 加载并注册 MCP 工具
    load_and_register_mcp_tools(&agent).await;

    // 创建后台任务管理器
    let task_manager = {
        let clarity_dir = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join(".clarity");
        let store_dir = clarity_dir.join("tasks");
        let work_dir = clarity_dir.join("work");
        let _ = tokio::fs::create_dir_all(&store_dir).await;
        let _ = tokio::fs::create_dir_all(&work_dir).await;

        let llm = agent.llm().unwrap_or_else(|| Arc::new(MockLlm));
        let registry = agent.registry().clone();
        let mut executor = DefaultAgentTaskExecutor::new(
            llm,
            registry,
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        );
        // Attach ModelRegistry for per-task model selection
        if let Ok(model_registry) = clarity_llm::ModelRegistry::load_async().await {
            executor = executor.with_registry(model_registry);
        }
        let executor = Arc::new(executor);

        Arc::new(
            BackgroundTaskManager::new(&store_dir, &work_dir, &work_dir)
                .with_agent_executor(executor),
        )
    };

    // Bind cron tools to the background task manager
    agent.with_cron_manager(task_manager.clone());
    info!("🔗 Bound cron tools to BackgroundTaskManager");

    // 加载渠道配置
    let (telegram_config, discord_config, webhook_config, slack_config, wechat_config) =
        load_channel_configs();

    // 创建渠道管理器
    let mut channel_manager = ChannelManager::new();

    // 注册 Telegram 渠道
    if telegram_config.enabled {
        info!("📱 Telegram channel enabled");
        channel_manager.register(Box::new(TelegramChannel::new(telegram_config)));
    } else {
        info!("📱 Telegram channel disabled (set TELEGRAM_ENABLED=true to enable)");
    }

    // 注册 Discord 渠道
    if discord_config.enabled {
        info!("💬 Discord channel enabled");
        channel_manager.register(Box::new(DiscordChannel::new(discord_config)));
    } else {
        info!("💬 Discord channel disabled (set DISCORD_ENABLED=true to enable)");
    }

    // 注册 Webhook 渠道
    if webhook_config.enabled {
        info!("🔗 Webhook channel enabled");
        channel_manager.register(Box::new(WebhookChannel::new(webhook_config)));
    } else {
        info!("🔗 Webhook channel disabled (set WEBHOOK_ENABLED=true to enable)");
    }

    // 注册 Slack 渠道
    if slack_config.enabled {
        info!("💼 Slack channel enabled");
        channel_manager.register(Box::new(SlackChannel::new(slack_config)));
    } else {
        info!("💼 Slack channel disabled (set SLACK_ENABLED=true to enable)");
    }

    // 注册 WeChat 渠道
    if wechat_config.enabled {
        info!("📲 WeChat channel enabled");
        channel_manager.register(Box::new(WeChatGatewayChannel::new(wechat_config)));
    } else {
        info!("📲 WeChat channel disabled (set WECHAT_ENABLED=true to enable)");
    }

    // 启动所有渠道（在后台任务中运行）
    let channel_names = channel_manager.get_channel_names();
    if !channel_names.is_empty() {
        info!(
            "🔄 Starting {} channels: {:?}",
            channel_names.len(),
            channel_names
        );

        let agent_clone = agent.clone();
        tokio::spawn(async move {
            if let Err(e) = channel_manager.start_all(agent_clone).await {
                error!("Channel manager error: {}", e);
            }
        });
    } else {
        info!("ℹ️  No channels enabled");
    }

    // 启动主 HTTP 服务器
    info!("🌐 Starting main HTTP server...");
    if let Err(e) = server::run(agent, task_manager).await {
        error!("Server error: {}", e);
    }

    info!("👋 Clarity Gateway stopped");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_channel_config_loading() {
        // 测试默认配置（禁用状态）
        let (telegram, discord, webhook, slack, wechat) = load_channel_configs();

        assert!(!telegram.enabled);
        assert!(!discord.enabled);
        assert!(!webhook.enabled);
        assert!(!slack.enabled);
        assert!(!wechat.enabled);
    }

    #[tokio::test]
    async fn test_agent_creation() {
        let agent = create_agent().await;
        assert!(agent.is_ok());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn test_agent_workspace_loaded() {
        // Create a self-contained agent workspace so the test does not depend on
        // the user's home directory.
        let tmp = tempfile::tempdir().expect("failed to create temp dir");
        let workspace = tmp.path();
        let agent_yaml = r#"
version: 1
agent:
  name: "test-agent"
  system_prompt_path: "system.md"
"#;
        std::fs::write(workspace.join("agent.yaml"), agent_yaml)
            .expect("failed to write agent.yaml");
        std::fs::write(workspace.join("system.md"), "You are a test assistant.")
            .expect("failed to write system.md");

        // The environment variable is read by create_agent(). Serialize this
        // test with other tests that call create_agent() to avoid races.
        let _guard = AGENT_WORKSPACE_LOCK.lock().await;
        // SAFETY: test-only mutation of an environment variable that is not
        // read by any other concurrently-running test while the lock is held.
        unsafe {
            std::env::set_var("CLARITY_AGENT_WORKSPACE", workspace.as_os_str());
        }
        let agent = create_agent().await.expect("agent should be created");
        // SAFETY: paired with the set_var above; restores the prior state.
        unsafe {
            std::env::remove_var("CLARITY_AGENT_WORKSPACE");
        }
        drop(_guard);

        let config = agent.config();
        assert_eq!(config.name.as_deref(), Some("test-agent"));
        assert!(
            config.system_prompt.contains("test assistant"),
            "Test system prompt should be loaded from workspace"
        );
    }

    static AGENT_WORKSPACE_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
}
