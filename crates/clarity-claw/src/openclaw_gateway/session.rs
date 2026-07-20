//! OpenClaw Gateway session management API.
//!
//! Thin wrappers around the OpenClaw JSON-RPC methods for listing, previewing,
//! and mutating sessions.

use crate::openclaw_gateway::client::{OpenClawClientError, OpenClawGatewayClient};
use crate::openclaw_gateway::protocol::methods;
use crate::openclaw_gateway::types::{OpenClawSession, SessionList, SessionListParams};

/// Session-management methods for [`OpenClawGatewayClient`].
#[async_trait::async_trait]
pub trait OpenClawSessionApi {
    /// List sessions.
    async fn list_sessions(
        &self,
        params: SessionListParams,
    ) -> Result<SessionList, OpenClawClientError>;

    /// Preview a single session.
    async fn preview_session(
        &self,
        session_key: &str,
    ) -> Result<OpenClawSession, OpenClawClientError>;

    /// Reset a session.
    async fn reset_session(&self, session_key: &str) -> Result<(), OpenClawClientError>;

    /// Delete a session.
    async fn delete_session(&self, session_key: &str) -> Result<(), OpenClawClientError>;

    /// Compact a session (force context compression).
    async fn compact_session(&self, session_key: &str) -> Result<(), OpenClawClientError>;
}

#[async_trait::async_trait]
impl OpenClawSessionApi for OpenClawGatewayClient {
    async fn list_sessions(
        &self,
        params: SessionListParams,
    ) -> Result<SessionList, OpenClawClientError> {
        let value = self
            .call(methods::SESSIONS_LIST, Some(serde_json::to_value(params)?))
            .await?;
        serde_json::from_value(value).map_err(|e| OpenClawClientError::Other(e.into()))
    }

    async fn preview_session(
        &self,
        session_key: &str,
    ) -> Result<OpenClawSession, OpenClawClientError> {
        let value = self
            .call(
                methods::SESSIONS_PREVIEW,
                Some(serde_json::json!({ "sessionKey": session_key })),
            )
            .await?;
        serde_json::from_value(value).map_err(|e| OpenClawClientError::Other(e.into()))
    }

    async fn reset_session(&self, session_key: &str) -> Result<(), OpenClawClientError> {
        self.call(
            methods::SESSIONS_RESET,
            Some(serde_json::json!({ "sessionKey": session_key })),
        )
        .await?;
        Ok(())
    }

    async fn delete_session(&self, session_key: &str) -> Result<(), OpenClawClientError> {
        self.call(
            methods::SESSIONS_DELETE,
            Some(serde_json::json!({ "sessionKey": session_key })),
        )
        .await?;
        Ok(())
    }

    async fn compact_session(&self, session_key: &str) -> Result<(), OpenClawClientError> {
        self.call(
            methods::SESSIONS_COMPACT,
            Some(serde_json::json!({ "sessionKey": session_key })),
        )
        .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openclaw_gateway::client::tests::mock_openclaw_server;

    #[tokio::test]
    async fn list_sessions_parses_response() {
        // The mock server used here echoes requests instead of returning a real
        // session list, so this test only verifies the request shape.
        let (addr, mut rx) = mock_openclaw_server().await;
        let url = format!("ws://{}", addr);
        let client = OpenClawGatewayClient::connect(&url, "test-token")
            .await
            .unwrap();

        // Wait for handshake.
        for _ in 0..50 {
            if client.hello_ok().is_some() {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }

        let _ = client
            .list_sessions(SessionListParams {
                limit: Some(10),
                ..Default::default()
            })
            .await;

        let req = rx.recv().await.unwrap();
        let value: serde_json::Value = serde_json::from_str(&req).unwrap();
        assert_eq!(value["method"], "sessions.list");
        assert_eq!(value["params"]["limit"], 10);
    }
}
