//! OpenClaw Gateway device pairing API.
//!
//! Thin wrappers around `device.pair.request` and `device.pair.list`, plus the
//! response type used to persist a paired-device token.

use crate::device::DeviceIdentity;
use crate::openclaw_gateway::client::{OpenClawClientError, OpenClawGatewayClient};
use crate::openclaw_gateway::protocol::methods;
use serde::{Deserialize, Serialize};

/// Result of a `device.pair.request` call.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PairRequestResult {
    /// Device id that was registered.
    #[serde(alias = "deviceId", alias = "device_id")]
    pub device_id: String,
    /// Whether the pairing was immediately approved.
    pub approved: bool,
    /// Device token returned on approval.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Granted scopes.
    #[serde(default)]
    pub scopes: Vec<String>,
}

/// Device-management methods for [`OpenClawGatewayClient`].
#[async_trait::async_trait]
pub trait OpenClawDeviceApi {
    /// Request pairing for the given device identity.
    ///
    /// The caller should connect with an admin-capable token (or a token that
    /// already has `operator.pairing` scope) before calling this method.
    async fn pair_request(
        &self,
        device: &DeviceIdentity,
    ) -> Result<PairRequestResult, OpenClawClientError>;

    /// List pending/approved paired devices.
    async fn pair_list(&self) -> Result<Vec<PairRequestResult>, OpenClawClientError>;
}

#[async_trait::async_trait]
impl OpenClawDeviceApi for OpenClawGatewayClient {
    async fn pair_request(
        &self,
        device: &DeviceIdentity,
    ) -> Result<PairRequestResult, OpenClawClientError> {
        let params = serde_json::json!({
            "deviceId": device.device_id(),
            "publicKey": device.public_key(),
            "clientId": "gateway-client",
            "clientMode": "backend",
            "platform": platform_string(),
            "role": "operator",
            "scopes": [
                "operator.admin",
                "operator.read",
                "operator.write",
                "operator.approvals",
                "operator.pairing",
                "operator.talk.secrets"
            ],
        });
        let value = self
            .call(methods::DEVICE_PAIR_REQUEST, Some(params))
            .await?;
        serde_json::from_value(value).map_err(|e| OpenClawClientError::Other(e.into()))
    }

    async fn pair_list(&self) -> Result<Vec<PairRequestResult>, OpenClawClientError> {
        let value = self.call(methods::DEVICE_PAIR_LIST, None).await?;
        serde_json::from_value(value).map_err(|e| OpenClawClientError::Other(e.into()))
    }
}

fn platform_string() -> String {
    #[cfg(target_os = "windows")]
    {
        "win32".to_string()
    }
    #[cfg(target_os = "macos")]
    {
        "darwin".to_string()
    }
    #[cfg(target_os = "linux")]
    {
        "linux".to_string()
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        "unknown".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openclaw_gateway::client::tests::mock_openclaw_server;

    #[test]
    fn pair_request_result_deserializes_all_aliases() {
        let json = serde_json::json!({
            "deviceId": "did-1",
            "approved": true,
            "token": "tok-1",
            "scopes": ["operator.admin", "operator.read"]
        });
        let result: PairRequestResult = serde_json::from_value(json).unwrap();
        assert_eq!(result.device_id, "did-1");
        assert!(result.approved);
        assert_eq!(result.token, Some("tok-1".to_string()));
        assert_eq!(result.scopes, vec!["operator.admin", "operator.read"]);

        let json_snake = serde_json::json!({
            "device_id": "did-2",
            "approved": false,
            "scopes": []
        });
        let result: PairRequestResult = serde_json::from_value(json_snake).unwrap();
        assert_eq!(result.device_id, "did-2");
        assert!(!result.approved);
        assert!(result.token.is_none());
        assert!(result.scopes.is_empty());
    }

    #[tokio::test]
    async fn pair_request_serializes_request() {
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

        let device = DeviceIdentity::generate_unpersisted();
        let _ = client.pair_request(&device).await;

        let req = rx.recv().await.unwrap();
        let value: serde_json::Value = serde_json::from_str(&req).unwrap();
        assert_eq!(value["method"], "device.pair.request");
        assert_eq!(value["params"]["deviceId"], device.device_id());
        assert_eq!(value["params"]["publicKey"], device.public_key());
        assert_eq!(value["params"]["clientId"], "gateway-client");
        assert_eq!(value["params"]["role"], "operator");
    }
}
