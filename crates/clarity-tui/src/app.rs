use crate::events::ToolCallInfo;
use anyhow::Result;
use chrono::Local;
use clarity_core::agent::Agent;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

use crate::events::Event;

/// 消息类型
#[derive(Clone, Debug)]
pub enum MessageType {
    User,
    Assistant,
    System,
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
    /// 当前输入
    pub input: String,
    /// 输入光标位置
    pub cursor_position: usize,
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
    event_tx: Option<Sender<Event>>,
}

impl App {
    pub fn new(agent: Arc<Agent>) -> Self {
        // 从环境变量获取模型名称
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
            input: String::new(),
            cursor_position: 0,
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

    /// 设置事件发送器
    pub fn set_event_sender(&mut self, tx: Sender<Event>) {
        self.event_tx = Some(tx);
    }

    /// 处理按键事件
    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        match key.code {
            // Ctrl+C: 停止生成或退出
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.is_generating {
                    self.stop_generation();
                } else {
                    return Ok(false);
                }
            }
            // Ctrl+D: 退出
            KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Ok(false);
            }
            // 回车发送消息
            KeyCode::Enter => {
                self.submit_message().await?;
            }
            // 退格删除
            KeyCode::Backspace => {
                self.delete_char();
            }
            // 删除键
            KeyCode::Delete => {
                self.delete_char_forward();
            }
            // 左箭头
            KeyCode::Left => {
                self.move_cursor_left();
            }
            // 右箭头
            KeyCode::Right => {
                self.move_cursor_right();
            }
            // Home
            KeyCode::Home => {
                self.cursor_position = 0;
            }
            // End
            KeyCode::End => {
                self.cursor_position = self.input.len();
            }
            // 上箭头 - 历史记录
            KeyCode::Up => {
                self.scroll_up();
            }
            // 下箭头 - 历史记录
            KeyCode::Down => {
                self.scroll_down();
            }
            // 输入字符
            KeyCode::Char(c) => {
                self.insert_char(c);
            }
            _ => {}
        }

        Ok(true)
    }

    /// 插入字符
    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor_position, c);
        self.cursor_position += 1;
    }

    /// 删除字符
    pub fn delete_char(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
            self.input.remove(self.cursor_position);
        }
    }

    /// 向前删除字符
    pub fn delete_char_forward(&mut self) {
        if self.cursor_position < self.input.len() {
            self.input.remove(self.cursor_position);
        }
    }

    /// 移动光标左
    pub fn move_cursor_left(&mut self) {
        if self.cursor_position > 0 {
            self.cursor_position -= 1;
        }
    }

    /// 移动光标右
    pub fn move_cursor_right(&mut self) {
        if self.cursor_position < self.input.len() {
            self.cursor_position += 1;
        }
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
        let content = self.input.trim().to_string();
        if content.is_empty() {
            return Ok(());
        }

        // 清空输入
        self.input.clear();
        self.cursor_position = 0;

        // 处理命令
        if content.starts_with('/') {
            self.handle_command(&content).await;
            return Ok(());
        }

        // 添加用户消息
        self.messages.push(Message::new(&content, MessageType::User));

        // 开始生成响应
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

    /// 开始生成响应
    async fn start_generation(&mut self, user_input: String) {
        self.is_generating = true;

        // 添加一个空的助手消息（流式）
        self.messages
            .push(Message::new("", MessageType::Assistant).streaming());

        // 克隆需要移动到异步任务中的数据
        let agent = self.agent.clone();
        let event_tx = self.event_tx.clone();

        // 在后台任务中调用 LLM
        tokio::spawn(async move {
            match agent.run(&user_input).await {
                Ok(response) => {
                    // 将响应按字符分块发送，模拟流式效果
                    // 注意：这里是非真实的流式，真实流式需要修改 LLM Provider
                    let chunk_size = 5;
                    let chars: Vec<char> = response.chars().collect();
                    
                    for chunk in chars.chunks(chunk_size) {
                        let text: String = chunk.iter().collect();
                        if let Some(ref tx) = event_tx {
                            let _ = tx.send(Event::StreamResponse(text)).await;
                        }
                        // 小延迟以模拟打字效果
                        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                    }
                    
                    // 发送完成事件
                    if let Some(ref tx) = event_tx {
                        let _ = tx.send(Event::ResponseComplete).await;
                    }
                }
                Err(e) => {
                    // 发送错误事件
                    if let Some(ref tx) = event_tx {
                        let _ = tx.send(Event::Error(format!("LLM 错误: {}", e))).await;
                    }
                }
            }
        });
    }

    /// 停止生成
    pub fn stop_generation(&mut self) {
        if self.is_generating {
            self.is_generating = false;
            // 将最后一条消息标记为非流式
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
    pub fn handle_tool_call(&mut self, tool: ToolCallInfo) {
        let tool_msg = format!("[使用 {}] {}", tool.name, tool.params);
        self.messages.push(Message::new(tool_msg, MessageType::ToolCall));
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
        // 创建一个空的 Agent 用于 default，实际使用时会替换
        let registry = ToolRegistry::with_builtin_tools();
        let config = clarity_core::agent::AgentConfig::default();
        let agent = Arc::new(Agent::with_config(registry, config));
        Self::new(agent)
    }
}

// 引入需要的类型
use clarity_core::registry::ToolRegistry;
