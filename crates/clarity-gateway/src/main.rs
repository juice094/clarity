use std::sync::Arc;
use tracing::{error, info, warn};

use clarity_gateway::{channels, server};

use channels::{ChannelManager, ChannelConfig, telegram::TelegramChannel, discord::DiscordChannel, webhook::WebhookChannel};
use clarity_core::agent::{Agent, AgentConfig};
use clarity_core::llm::LlmFactory;
use clarity_core::registry::ToolRegistry;

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
fn create_agent() -> anyhow::Result<Arc<Agent>> {
    info!("Creating Agent with built-in tools...");

    // 创建工具注册表
    let registry = ToolRegistry::with_builtin_tools();

    // 配置 Agent
    let config = AgentConfig::new()
        .with_max_iterations(10)
        .with_read_only(false);

    // 创建 Agent
    let agent = Agent::with_config(registry, config);

    // 尝试创建 LLM provider (优先尝试 Kimi)
    match LlmFactory::kimi() {
        Ok(llm) => {
            info!("LLM provider initialized successfully");
            Ok(Arc::new(agent.with_llm(Arc::from(llm))))
        }
        Err(e) => {
            warn!("Failed to create LLM provider: {}", e);
            warn!("Agent will use mock responses");
            Ok(Arc::new(agent))
        }
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
    let agent: Arc<Agent> = match create_agent() {
        Ok(agent) => agent,
        Err(e) => {
            error!("Failed to create agent: {}", e);
            std::process::exit(1);
        }
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
        info!("🔄 Starting {} channels: {:?}", channel_names.len(), channel_names);
        
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
    if let Err(e) = server::run(agent).await {
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

    #[test]
    fn test_agent_creation() {
        let agent = create_agent();
        assert!(agent.is_ok());
    }
}
