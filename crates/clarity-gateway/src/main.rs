use std::sync::Arc;
use tracing::{error, info, warn};

use clarity_gateway::{channels, server};

use channels::{
    discord::DiscordChannel, slack::SlackChannel, telegram::TelegramChannel,
    webhook::WebhookChannel, ChannelConfig, ChannelManager,
};
use clarity_core::agent::{Agent, AgentConfig, MockLlm};
use clarity_core::background::agent_executor::DefaultAgentTaskExecutor;
use clarity_core::background::BackgroundTaskManager;
use clarity_llm::LlmFactory;
use clarity_core::mcp::{config::McpConfig, register_mcp_tools, McpClientBuilder, McpRegistry};
use clarity_core::memory::{
    LlmProviderBridge, MemoryTicker, PersistentMemoryStore, SharedMemoryTicker,
};
use clarity_core::registry::ToolRegistry;
use std::path::PathBuf;

/// 从环境变量加载渠道配置
fn load_channel_configs() -> (ChannelConfig, ChannelConfig, ChannelConfig, ChannelConfig) {
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

    (
        telegram_config,
        discord_config,
        webhook_config,
        slack_config,
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

    // 配置 Agent（window 入口：方法论驱动）
    let window_context = r#"# Methodology
You are a methodological query assistant. When answering:
1. Clarify the scope of the question first
2. Reason step by step, showing your thinking
3. Cite sources when possible; distinguish facts from speculation
4. Explicitly state when you are uncertain"#;

    let config = AgentConfig::new()
        .with_max_iterations(10)
        .with_read_only(false)
        .with_entry_context(window_context);

    // 创建持久化记忆存储
    let clarity_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".clarity");
    let memory_db = clarity_dir.join("memory.db");
    let _ = tokio::fs::create_dir_all(&clarity_dir).await;

    let memory_store = Arc::new(
        PersistentMemoryStore::new(&memory_db)
            .await
            .unwrap_or_else(|e| {
                warn!(
                    "Failed to create persistent memory store: {}, using in-memory",
                    e
                );
                PersistentMemoryStore::new_in_memory().expect("in-memory store should not fail")
            }),
    );

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

    // 注入 SubagentOrchestrator（SubagentManager）
    let subagent_ctx = clarity_dir.join("subagent_context");
    let _ = tokio::fs::create_dir_all(&subagent_ctx).await;
    let orchestrator = Arc::new(
        clarity_subagents::SubagentManager::new(
            agent.registry().clone(),
            &clarity_dir,
            &subagent_ctx,
        )
    );
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
    match clarity_llm::mcp_llm_provider::McpLlmProvider::connect_stdio(&command, &args).await {
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

/// Load a single LLM provider: persisted config → auto-detection fallback.
async fn load_llm_single() -> Arc<dyn clarity_llm::api::LlmProvider> {
    if let Some(user_cfg) = clarity_gateway::handlers::load_persisted_config().await {
        info!(
            "Found persisted user config for provider: {}",
            user_cfg.provider
        );
        match clarity_gateway::handlers::build_provider_from_config(&user_cfg).await {
            Ok(provider) => {
                info!("LLM provider loaded from persisted config");
                Arc::from(provider)
            }
            Err(e) => {
                warn!("Failed to build provider from persisted config: {}", e);
                load_llm_fallback().await
            }
        }
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
    let (telegram_config, discord_config, webhook_config, slack_config) = load_channel_configs();

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
        let (telegram, discord, webhook, slack) = load_channel_configs();

        assert!(!telegram.enabled);
        assert!(!discord.enabled);
        assert!(!webhook.enabled);
        assert!(!slack.enabled);
    }

    #[tokio::test]
    async fn test_agent_creation() {
        let agent = create_agent().await;
        assert!(agent.is_ok());
    }
}
