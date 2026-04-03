use crate::events::ToolCallInfo;
use anyhow::Result;
use chrono::Local;
use clarity_core::agent::Agent;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

use crate::events::Event;
use crate::widgets::input_pane::InputPane;

/// 消息类型
#[derive(Clone, Debug)]
pub enum MessageType {
    User,
    Assistant,
    System,
    #[allow(dead_code)]
    ToolCall,
}

/// 聊天消息
#[derive(Clone, Debug)]
pub struct Message {
    pub content: String,
    pub msg_type: MessageType,
    pub timestamp: String,
    pub is_streaming: bool,
}

impl Message {
    pub fn new(content: impl Into<String>, msg_type: MessageType) -> Self {
        Self {
            content: content.into(),
            msg_type,
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            is_streaming: false,
        }
    }

    pub fn streaming(mut self) -> Self {
        self.is_streaming = true;
        self
    }
}

/// 应用状态
pub struct App {
    /// 聊天历史
    pub messages: Vec<Message>,
    /// 输入框组件
    pub input_pane: InputPane,
    /// 是否运行中
    pub running: bool,
    /// 是否正在生成响应
    pub is_generating: bool,
    /// 当前模型名称
    pub model_name: String,
    /// 会话ID
    pub session_id: String,
    /// 滚动位置
    pub scroll_offset: usize,
    /// 窗口大小
    pub terminal_size: (u16, u16),
    /// 输入框高度
    pub input_height: u16,
    /// Agent 实例
    agent: Arc<Agent>,
    /// 事件发送器（用于后台任务发送事件）
    event_tx: Option<UnboundedSender<Event>>,
}

impl App {
    pub fn new(agent: Arc<Agent>) -> Self {
        let model_name = std::env::var("ANTHROPIC_MODEL")
            .or_else(|_| std::env::var("KIMI_MODEL"))
            .unwrap_or_else(|_| "default".to_string());

        Self {
            messages: vec![
                Message::new(
                    "欢迎使用 Clarity! 输入 /help 查看可用命令。",
                    MessageType::System,
                ),
            ],
            input_pane: InputPane::new(),
            running: true,
            is_generating: false,
            model_name,
            session_id: format!("session_{}", Local::now().timestamp()),
            scroll_offset: 0,
            terminal_size: (80, 24),
            input_height: 3,
            agent,
            event_tx: None,
        }
    }

    /// 向后兼容的 input 访问器
    #[allow(dead_code)]
    pub fn input(&self) -> &str {
        self.input_pane.input()
    }

    /// 向后兼容的光标位置访问器
    #[allow(dead_code)]
    pub fn cursor_position(&self) -> usize {
        self.input_pane.cursor_position()
    }

    /// 设置事件发送器
    pub fn set_event_sender(&mut self, tx: UnboundedSender<Event>) {
        self.event_tx = Some(tx);
    }

