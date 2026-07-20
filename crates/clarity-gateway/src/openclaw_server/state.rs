//! Runtime state for the OpenClaw-compatible endpoint.

use clarity_contract::openclaw_protocol::{
    HelloOk, HelloOkAuth, OpenClawFeatures, OpenClawPolicy, OpenClawServerInfo,
};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Persistent record of an approved paired device.
#[derive(Clone, Debug)]
pub struct ApprovedDevice {
    /// Device id (SHA-256 of public key, hex).
    pub device_id: String,
    /// Base64url-encoded Ed25519 public key.
    pub public_key: String,
    /// Granted scopes.
    pub scopes: Vec<String>,
    /// Token to use on subsequent reconnects.
    pub device_token: String,
    /// Approval timestamp (ms since Unix epoch).
    pub approved_at_ms: u64,
}

/// Shared state for the OpenClaw server.
#[derive(Clone)]
pub struct OpenClawServerState {
    inner: Arc<Mutex<OpenClawServerStateInner>>,
    admin_token_path: PathBuf,
    admin_token: String,
}

struct OpenClawServerStateInner {
    approved_devices: HashMap<String, ApprovedDevice>,
    conn_counter: u64,
}

impl OpenClawServerState {
    /// Load or create the OpenClaw server state.
    ///
    /// `clarity_home` is the Clarity data directory (e.g. `.clarity` under the
    /// current working directory). The admin token is persisted in
    /// `{clarity_home}/openclaw-admin-token`.
    pub fn load_or_create(clarity_home: impl AsRef<Path>) -> Self {
        let clarity_home = clarity_home.as_ref().to_path_buf();
        let _ = std::fs::create_dir_all(&clarity_home);
        let admin_token_path = clarity_home.join("openclaw-admin-token");

        let admin_token = if admin_token_path.exists() {
            std::fs::read_to_string(&admin_token_path)
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|_| {
                    let t = crate::openclaw_server::auth::generate_admin_token();
                    let _ = std::fs::write(&admin_token_path, &t);
                    t
                })
        } else {
            let t = crate::openclaw_server::auth::generate_admin_token();
            let _ = std::fs::write(&admin_token_path, &t);
            t
        };

        Self {
            inner: Arc::new(Mutex::new(OpenClawServerStateInner {
                approved_devices: HashMap::new(),
                conn_counter: 0,
            })),
            admin_token_path,
            admin_token,
        }
    }

    /// Admin token used for CLI/admin connections.
    pub fn admin_token(&self) -> &str {
        &self.admin_token
    }

    /// Path where the admin token is persisted.
    pub fn admin_token_path(&self) -> &Path {
        &self.admin_token_path
    }

    /// Atomically increment and return the next connection id.
    pub fn next_conn_id(&self) -> u64 {
        let mut inner = self.inner.lock();
        inner.conn_counter += 1;
        inner.conn_counter
    }

    /// Approve a new device and return its record.
    pub fn approve_device(
        &self,
        device_id: String,
        public_key: String,
        scopes: Vec<String>,
    ) -> ApprovedDevice {
        let device_token = generate_device_token();
        let approved_at_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let record = ApprovedDevice {
            device_id: device_id.clone(),
            public_key,
            scopes,
            device_token: device_token.clone(),
            approved_at_ms,
        };

        self.inner
            .lock()
            .approved_devices
            .insert(device_id, record.clone());
        record
    }

    /// Look up an approved device by id.
    pub fn get_device(&self, device_id: &str) -> Option<ApprovedDevice> {
        self.inner.lock().approved_devices.get(device_id).cloned()
    }

    /// Return all approved devices.
    pub fn list_devices(&self) -> Vec<ApprovedDevice> {
        self.inner
            .lock()
            .approved_devices
            .values()
            .cloned()
            .collect()
    }

    /// Build the `hello-ok` payload advertised to clients.
    pub fn hello_ok(&self, conn_id: &str, role: &str, scopes: &[String]) -> HelloOk {
        HelloOk {
            kind: "hello-ok".to_string(),
            protocol: 3,
            server: OpenClawServerInfo {
                version: env!("CARGO_PKG_VERSION").to_string(),
                conn_id: conn_id.to_string(),
            },
            features: OpenClawFeatures {
                methods: vec![
                    "chat.send".to_string(),
                    "chat.history".to_string(),
                    "chat.abort".to_string(),
                    "sessions.list".to_string(),
                    "sessions.preview".to_string(),
                    "sessions.reset".to_string(),
                    "sessions.delete".to_string(),
                    "sessions.compact".to_string(),
                    "device.pair.request".to_string(),
                    "device.pair.list".to_string(),
                ],
                events: vec!["chat".to_string()],
            },
            policy: OpenClawPolicy {
                max_payload: 26214400,
                max_buffered_bytes: 52428800,
                tick_interval_ms: 30000,
            },
            auth: Some(HelloOkAuth {
                device_token: String::new(),
                role: role.to_string(),
                scopes: scopes.to_vec(),
                issued_at_ms: None,
            }),
            canvas_host_url: None,
        }
    }
}

fn generate_device_token() -> String {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    format!("dt_{}", hex::encode(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_or_create_generates_and_reloads_admin_token() {
        let dir = tempfile::tempdir().unwrap();
        let state = OpenClawServerState::load_or_create(dir.path());
        let token = state.admin_token().to_string();
        assert!(!token.is_empty());
        assert!(state.admin_token_path().exists());

        // Reloading from the same directory must return the same token.
        let state2 = OpenClawServerState::load_or_create(dir.path());
        assert_eq!(state2.admin_token(), token);
    }

    #[test]
    fn approve_device_returns_record_with_token() {
        let dir = tempfile::tempdir().unwrap();
        let state = OpenClawServerState::load_or_create(dir.path());
        let record = state.approve_device(
            "device-1".to_string(),
            "pk".to_string(),
            vec!["operator.read".to_string()],
        );
        assert_eq!(record.device_id, "device-1");
        assert_eq!(record.public_key, "pk");
        assert_eq!(record.scopes, vec!["operator.read"]);
        assert!(record.device_token.starts_with("dt_"));

        let listed = state.list_devices();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].device_id, "device-1");
    }

    #[test]
    fn hello_ok_advertises_supported_methods() {
        let dir = tempfile::tempdir().unwrap();
        let state = OpenClawServerState::load_or_create(dir.path());
        let hello = state.hello_ok("conn-1", "operator", &["operator.read".to_string()]);
        assert_eq!(hello.kind, "hello-ok");
        assert_eq!(hello.protocol, 3);
        assert!(hello.features.methods.contains(&"chat.send".to_string()));
        assert!(
            hello
                .features
                .methods
                .contains(&"device.pair.request".to_string())
        );
        assert_eq!(hello.auth.as_ref().unwrap().role, "operator");
    }
}
