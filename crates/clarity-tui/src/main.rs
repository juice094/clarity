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
    let (agent, model_name, yuan_type) = create_agent()?;

    // 创建应用
    let mut app = App::new(agent, model_name, yuan_type.to_string());

    // 运行应用
    let result = run_app(&mut terminal, &mut app).await;

    // 恢复终端
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    result
}

fn create_agent() -> Result<(Arc<Agent>, String, YuanType)> {
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

    // 创建 Agent 配置
    let config = AgentConfig::default()
        .with_max_iterations(10)
        .with_read_only(false)
        .with_personality(personality_config);

    // 从环境变量创建 LLM Provider (自动检测: ANTHROPIC > KIMI_CODE > KIMI > DEEPSEEK > OPENAI)
    let mut llm = LlmFactory::auto()
        .map_err(|e| anyhow::anyhow!("Failed to create LLM provider: {}", e))?;
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
    let memory_store = Arc::new(PersistentMemoryStore::new(&memory_db_path)?);

    // 创建记忆触发器（每 5 轮对话触发一次）
    let memory_ticker = MemoryTicker::new(5);

    // 创建 Agent
    let agent = Agent::with_config(registry, config)
        .with_llm(Arc::from(llm))
        .with_memory(memory_store)
        .with_memory_ticker(memory_ticker);

    Ok((Arc::new(agent), model_name, yuan_type))
}

/// Detect the active model name from environment variables, matching LlmFactory::auto() logic.
fn detect_model_name() -> String {
    if std::env::var("ANTHROPIC_AUTH_TOKEN").is_ok() {
        return std::env::var("ANTHROPIC_MODEL")
            .unwrap_or_else(|_| "claude-3-sonnet-20240229".into());
    }
    if std::env::var("KIMI_CODE_API_KEY").is_ok() {
        return std::env::var("KIMI_CODE_MODEL")
            .unwrap_or_else(|_| "kimi-k2-07132k".into());
    }
    if let Ok(kimi_key) = std::env::var("KIMI_API_KEY") {
        if kimi_key.starts_with("sk-kimi-") {
            return std::env::var("KIMI_CODE_MODEL")
                .or_else(|_| std::env::var("KIMI_MODEL"))
                .unwrap_or_else(|_| "kimi-k2-07132k".into());
        }
        return std::env::var("KIMI_MODEL")
            .unwrap_or_else(|_| "kimi-k2-07132k".into());
    }
    if std::env::var("DEEPSEEK_API_KEY").is_ok() {
        return std::env::var("DEEPSEEK_MODEL")
            .unwrap_or_else(|_| "deepseek-chat".into());
    }
    if std::env::var("OPENAI_API_KEY").is_ok() {
        return std::env::var("OPENAI_MODEL")
            .unwrap_or_else(|_| "gpt-4o".into());
    }
    "unknown".to_string()
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    let mut events = events::EventHandler::new();

    // 设置事件发送器，用于后台任务向主循环发送事件
    app.set_event_sender(events.get_sender());

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
            events::Event::MouseScroll(scroll) => {
                match scroll {
                    events::MouseScroll::Up => app.scroll_up(),
                    events::MouseScroll::Down => app.scroll_down(),
                }
            }
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
