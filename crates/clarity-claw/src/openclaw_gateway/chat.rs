//! OpenClaw Gateway chat API.
//!
//! Wrappers for `chat.send`, `chat.history`, and `chat.abort`, plus helpers to
//! extract text from server-side chat events.

use crate::openclaw_gateway::client::{OpenClawClientError, OpenClawGatewayClient};
use crate::openclaw_gateway::protocol::methods;
use crate::openclaw_gateway::types::{
    ChatBlock, ChatHistory, ChatHistoryParams, ChatSendParams, OpenClawMessage,
};

/// Chat methods for [`OpenClawGatewayClient`].
#[async_trait::async_trait]
pub trait OpenClawChatApi {
    /// Send a chat message to a session.
    async fn chat_send(
        &self,
        session_key: &str,
        blocks: Vec<ChatBlock>,
    ) -> Result<OpenClawMessage, OpenClawClientError>;

    /// Send a plain-text chat message to a session.
    async fn chat_send_text(
        &self,
        session_key: &str,
        text: &str,
    ) -> Result<OpenClawMessage, OpenClawClientError>;

    /// Fetch chat history for a session.
    async fn chat_history(
        &self,
        session_key: &str,
        limit: Option<usize>,
    ) -> Result<ChatHistory, OpenClawClientError>;

    /// Abort the current in-flight generation in a session.
    async fn chat_abort(&self, session_key: &str) -> Result<(), OpenClawClientError>;
}

#[async_trait::async_trait]
impl OpenClawChatApi for OpenClawGatewayClient {
    async fn chat_send(
        &self,
        session_key: &str,
        blocks: Vec<ChatBlock>,
    ) -> Result<OpenClawMessage, OpenClawClientError> {
        let params = ChatSendParams {
            session_key: session_key.to_string(),
            message: blocks,
            stream: Some(false),
        };
        let value = self
            .call(methods::CHAT_SEND, Some(serde_json::to_value(params)?))
            .await?;
        serde_json::from_value(value).map_err(|e| OpenClawClientError::Other(e.into()))
    }

    async fn chat_send_text(
        &self,
        session_key: &str,
        text: &str,
    ) -> Result<OpenClawMessage, OpenClawClientError> {
        self.chat_send(session_key, vec![ChatBlock::Text { text: text.into() }])
            .await
    }

    async fn chat_history(
        &self,
        session_key: &str,
        limit: Option<usize>,
    ) -> Result<ChatHistory, OpenClawClientError> {
        let params = ChatHistoryParams {
            session_key: session_key.to_string(),
            limit,
            include_tools: Some(true),
        };
        let value = self
            .call(methods::CHAT_HISTORY, Some(serde_json::to_value(params)?))
            .await?;
        serde_json::from_value(value).map_err(|e| OpenClawClientError::Other(e.into()))
    }

    async fn chat_abort(&self, session_key: &str) -> Result<(), OpenClawClientError> {
        self.call(
            methods::CHAT_ABORT,
            Some(serde_json::json!({ "sessionKey": session_key })),
        )
        .await?;
        Ok(())
    }
}

/// Extract plain text from a list of chat blocks.
pub fn blocks_to_text(blocks: &[ChatBlock]) -> String {
    blocks
        .iter()
        .filter_map(|b| match b {
            ChatBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openclaw_gateway::client::tests::mock_openclaw_server;

    #[tokio::test]
    async fn chat_send_text_serializes_request() {
        let (addr, mut rx) = mock_openclaw_server().await;
        let url = format!("ws://{}", addr);
        let client = OpenClawGatewayClient::connect(&url, "test-token")
            .await
            .unwrap();

        for _ in 0..50 {
            if client.hello_ok().is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        let _ = client.chat_send_text("agent:main:main", "hello").await;

        let req = rx.recv().await.unwrap();
        let value: serde_json::Value = serde_json::from_str(&req).unwrap();
        assert_eq!(value["method"], "chat.send");
        assert_eq!(value["params"]["sessionKey"], "agent:main:main");
        assert_eq!(value["params"]["message"][0]["type"], "text");
        assert_eq!(value["params"]["message"][0]["text"], "hello");
    }

    #[test]
    fn blocks_to_text_filters_non_text() {
        let blocks = vec![
            ChatBlock::Text { text: "hi ".into() },
            ChatBlock::ResourceLink {
                uri: "kimi-file://x".into(),
                title: None,
                name: None,
                mime_type: None,
            },
            ChatBlock::Text {
                text: "there".into(),
            },
        ];
        assert_eq!(blocks_to_text(&blocks), "hi there");
    }
}
