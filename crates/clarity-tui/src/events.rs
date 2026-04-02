use crossterm::event::{self, Event as CrosstermEvent, KeyEvent};
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
    /// 流式响应
    StreamResponse(String),
    /// 响应完成
    ResponseComplete,
    /// 工具调用
    ToolCall(ToolCallInfo),
    /// 错误
    Error(String),
}

/// 事件处理器
pub struct EventHandler {
    rx: mpsc::Receiver<Event>,
    tx: mpsc::Sender<Event>,
}

impl EventHandler {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::channel(100);
        let tx_clone = tx.clone();

        // 启动事件监听任务
        tokio::spawn(async move {
            let mut tick_interval = tokio::time::interval(Duration::from_millis(250));

            loop {
                tokio::select! {
                    // 定时滴答
                    _ = tick_interval.tick() => {
                        let _ = tx.send(Event::Tick).await;
                    }
                    // 处理终端事件
                    Ok(()) = async {
                        if event::poll(Duration::from_millis(100))? {
                            match event::read()? {
                                CrosstermEvent::Key(key) => {
                                    let _ = tx.send(Event::Key(key)).await;
                                }
                                CrosstermEvent::Resize(width, height) => {
                                    let _ = tx.send(Event::Resize(width, height)).await;
                                }
                                _ => {}
                            }
                        }
                        Ok::<(), std::io::Error>(())
                    } => {}
                }
            }
        });

        Self {
            rx,
            tx: tx_clone,
        }
    }

    /// 获取事件发送器
    pub fn get_sender(&self) -> mpsc::Sender<Event> {
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
