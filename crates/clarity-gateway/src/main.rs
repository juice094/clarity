use std::sync::Arc;
use tracing::{error, info, warn};

use clarity_gateway::{channels, server};

use channels::{
    discord::DiscordChannel, telegram::TelegramChannel, webhook::WebhookChannel, ChannelConfig,
    ChannelManager,
};
use clarity_core::agent::{Agent, AgentConfig, MockLlm};
use clarity_core::background::BackgroundTaskManager;
use clarity_core::background::agent_executor::DefaultAgentTaskExecutor;
use clarity_core::llm::LlmFactory;
use clarity_core::mcp::{
    config::McpConfig,
    register_mcp_tools, McpClientBuilder, McpRegistry, McpServerConfig,
};
use clarity_core::personality::{PersonalityConfig, YuanType};
use clarity_core::registry::ToolRegistry;
use std::path::PathBuf;

/// 从环境变量加载渠道配置
fn load_channel_configs() -> (ChannelConfig, ChannelConfig, ChannelConfig) {
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

    (telegram_config, discord_config, webhook_config)
}

/// 创建并配置 Agent
async fn create_agent() -> anyhow::Result<Arc<Agent>> {
    info!("Creating Agent with built-in tools...");

    // 创建工具注册表
    let registry = ToolRegistry::with_builtin_tools();

    // 配置人格（默认 Direct 工程模式，可通过 CLARITY_YUAN_TYPE 覆盖）
    let yuan_type = std::env::var("CLARITY_YUAN_TYPE")
        .ok()
        .and_then(|s| s.parse::<YuanType>().ok())
        .unwrap_or(YuanType::Direct);
    let personality_config = PersonalityConfig::new()
        .with_agent_name("Clarity")
        .with_user_name("User")
        .with_yuan_type(yuan_type)
        .with_locale("zh-CN");

    // 配置 Agent
    let config = AgentConfig::new()
        .with_max_iterations(10)
        .with_read_only(false)
        .with_personality(personality_config);

    // 创建 Agent
    let agent = Agent::with_config(registry, config);

    // 尝试自动检测 LLM provider (ANTHROPIC > KIMI_CODE > KIMI > DEEPSEEK > OPENAI)
    match LlmFactory::auto().await {
        Ok(llm) => {
            info!("LLM provider initialized successfully");
            Ok(Arc::new(agent.with_llm(Arc::from(llm))))
        }
        Err(e) => {
            warn!("Failed to create LLM provider: {}", e);
            warn!("Agent will use mock responses (MockLlm)");
            Ok(Arc::new(agent.with_llm(Arc::new(MockLlm))))
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
        let server_config = McpServerConfig::stdio(name, &entry.command)
            .with_args(entry.args.clone())
            .with_envs(entry.env.clone());
        let client = McpClientBuilder::from_config(server_config);
        mcp_registry.register(name, client);
    }

    if let Err(e) = mcp_registry.connect_all().await {
        warn!("Failed to connect to one or more MCP servers: {}", e);
        // Graceful degradation: continue with built-in tools only
        return;
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
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "clarity_gateway=debug,tower_http=debug".into()),
        )
        .init();

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
        std::fs::create_dir_all(&store_dir).ok();
        std::fs::create_dir_all(&work_dir).ok();

        let llm = agent.llm().unwrap_or_else(|| Arc::new(MockLlm));
        let registry = agent.registry().clone();
        let executor = Arc::new(DefaultAgentTaskExecutor::new(
            llm,
            registry,
            std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        ));

        Arc::new(
            BackgroundTaskManager::new(&store_dir, &work_dir, &work_dir)
                .with_agent_executor(executor),
        )
    };

    // 加载渠道配置
    let (telegram_config, discord_config, webhook_config) = load_channel_configs();

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
        let (telegram, discord, webhook) = load_channel_configs();

        assert!(!telegram.enabled);
        assert!(!discord.enabled);
        assert!(!webhook.enabled);
    }

    #[tokio::test]
    async fn test_agent_creation() {
        let agent = create_agent().await;
        assert!(agent.is_ok());
    }
}
