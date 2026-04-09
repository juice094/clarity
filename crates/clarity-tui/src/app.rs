use crate::async_job::ToolCallJob;
use crate::diff::compute_diff;
use crate::events::ToolCallInfo;
use crate::popup::{EventState, Popup};
use crate::popups::{diff_popup::DiffPopup, HelpPopup, ToolResultPopup};
use anyhow::Result;
use chrono::Local;
use clarity_core::agent::{Agent, AgentController, Op};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;

use crate::events::Event;
use crate::widgets::input_pane::InputPane;
use crate::wire_adapter::spawn_wire_adapter;

/// 消息类型
#[derive(Clone, Debug, PartialEq)]
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

/// 应用模式
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AppMode {
    Normal,
    Input,
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
    /// 滚动偏移（以行为单位）
    pub scroll_offset: usize,
    /// 窗口大小
    pub terminal_size: (u16, u16),
    /// 输入框高度
    pub input_height: u16,
    /// 当前模式
    pub mode: AppMode,
    /// 当前弹窗
    pub popup: Option<Box<dyn Popup>>,
    /// 后台任务（工具调用）
    #[allow(dead_code)]
    pub async_job: ToolCallJob,
    /// Agent 实例
    agent: Arc<Agent>,
    /// 事件发送器（用于后台任务发送事件）
    event_tx: Option<UnboundedSender<Event>>,
    /// AgentController 操作发送器
    controller_tx: Option<UnboundedSender<Op>>,
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
            mode: AppMode::Input,
            popup: None,
            async_job: ToolCallJob::new(),
            agent,
            event_tx: None,
            controller_tx: None,
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
        self.event_tx = Some(tx.clone());
        let wire = std::sync::Arc::new(clarity_wire::Wire::new());
        let agent = (*self.agent).clone().with_wire(wire.clone());
        spawn_wire_adapter(wire.ui_side(false), tx);
        let (controller, controller_tx) = AgentController::new_with_sender(agent);
        tokio::spawn(controller.run());
        self.controller_tx = Some(controller_tx);
    }

    /// 处理按键事件
    pub async fn handle_key(&mut self, key: KeyEvent) -> Result<bool> {
        // 全局快捷键（Ctrl+C / Ctrl+D）
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => {
                    if self.is_generating {
                        self.stop_generation();
                        return Ok(true);
                    } else {
                        return Ok(false);
                    }
                }
                KeyCode::Char('d') => {
                    return Ok(false);
                }
                _ => {}
            }
        }

        // 优先将事件路由给弹窗
        if let Some(ref mut popup) = self.popup {
            let event = crossterm::event::Event::Key(key);
            match popup.handle_event(event) {
                EventState::Consumed => {
                    if popup.is_done() {
                        self.popup = None;
                    }
                    return Ok(true);
                }
                EventState::NotConsumed => {}
            }
        }

        match self.mode {
            AppMode::Normal => {
                match key.code {
                    KeyCode::Char('q') => return Ok(false),
                    KeyCode::Char('?') => {
                        self.popup = Some(Box::new(HelpPopup::new()));
                    }
                    KeyCode::Char('i') | KeyCode::Enter => {
                        self.mode = AppMode::Input;
                    }
                    KeyCode::Char('j') | KeyCode::Down => self.scroll_down(),
                    KeyCode::Char('k') | KeyCode::Up => self.scroll_up(),
                    KeyCode::Char('G') => {
                        let total = self.total_content_lines();
                        let visible = self.visible_chat_height();
                        self.scroll_offset = total.saturating_sub(visible);
                    }
                    KeyCode::Char('g') => {
                        self.scroll_offset = 0;
                    }
                    _ => {}
                }
            }
            AppMode::Input => {
                match key.code {
                    KeyCode::Esc => {
                        self.mode = AppMode::Normal;
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
            }
        }

        Ok(true)
    }

    /// Approximate total number of rendered content lines.
    fn total_content_lines(&self) -> usize {
        let mut total = 0usize;
        for msg in &self.messages {
            // header line(s)
            match msg.msg_type {
                MessageType::User | MessageType::Assistant => total += 1,
                MessageType::ToolCall => total += 1,
                MessageType::System => {}
            }
            // content lines
            let content_lines = msg.content.lines().count();
            total += content_lines;
            // streaming cursor indicator
            if msg.is_streaming && matches!(msg.msg_type, MessageType::Assistant) {
                total += 1;
            }
            // spacer between messages
            total += 1;
        }
        total
    }

    /// 可见聊天区域高度
    fn visible_chat_height(&self) -> usize {
        // 状态栏 1 行 + 输入框 input_height 行 + 命令栏 1 行
        self.terminal_size
            .1
            .saturating_sub(1 + self.input_height + 1) as usize
    }

    /// 滚动上（按行）
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
    }

    /// 滚动下（按行）
    pub fn scroll_down(&mut self) {
        let total = self.total_content_lines();
        let visible = self.visible_chat_height();
        let max_scroll = total.saturating_sub(visible);
        self.scroll_offset = (self.scroll_offset + 1).min(max_scroll);
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

    /// 开始生成响应（通过 clarity-wire 接收流式事件）
    async fn start_generation(&mut self, user_input: String) {
        self.is_generating = true;
        self.messages
            .push(Message::new("", MessageType::Assistant).streaming());

        if let Some(ref controller_tx) = self.controller_tx {
            let _ = controller_tx.send(Op::UserTurn(user_input));
            return;
        }

        // Fallback: direct spawn without controller
        let event_tx = self.event_tx.clone();

        // 创建 Wire 并绑定到 Agent 副本
        let wire = std::sync::Arc::new(clarity_wire::Wire::new());
        let agent = (*self.agent).clone().with_wire(wire.clone());

        // 启动适配器，将 WireMessage 转成 Event
        if let Some(ref tx) = event_tx {
            spawn_wire_adapter(wire.ui_side(false), tx.clone());
        }

        tokio::spawn(async move {
            let result = agent.run_streaming(&user_input, |_chunk: &str| {
                // 所有流式内容通过 wire 传递，回调留空
            }).await;

            match result {
                Ok(_) => {
                    // TurnEnd 已经由 wire adapter 转换为 ResponseComplete
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
            if let Some(ref controller_tx) = self.controller_tx {
                let _ = controller_tx.send(Op::Interrupt);
            }
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
    pub fn handle_tool_call(&mut self, tool: ToolCallInfo) {
        let text = if tool.params.is_empty() {
            format!("🔧 调用工具: {}", tool.name)
        } else {
            format!("🔧 调用工具: {} 参数: {}", tool.name, tool.params)
        };
        self.messages.push(Message::new(text, MessageType::ToolCall));
    }

    /// 处理工具结果
    pub fn handle_tool_result(&mut self, tool: ToolCallInfo) {
        let text = format!("✅ 工具结果: {}", tool.params);
        self.messages.push(Message::new(text, MessageType::ToolCall));

        let has_diff = serde_json::from_str::<serde_json::Value>(&tool.params)
            .ok()
            .and_then(|json| json.get("_diff_preview").cloned())
            .is_some();

        if has_diff {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&tool.params) {
                if let Some(diff_preview) = json.get("_diff_preview") {
                    if let (Some(path), Some(old), Some(new)) = (
                        json.get("path").and_then(|v| v.as_str()),
                        diff_preview.get("old").and_then(|v| v.as_str()),
                        diff_preview.get("new").and_then(|v| v.as_str()),
                    ) {
                        self.popup = Some(Box::new(DiffPopup::new(
                            path.to_string(),
                            compute_diff(old, new),
                        )));
                        return;
                    }
                }
            }
        }

        self.popup = Some(Box::new(ToolResultPopup::new(
            format!("Tool: {}", tool.name),
            tool.params,
        )));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::ToolStatus;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn test_app() -> App {
        App::default()
    }

    #[test]
    fn test_app_new_initial_state() {
        let app = test_app();
        assert_eq!(app.messages.len(), 1);
        assert!(matches!(app.messages[0].msg_type, MessageType::System));
        assert!(!app.is_generating);
        assert_eq!(app.scroll_offset, 0);
        assert_eq!(app.mode, AppMode::Input);
        assert!(app.popup.is_none());
    }

    #[test]
    fn test_add_message() {
        let mut app = test_app();
        app.messages.push(Message::new("hello", MessageType::User));
        assert_eq!(app.messages.len(), 2);
        assert_eq!(app.messages.last().unwrap().content, "hello");
    }

    #[test]
    fn test_scroll_up_down() {
        let mut app = test_app();
        app.messages.push(Message::new("a\nb\nc\nd\ne", MessageType::User));
        app.terminal_size = (80, 10);
        app.input_height = 3;

        let initial_offset = app.scroll_offset;
        app.scroll_down();
        assert!(app.scroll_offset > initial_offset);

        let after_down = app.scroll_offset;
        app.scroll_up();
        assert_eq!(app.scroll_offset, after_down - 1);
    }

    #[test]
    fn test_scroll_up_does_not_underflow() {
        let mut app = test_app();
        app.scroll_offset = 0;
        app.scroll_up();
        assert_eq!(app.scroll_offset, 0);
    }

    #[test]
    fn test_handle_tool_call() {
        let mut app = test_app();
        app.handle_tool_call(ToolCallInfo {
            name: "read_file".to_string(),
            params: "{\"path\":\"/tmp\"}".to_string(),
            status: ToolStatus::Running,
        });
        assert_eq!(app.messages.len(), 2);
        let last = app.messages.last().unwrap();
        assert!(matches!(last.msg_type, MessageType::ToolCall));
        assert!(last.content.contains("read_file"));
    }

    #[test]
    fn test_handle_tool_result() {
        let mut app = test_app();
        app.handle_tool_result(ToolCallInfo {
            name: "result".to_string(),
            params: "ok".to_string(),
            status: ToolStatus::Success,
        });
        let last = app.messages.last().unwrap();
        assert!(last.content.contains("ok"));
        assert!(app.popup.is_some());
    }

    #[test]
    fn test_finish_generation() {
        let mut app = test_app();
        app.messages.push(Message::new("", MessageType::Assistant).streaming());
        app.is_generating = true;
        app.finish_generation();
        assert!(!app.is_generating);
        assert!(!app.messages.last().unwrap().is_streaming);
    }

    #[test]
    fn test_handle_error() {
        let mut app = test_app();
        app.is_generating = true;
        app.messages.push(Message::new("", MessageType::Assistant).streaming());
        app.handle_error("boom".to_string());
        assert!(!app.is_generating);
        let last = app.messages.last().unwrap();
        assert_eq!(last.content, "boom");
        assert!(matches!(last.msg_type, MessageType::System));
    }

    #[tokio::test]
    async fn test_handle_key_exit_ctrl_d() {
        let mut app = test_app();
        let key = KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL);
        let keep = app.handle_key(key).await.unwrap();
        assert!(!keep);
    }

    #[tokio::test]
    async fn test_app_event_routing_to_popup_consumed() {
        let mut app = test_app();
        app.popup = Some(Box::new(HelpPopup::new()));
        let key = KeyEvent::new(KeyCode::Esc, KeyModifiers::empty());
        let keep = app.handle_key(key).await.unwrap();
        assert!(keep);
        assert!(app.popup.is_none());
    }

    #[tokio::test]
    async fn test_app_open_help_popup_in_normal_mode() {
        let mut app = test_app();
        app.mode = AppMode::Normal;
        let key = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::empty());
        let keep = app.handle_key(key).await.unwrap();
        assert!(keep);
        assert!(app.popup.is_some());
    }
}
