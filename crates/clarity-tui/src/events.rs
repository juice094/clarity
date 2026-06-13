use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEventKind};
use std::time::Duration;
use tokio::sync::mpsc;

/// Direction of a mouse wheel event.
#[derive(Clone, Debug)]
pub enum MouseScroll {
    /// Wheel scrolled up.
    Up,
    /// Wheel scrolled down.
    Down,
}

/// Metadata about a tool invocation or result.
#[derive(Clone, Debug)]
pub struct ToolCallInfo {
    /// Tool name.
    pub name: String,
    /// Serialized arguments or result payload.
    pub params: String,
    /// Current execution status.
    // Intentionally retained: the wire adapter currently only emits
    // `Running`/`Success`, but the field is part of the public event contract.
    #[allow(dead_code)]
    pub status: ToolStatus,
}

/// Lifecycle status of a tool call.
#[derive(Clone, Debug)]
pub enum ToolStatus {
    /// Tool is still running.
    Running,
    /// Tool completed successfully.
    Success,
    /// Tool failed.
    // Intentionally retained: reserved for future error visualization.
    #[allow(dead_code)]
    Error,
}

/// Event produced by the input loop and consumed by the TUI application.
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
    // Intentionally retained: consumed by the main loop; reserved for future
    // producers that emit structured errors.
    #[allow(dead_code)]
    Error(String),
    /// Token usage update
    Usage {
        prompt_tokens: u32,
        completion_tokens: u32,
        total_tokens: u32,
    },
}

/// 事件处理器
pub struct EventHandler {
    rx: mpsc::UnboundedReceiver<Event>,
    tx: mpsc::UnboundedSender<Event>,
}

impl EventHandler {
    /// Spawn the background event loop that forwards crossterm events into the
    /// returned channel.
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
