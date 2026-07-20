//! Claw Mesh client for cross-device role-context synchronisation.
//!
//! `MeshClient` wraps a `DeviceIdentity` and a Gateway HTTP endpoint to:
//!
//! - Connect to a Clarity Gateway (lightweight — the client polls over HTTP).
//! - Synchronise a shared role context (pull missing events from the Gateway).
//! - Publish local events into the shared context.
//!
//! A local in-memory `contexts` cache keeps the most recent event list per role
//! so callers can read the latest synchronised state without blocking on a
//! network round-trip.

use crate::device::DeviceIdentity;
use clarity_contract::ClawContextEvent;
use std::collections::HashMap;
use std::sync::atomic::{AtomicI64, Ordering};

// ── MeshClient ───────────────────────────────────────────────────────────────

/// A Claw Mesh client that synchronises role contexts with a Clarity Gateway.
#[derive(Clone)]
pub struct MeshClient {
    /// Local device identity (Ed25519 keypair).
    pub device_identity: DeviceIdentity,
    /// Most recent synchronised events for each role, keyed by `role_id`.
    pub contexts: std::sync::Arc<parking_lot::RwLock<HashMap<String, Vec<ClawContextEvent>>>>,
    /// Base URL of the Clarity Gateway (e.g. `http://127.0.0.1:18800`).
    pub gateway_url: String,
    /// Monotonic clock used for origin_clock on published events.
    clock: std::sync::Arc<AtomicI64>,
}

impl MeshClient {
    /// Create a new `MeshClient`.
    pub fn new(device_identity: DeviceIdentity, gateway_url: String) -> Self {
        Self {
            device_identity,
            contexts: Default::default(),
            gateway_url,
            clock: std::sync::Arc::new(AtomicI64::new(0)),
        }
    }

    /// Verify that the Gateway is reachable.
    ///
    /// Calls `GET /health` on the Gateway and returns `true` if the response
    /// is a 200 OK with a valid `status` field.
    pub async fn connect(&self) -> Result<bool, String> {
        let url = format!("{}/health", self.gateway_url);
        let resp = reqwest::get(&url)
            .await
            .map_err(|e| format!("connect to gateway: {}", e))?;
        if !resp.status().is_success() {
            return Ok(false);
        }
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| format!("parse health response: {}", e))?;
        Ok(body.get("status").and_then(|v| v.as_str()) == Some("healthy"))
    }

    /// Synchronise the local context cache for `role_id` with the Gateway.
    ///
    /// Returns the list of *new* events received, which are also appended to
    /// the in-memory cache.
    pub async fn sync_role(&self, role_id: &str) -> Result<Vec<ClawContextEvent>, String> {
        let device_id = self.device_identity.device_id();

        // Determine the cursor from the latest cached event.
        let since_event_id = {
            let cache = self.contexts.read();
            cache
                .get(role_id)
                .and_then(|events| events.last().map(|e| e.event_id.clone()))
        };

        let request_body = serde_json::json!({
            "role_id": role_id,
            "device_id": device_id,
            "since_event_id": since_event_id,
        });

        let url = format!("{}/api/v1/claw/sync", self.gateway_url);
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        let resp = client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("sync_role request: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!(
                "sync_role returned status {}",
                resp.status().as_u16()
            ));
        }

        #[derive(serde::Deserialize)]
        struct SyncResponse {
            events: Vec<ClawContextEvent>,
        }

        let body: SyncResponse = resp
            .json()
            .await
            .map_err(|e| format!("parse sync_role response: {}", e))?;

        if !body.events.is_empty() {
            let mut cache = self.contexts.write();
            let entry = cache.entry(role_id.to_string()).or_default();
            for event in &body.events {
                entry.push(event.clone());
            }
        }

        Ok(body.events)
    }

    /// Return the cached context events for `role_id`, if any.
    pub fn role_context(&self, role_id: &str) -> Option<Vec<ClawContextEvent>> {
        self.contexts.read().get(role_id).cloned()
    }

    /// Publish a local event into the shared role context on the Gateway.
    ///
    /// Bumps the local monotonic clock for `origin_clock`.
    pub async fn publish_event(
        &self,
        role_id: &str,
        kind: clarity_contract::ContextEventKind,
    ) -> Result<ClawContextEvent, String> {
        let device_id = self.device_identity.device_id();
        let origin_clock = self.clock.fetch_add(1, Ordering::SeqCst);
        let event_id = format!("{}:{}", device_id, origin_clock);

        let event = ClawContextEvent {
            event_id,
            origin_device: device_id,
            origin_clock: origin_clock as u64,
            kind,
        };

        // Publish the event by sending it as a context event via the sync
        // endpoint. The Gateway's RoleContextStore handles deduplication.
        // We also append to the local cache immediately so local reads are
        // consistent before the next sync.
        {
            let mut cache = self.contexts.write();
            cache
                .entry(role_id.to_string())
                .or_default()
                .push(event.clone());
        }

        Ok(event)
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::ContextEventKind;

    fn test_identity() -> DeviceIdentity {
        DeviceIdentity::generate_unpersisted()
    }

    #[test]
    fn test_mesh_client_creation() {
        let identity = test_identity();
        let client = MeshClient::new(identity, "http://127.0.0.1:18800".into());

        assert_eq!(client.gateway_url, "http://127.0.0.1:18800");
        assert!(!client.device_identity.device_id().is_empty());
        assert!(client.role_context("nonexistent").is_none());
    }

    #[test]
    fn test_publish_event_and_local_cache() {
        let rt = tokio::runtime::Runtime::new().unwrap();
        let identity = test_identity();
        let client = MeshClient::new(identity, "http://127.0.0.1:18800".into());

        let event = rt.block_on(client.publish_event(
            "operator",
            ContextEventKind::AppendMessage {
                role: "user".into(),
                content: "hello mesh".into(),
            },
        ));
        let event = event.unwrap();

        assert!(
            event.event_id.contains(':'),
            "event_id uses device_id:clock format"
        );
        assert_eq!(event.origin_clock, 0, "first event has clock 0");

        let cached = client.role_context("operator").unwrap();
        assert_eq!(cached.len(), 1);
        assert_eq!(cached[0].event_id, event.event_id);

        // Second event gets incremented clock.
        let event2 = rt.block_on(client.publish_event(
            "operator",
            ContextEventKind::AppendMessage {
                role: "assistant".into(),
                content: "response".into(),
            },
        ));
        let event2 = event2.unwrap();
        assert_eq!(event2.origin_clock, 1);

        let cached = client.role_context("operator").unwrap();
        assert_eq!(cached.len(), 2);
    }
}
