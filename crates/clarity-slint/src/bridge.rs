//! UI 与 tokio 运行时之间的最小桥接层。
//!
//! Slint 的 UI 句柄与组件不是线程安全的，所有跨线程的状态更新
//! 必须通过 [`slint::invoke_from_event_loop`] 回到 UI 线程执行。
//!
//! 阶段 1 保留结构但暂不主动驱动 UI；阶段 2 将用其把 LLM 流式
//! 结果注入 `ChatArea` 的消息列表。

use slint::SharedString;
use std::sync::Arc;
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

/// 从 UI 线程发到后台线程的请求。
#[derive(Debug, Clone)]
pub enum UiRequest {
    /// 发送一条用户消息，由后台任务处理。
    SendMessage {
        /// 用户输入文本。
        text: String,
    },
}

/// 从后台线程发回 UI 线程的响应。
#[derive(Debug, Clone)]
pub enum UiResponse {
    /// 后台已成功处理消息。
    MessageReceived {
        /// 回复文本。
        text: String,
    },
    /// 后台处理失败。
    Error {
        /// 错误信息。
        message: String,
    },
}

/// 桥接器：持有 tokio runtime 和 UI 回调句柄。
#[derive(Clone)]
pub struct Bridge {
    runtime: Arc<Runtime>,
    ui_tx: slint::Weak<crate::ui::AppWindow>,
    req_tx: mpsc::UnboundedSender<UiRequest>,
}

impl Bridge {
    /// 创建新的桥接器，返回 `(Bridge, 请求接收端)`。
    ///
    /// # Errors
    /// 当 tokio runtime 构建失败时返回错误。
    pub fn new(
        ui: slint::Weak<crate::ui::AppWindow>,
    ) -> anyhow::Result<(Self, mpsc::UnboundedReceiver<UiRequest>)> {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .worker_threads(2)
                .thread_name("clarity-slint-worker")
                .build()?,
        );

        let (req_tx, req_rx) = mpsc::unbounded_channel();

        Ok((
            Bridge {
                runtime,
                ui_tx: ui,
                req_tx,
            },
            req_rx,
        ))
    }

    /// UI 按钮点击时调用：把请求丢进 tokio 任务队列。
    pub fn send_message(&self, text: String) {
        let _ = self.req_tx.send(UiRequest::SendMessage { text });
    }

    /// 在 tokio 任务中调用，通过闭包安全回写 Slint UI。
    ///
    /// 闭包 `f` 会在 UI 事件循环中执行，接收一个 `AppWindow` 句柄，
    /// 可在其中调用任意 setter。要求 `f` 满足 `Send + 'static`。
    pub fn respond<F>(&self, f: F)
    where
        F: FnOnce(crate::ui::AppWindow) + Send + 'static,
    {
        let ui = self.ui_tx.clone();
        // 必须走 invoke_from_event_loop：Slint UI 不是线程安全句柄
        let _ = slint::invoke_from_event_loop(move || {
            ui.upgrade_in_event_loop(f).ok();
        });
    }

    /// 便捷的响应封装：更新 Bot 头部名称。
    ///
    /// 阶段 2 将替换为向消息列表追加条目。
    pub fn respond_with_bot_name(&self, text: impl Into<SharedString>) {
        let text = text.into();
        self.respond(move |window| {
            window.set_bot_name(text);
        });
    }

    /// 返回共享的 tokio runtime 句柄。
    pub fn runtime(&self) -> Arc<Runtime> {
        self.runtime.clone()
    }

    /// 返回 UI 弱引用。
    pub fn ui_handle(&self) -> slint::Weak<crate::ui::AppWindow> {
        self.ui_tx.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_request_round_trip() {
        let req = UiRequest::SendMessage {
            text: "hello".to_string(),
        };
        let cloned = req.clone();
        assert!(matches!(cloned, UiRequest::SendMessage { text } if text == "hello"));
        assert_eq!(format!("{:?}", req), "SendMessage { text: \"hello\" }");
    }

    #[test]
    fn ui_response_round_trip() {
        let resp = UiResponse::MessageReceived {
            text: "world".to_string(),
        };
        let cloned = resp.clone();
        assert!(matches!(cloned, UiResponse::MessageReceived { text } if text == "world"));
    }
}