    /// 处理按键事件
    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.is_generating {
                    self.stop_generation();
                } else {
                    return Ok(false);
                }
            }
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(false);
            }
            KeyCode::Enter => {
                self.submit_message().await?;
            }
            KeyCode::Backspace => {
                self.input_pane.delete_char();
            }
            KeyCode::Delete => {
                self.input_pane.delete_char_forward();
            }
            KeyCode::Left => {
                self.input_pane.move_cursor_left();
            }
            KeyCode::Right => {
                self.input_pane.move_cursor_right();
            }
            KeyCode::Home => {
                self.input_pane.set_cursor_position(0);
            }
            KeyCode::End => {
                let len = self.input_pane.input().chars().count();
                self.input_pane.set_cursor_position(len);
            }
            KeyCode::Up => {
                self.scroll_up();
            }
            KeyCode::Down => {
                self.scroll_down();
            }
            KeyCode::Char(c) => {
                // Only handle key press, ignore repeat/release
                if key.kind == crossterm::event::KeyEventKind::Press {
                    self.input_pane.insert_char(c);
                }
            }
            _ => {}
        }

        Ok(true)
    }

    /// 滚动上
    pub fn scroll_up(&mut self) {
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
        }
    }

    /// 滚动下
    pub fn scroll_down(&mut self) {
        let max_scroll = self.messages.len().saturating_sub(1);
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        }
    }

    /// 提交消息
    async fn submit_message(&mut self) -> Result<()> {
        let content = self.input_pane.input().trim().to_string();
        if content.is_empty() {
            return Ok(());
        }

        self.input_pane.clear();

        if content.starts_with('/') {
            self.handle_command(&content).await;
            return Ok(());
        }

        self.messages.push(Message::new(&content, MessageType::User));
        self.start_generation(content).await;

        Ok(())
    }

    /// 处理命令
    async fn handle_command(&mut self, cmd: &str) {
        match cmd {
            "/exit" | "/quit" | "/q" => {
                self.running = false;
            }
            "/clear" | "/cls" => {
                self.messages.clear();
                self.messages.push(Message::new(
                    "屏幕已清空。输入 /help 查看可用命令。",
                    MessageType::System,
                ));
                self.scroll_offset = 0;
            }
            "/help" | "/h" => {
                let help_text = r#"可用命令:
  /exit, /quit, /q    - 退出程序
  /clear, /cls        - 清空屏幕
  /help, /h           - 显示帮助
  /model              - 显示当前模型
  /stop               - 停止生成

快捷键:
  Ctrl+C              - 停止生成 / 退出
  Ctrl+D              - 退出
  ↑ / ↓               - 滚动聊天记录
  Home / End          - 移动光标到行首/行尾"#;
                self.messages
                    .push(Message::new(help_text, MessageType::System));
            }
            "/stop" => {
                self.stop_generation();
            }
            "/model" => {
                self.messages.push(Message::new(
                    format!("当前模型: {}", self.model_name),
                    MessageType::System,
                ));
            }
            _ => {
                self.messages.push(Message::new(
                    format!("未知命令: {}。输入 /help 查看可用命令。", cmd),
                    MessageType::System,
                ));
            }
        }
    }

    /// 开始生成响应（真实流式）
    async fn start_generation(&mut self, user_input: String) {
        self.is_generating = true;
        self.messages
            .push(Message::new("", MessageType::Assistant).streaming());

        let agent = self.agent.clone();
        let event_tx = self.event_tx.clone();
        let chunk_tx = event_tx.clone();

        tokio::spawn(async move {
            let result = agent.run_streaming(&user_input, move |chunk: &str| {
                if let Some(ref tx) = chunk_tx {
                    let _ = tx.send(Event::StreamResponse(chunk.to_string()));
                }
            }).await;

            match result {
                Ok(_) => {
                    if let Some(ref tx) = event_tx {
                        let _ = tx.send(Event::ResponseComplete);
                    }
                }
                Err(e) => {
                    if let Some(ref tx) = event_tx {
                        let _ = tx.send(Event::Error(format!("LLM 错误: {}", e)));
                    }
                }
            }
        });
    }

    /// 停止生成
    pub fn stop_generation(&mut self) {
        if self.is_generating {
            self.is_generating = false;
            if let Some(last) = self.messages.last_mut() {
                last.is_streaming = false;
            }
            self.messages.push(Message::new(
                "⏹️ 生成已停止。",
                MessageType::System,
            ));
        }
    }

    /// 完成生成
    pub fn finish_generation(&mut self) {
        self.is_generating = false;
        if let Some(last) = self.messages.last_mut() {
            last.is_streaming = false;
        }
    }

    /// 处理错误
    pub fn handle_error(&mut self, error: String) {
        self.is_generating = false;
        if let Some(last) = self.messages.last_mut() {
            last.is_streaming = false;
        }
        self.messages.push(Message::new(error, MessageType::System));
    }

    /// 处理流式响应块
    pub fn handle_stream_chunk(&mut self, chunk: String) {
        if let Some(last) = self.messages.last_mut() {
            if matches!(last.msg_type, MessageType::Assistant) && last.is_streaming {
                last.content.push_str(&chunk);
            }
        }
    }

    /// 处理工具调用
    pub fn handle_tool_call(&mut self, _tool: ToolCallInfo) {
        // Tool call visualization is handled inline in chat messages
    }

    /// 时钟滴答
    pub fn on_tick(&mut self) {
        // 可以在这里处理定时任务
    }

    /// 窗口大小变化
    pub fn on_resize(&mut self, width: u16, height: u16) {
        self.terminal_size = (width, height);
    }
}

impl Default for App {
    fn default() -> Self {
        let registry = clarity_core::registry::ToolRegistry::with_builtin_tools();
        let config = clarity_core::agent::AgentConfig::default();
        let agent = Arc::new(Agent::with_config(registry, config));
        Self::new(agent)
    }
}
