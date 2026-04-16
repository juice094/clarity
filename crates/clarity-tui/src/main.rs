mod app;
mod async_job;
mod command_bar;
mod commands;
mod diff;
mod events;
mod popup;
mod popups;
mod ui;
mod widgets;
mod wire_adapter;

use anyhow::Result;
use app::App;
use clarity_core::agent::{Agent, AgentConfig};
use clarity_core::llm::LlmFactory;
use clarity_core::mcp::config::McpConfig;
use clarity_core::mcp::{register_mcp_tools, McpClientBuilder, McpRegistry};
use clarity_core::memory::{MemoryTicker, PersistentMemoryStore};
use clarity_core::personality::{PersonalityConfig, YuanType};
use clarity_core::registry::ToolRegistry;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化终端
    enable_raw_mode()?;
    let mut stdout = std::io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 创建 Agent
    let (agent, model_name, yuan_type) = create_agent().await?;

    // 创建应用
    let mut app = App::new(agent, model_name, yuan_type.to_string());

    // 运行应用
    let result = run_app(&mut terminal, &mut app).await;

    // 恢复终端
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

async fn create_agent() -> Result<(Arc<Agent>, String, YuanType)> {
    // 创建工具注册表
    let registry = ToolRegistry::with_builtin_tools();

    // 尝试加载 MCP 配置并注入外部工具
    load_and_register_mcp_tools(&registry).await;

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

    // 创建 Agent 配置
    let config = AgentConfig::default()
        .with_max_iterations(10)
        .with_read_only(false)
        .with_personality(personality_config);

    // 从环境变量创建 LLM Provider (自动检测: ANTHROPIC > KIMI_CODE > KIMI > DEEPSEEK > OPENAI)
    let mut llm =
        LlmFactory::auto().map_err(|e| anyhow::anyhow!("Failed to create LLM provider: {}", e))?;
    let session_id = uuid::Uuid::new_v4().to_string();
    llm.set_prompt_cache_key(&session_id);

    // 检测实际使用的模型名称，与 provider 保持一致
    let model_name = detect_model_name();

    // 创建持久化记忆存储（放在当前工作目录下的 .clarity/memory.db）
    let memory_db_path = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("."))
        .join(".clarity")
        .join("memory.db");
    std::fs::create_dir_all(memory_db_path.parent().unwrap())?;
    let memory_store = Arc::new(PersistentMemoryStore::new(&memory_db_path).await?);

    // 创建记忆触发器（每 5 轮对话触发一次）
    let memory_ticker = MemoryTicker::new(5);

    // 创建 Agent
    let agent = Agent::with_config(registry, config)
        .with_llm(Arc::from(llm))
        .with_memory(memory_store)
        .with_memory_ticker(memory_ticker);

    Ok((Arc::new(agent), model_name, yuan_type))
}

/// Load `~/.config/clarity/mcp.json` and register available MCP tools.
async fn load_and_register_mcp_tools(registry: &ToolRegistry) {
    let config = match McpConfig::load_default() {
        Ok(cfg) => cfg,
        Err(e) => {
            println!("No MCP config found or failed to load: {}", e);
            return;
        }
    };

    let mut mcp_registry = McpRegistry::new();
    for (name, entry) in &config.servers {
        if entry.disabled {
            println!("MCP server '{}' is disabled, skipping", name);
            continue;
        }
        let server_config = clarity_core::mcp::McpServerConfig::stdio(name, &entry.command)
            .with_args(entry.args.clone())
            .with_envs(entry.env.clone());
        let client = McpClientBuilder::from_config(server_config);
        mcp_registry.register(name, client);
    }

    if let Err(e) = mcp_registry.connect_all().await {
        eprintln!("Failed to connect to one or more MCP servers: {}", e);
        return;
    }

    if let Err(e) = register_mcp_tools(&mcp_registry, registry).await {
        eprintln!("Failed to register MCP tools: {}", e);
    } else {
        let names = registry.list_tools().unwrap_or_default();
        let mcp_names: Vec<_> = names.into_iter().filter(|n| n.contains('_')).collect();
        println!("Registered MCP tools: {:?}", mcp_names);
    }
}

/// Detect the active model name from environment variables, matching LlmFactory::auto() logic.
fn detect_model_name() -> String {
    if std::env::var("ANTHROPIC_AUTH_TOKEN").is_ok() {
        return std::env::var("ANTHROPIC_MODEL")
            .unwrap_or_else(|_| "claude-3-sonnet-20240229".into());
    }
    if std::env::var("KIMI_CODE_API_KEY").is_ok() {
        return std::env::var("KIMI_CODE_MODEL").unwrap_or_else(|_| "kimi-k2-07132k".into());
    }
    if let Ok(kimi_key) = std::env::var("KIMI_API_KEY") {
        if kimi_key.starts_with("sk-kimi-") {
            return std::env::var("KIMI_CODE_MODEL")
                .or_else(|_| std::env::var("KIMI_MODEL"))
                .unwrap_or_else(|_| "kimi-k2-07132k".into());
        }
        return std::env::var("KIMI_MODEL").unwrap_or_else(|_| "kimi-k2-07132k".into());
    }
    if std::env::var("DEEPSEEK_API_KEY").is_ok() {
        return std::env::var("DEEPSEEK_MODEL").unwrap_or_else(|_| "deepseek-chat".into());
    }
    if std::env::var("OPENAI_API_KEY").is_ok() {
        return std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o".into());
    }
    "unknown".to_string()
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    let mut events = events::EventHandler::new();

    // 设置事件发送器，用于后台任务向主循环发送事件
    app.set_event_sender(events.get_sender());

    // 捕获 OS 级别的 SIGINT，发送 Interrupt 而不是直接退出进程
    if let Some(ref ctrl_tx) = app.controller_tx {
        let ctrl_tx = ctrl_tx.clone();
        let _ = ctrlc::set_handler(move || {
            let _ = ctrl_tx.send(clarity_core::agent::Op::Interrupt);
        });
    }

    loop {
        // 渲染界面
        terminal.draw(|f| ui::draw(f, app))?;

        // 处理事件
        match events.next_event().await {
            events::Event::Tick => {
                app.on_tick();
            }
            events::Event::Key(key) => {
                if !app.handle_key(key).await? {
                    break;
                }
            }
            events::Event::Resize(width, height) => {
                app.on_resize(width, height);
            }
            events::Event::MouseScroll(scroll) => match scroll {
                events::MouseScroll::Up => app.scroll_up(),
                events::MouseScroll::Down => app.scroll_down(),
            },
            events::Event::StreamResponse(chunk) => {
                app.handle_stream_chunk(chunk);
            }
            events::Event::ToolCall(tool) => {
                app.handle_tool_call(tool);
            }
            events::Event::ToolResult(tool) => {
                app.handle_tool_result(tool);
            }
            events::Event::ResponseComplete => {
                app.finish_generation();
            }
            events::Event::Error(err) => {
                app.handle_error(err);
            }
        }
    }

    Ok(())
}
