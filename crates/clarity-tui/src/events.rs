#![allow(dead_code)]

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEventKind};

#[derive(Clone, Debug)]
pub enum MouseScroll {
    Up,
    Down,
}
use std::time::Duration;
use tokio::sync::mpsc;

/// 工具调用信息
#[derive(Clone, Debug)]
pub struct ToolCallInfo {
    pub name: String,
    pub params: String,
    pub status: ToolStatus,
}

#[derive(Clone, Debug)]
pub enum ToolStatus {
    Running,
    Success,
    Error,
}

/// 应用事件
#[derive(Debug)]
pub enum Event {
    /// 定时滴答
    Tick,
    /// 按键事件
    Key(KeyEvent),
    /// 窗口大小变化
    Resize(u16, u16),
    /// 鼠标滚轮
    MouseScroll(MouseScroll),
    /// 流式响应
    StreamResponse(String),
    /// 响应完成
    ResponseComplete,
    /// 工具调用
    ToolCall(ToolCallInfo),
    /// 工具结果
    ToolResult(ToolCallInfo),
    /// 错误
    Error(String),
    /// Token usage update
    Usage {
        prompt_tokens: u32,
        completion_tokens: u32,
        total_tokens: u32,
    },
    /// Declarative UI command batch from the wire view channel.
    ViewUpdate(Vec<clarity_wire::ViewCommand>),
}

/// 事件处理器
pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
    tx: mpsc::UnboundedSender<Event>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        let tx_clone = tx.clone();

        // 启动事件监听任务
        tokio::spawn(async move {
            let mut tick_interval = tokio::time::interval(Duration::from_millis(50));

            loop {
                tokio::select! {
                    // 定时滴答
                    _ = tick_interval.tick() => {
                        let _ = tx.send(Event::Tick);
                    }
                    // 处理终端事件
                    Ok(()) = async {
                        if event::poll(Duration::from_millis(100))? {
                            match event::read()? {
                                CrosstermEvent::Key(key) => {
                                    let _ = tx.send(Event::Key(key));
                                }
                                CrosstermEvent::Resize(width, height) => {
                                    let _ = tx.send(Event::Resize(width, height));
                                }
                                CrosstermEvent::Mouse(mouse_event) => {
                                    match mouse_event.kind {
                                        MouseEventKind::ScrollUp => {
                                            let _ = tx.send(Event::MouseScroll(MouseScroll::Up));
                                        }
                                        MouseEventKind::ScrollDown => {
                                            let _ = tx.send(Event::MouseScroll(MouseScroll::Down));
                                        }
                                        _ => {}
                                    }
                                }
                                _ => {}
                            }
                        }
                        Ok::<(), std::io::Error>(())
                    } => {}
                }
            }
        });

        Self { rx, tx: tx_clone }
    }

    /// 获取事件发送器
    pub fn get_sender(&self) -> mpsc::UnboundedSender<Event> {
        self.tx.clone()
    }

    /// 获取下一个事件
    pub async fn next_event(&mut self) -> Event {
        self.rx.recv().await.unwrap_or(Event::Tick)
    }
}

impl Default for EventHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_event_handler_sender() {
        let handler = EventHandler::new();
        let tx = handler.get_sender();
        assert!(tx.send(Event::Tick).is_ok());
    }

    #[test]
    fn test_tool_call_info_clone() {
        let info = ToolCallInfo {
            name: "test".to_string(),
            params: "{}".to_string(),
            status: ToolStatus::Running,
        };
        let cloned = info.clone();
        assert_eq!(cloned.name, "test");
        assert!(matches!(cloned.status, ToolStatus::Running));
    }

    #[tokio::test]
    async fn test_event_roundtrip() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        tx.send(Event::StreamResponse("hi".to_string())).unwrap();
        let ev = rx.recv().await.unwrap();
        assert!(matches!(ev, Event::StreamResponse(s) if s == "hi"));
    }
}
