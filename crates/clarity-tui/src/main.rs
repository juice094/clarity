mod app;
mod events;
mod ui;
mod widgets;

use anyhow::Result;
use app::App;
use clarity_core::agent::{Agent, AgentConfig};
use clarity_core::llm::LlmFactory;
use clarity_core::memory::{MemoryTicker, PersistentMemoryStore};
use clarity_core::personality::{PersonalityConfig, YuanType};
use clarity_core::registry::ToolRegistry;
use crossterm::{
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
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 创建 Agent
    let agent = create_agent()?;

    // 创建应用
    let mut app = App::new(agent);

    // 运行应用
    let result = run_app(&mut terminal, &mut app).await;

    // 恢复终端
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn create_agent() -> Result<Arc<Agent>> {
    // 创建工具注册表
    let registry = ToolRegistry::with_builtin_tools();

    // 配置人格
    let personality_config = PersonalityConfig::new()
        .with_agent_name("Clarity")
        .with_user_name("User")
        .with_yuan_type(YuanType::Hanako)
        .with_locale("zh-CN");

    // 创建 Agent 配置
    let config = AgentConfig::default()
        .with_max_iterations(10)
        .with_read_only(false)
        .with_personality(personality_config);

    // 从环境变量创建 LLM Provider (自动检测: ANTHROPIC > KIMI > DEEPSEEK > OPENAI)
    let llm = LlmFactory::auto()
        .map_err(|e| anyhow::anyhow!("Failed to create LLM provider: {}", e))?;

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

    Ok(Arc::new(agent))
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
            events::Event::StreamResponse(chunk) => {
                app.handle_stream_chunk(chunk);
            }
            events::Event::ToolCall(tool) => {
                app.handle_tool_call(tool);
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
