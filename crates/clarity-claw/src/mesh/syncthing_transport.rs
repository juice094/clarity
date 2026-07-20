//! Syncthing-rust transport for Claw Mesh role contexts.
//!
//! Each role maps to a Syncthing folder (`claw_role_{role_id}`) rooted at
//! `{base_dir}/{role_id}`. Events are stored as individual JSON files under
//! `events/`, allowing the Syncthing scanner to index and synchronize them with
//! configured peers.
//!
//! ponytail: this is a file-based integration. Network peer discovery and
//! automatic connection setup are left to future work; peers must currently be
//! added with [`SyncthingTransport::add_peer`] after the transport is created,
//! or via the underlying `SyncService` config.

use super::{
    crypto,
    transport::{MeshTransportError, Result, RoleContextTransport},
};
use bytes::Bytes;
use clarity_contract::{ClawContextEvent, RoleContextId};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use syncthing_core::types::{Config, Folder, FolderType};
use syncthing_sync::{
    database::FileSystemDatabase, events::SyncEvent, model::SyncManager, puller::BlockSource,
    service::SyncService,
};

/// File-based transport backed by syncthing-rust.
///
/// The transport owns a `SyncService` that indexes the role event files and
/// (once peers are configured) synchronizes them over the Syncthing BEP
/// protocol. Local reads and writes always go through the filesystem, so the
/// transport keeps working even when no peers are connected.
pub struct SyncthingTransport {
    service: Arc<SyncService>,
    base_dir: PathBuf,
    notify_tx: tokio::sync::mpsc::UnboundedSender<RoleContextId>,
    passphrases: Arc<tokio::sync::RwLock<HashMap<RoleContextId, String>>>,
}

impl SyncthingTransport {
    /// Create and start a new syncthing-rust backed transport.
    ///
    /// `base_dir` is the root directory for all mesh data (indexes under
    /// `.stindex`, role folders under `{base_dir}/{role_id}/`). `device_name`
    /// is used as the local Syncthing device label.
    ///
    /// The returned receiver yields role ids when the transport observes local
    /// or remote index changes for that role.
    pub async fn new(
        base_dir: impl Into<PathBuf>,
        device_name: impl Into<String>,
    ) -> Result<(Self, tokio::sync::mpsc::UnboundedReceiver<RoleContextId>)> {
        let base_dir = base_dir.into();
        let device_name = device_name.into();

        std::fs::create_dir_all(&base_dir).map_err(|e| {
            MeshTransportError::Other(format!("create mesh base dir {}: {e}", base_dir.display()))
        })?;

        let index_dir = base_dir.join(".stindex");
        std::fs::create_dir_all(&index_dir).map_err(|e| {
            MeshTransportError::Other(format!(
                "create mesh index dir {}: {e}",
                index_dir.display()
            ))
        })?;

        let db = FileSystemDatabase::new(&index_dir);
        let service = Arc::new(SyncService::new(db));

        let mut config = Config::new();
        config.device_name = device_name;
        service
            .update_config(config)
            .await
            .map_err(map_sync_error)?;

        let block_source = Arc::new(LocalBlockSource {
            base_dir: base_dir.clone(),
        });
        service.set_block_source(block_source).await;

        service.start().await.map_err(map_sync_error)?;

        let (notify_tx, notify_rx) = tokio::sync::mpsc::unbounded_channel();
        let transport = Self {
            service,
            base_dir,
            notify_tx,
            passphrases: Arc::new(tokio::sync::RwLock::new(HashMap::new())),
        };
        transport.spawn_event_loop();

        Ok((transport, notify_rx))
    }

    /// Add a remote peer device that is allowed to sync Claw role folders.
    ///
    /// ponytail: peers must be pre-shared out-of-band. Auto-discovery is not
    /// implemented yet.
    pub async fn add_peer(
        &self,
        device_id: syncthing_core::DeviceId,
        name: Option<String>,
    ) -> Result<()> {
        let device = syncthing_core::types::Device {
            id: device_id,
            name,
            addresses: Vec::new(),
            paused: false,
            introducer: false,
        };
        self.service
            .add_device(device)
            .await
            .map_err(map_sync_error)
    }

    fn folder_id(role_id: &RoleContextId) -> String {
        role_id.syncthing_folder_id()
    }

    fn role_folder_path(&self, role_id: &RoleContextId) -> PathBuf {
        // ponytail: assumes role_id is filesystem-safe (e.g. "operator"). Sanitize
        // if user-supplied role ids are allowed to contain path separators.
        self.base_dir.join(role_id.as_ref())
    }

    fn event_path(&self, role_id: &RoleContextId, event_id: &str, encrypted: bool) -> PathBuf {
        let ext = if encrypted { "enc" } else { "json" };
        self.role_folder_path(role_id)
            .join("events")
            .join(format!("{event_id}.{ext}"))
    }

    async fn passphrase_for(&self, role_id: &RoleContextId) -> Option<String> {
        self.passphrases.read().await.get(role_id).cloned()
    }

    /// Set or clear the passphrase used to encrypt events for `role_id`.
    ///
    /// An empty passphrase removes any existing encryption key for the role;
    /// subsequent `publish` calls will write plaintext `.json` files. Existing
    /// encrypted `.enc` files remain on disk and can still be collected if the
    /// passphrase is re-supplied later.
    pub async fn set_role_passphrase(&self, role_id: RoleContextId, passphrase: String) {
        let mut passphrases = self.passphrases.write().await;
        if passphrase.is_empty() {
            passphrases.remove(&role_id);
        } else {
            passphrases.insert(role_id, passphrase);
        }
    }

    /// Ensure the Syncthing folder for `role_id` exists in the service.
    ///
    /// ponytail: checks the full config each call. If this becomes hot, cache
    /// known folder ids in a `DashSet`.
    async fn ensure_folder(&self, role_id: &RoleContextId) -> Result<()> {
        let folder_id = Self::folder_id(role_id);
        let config = self.service.get_config().await.map_err(map_sync_error)?;
        if config.folders.iter().any(|f| f.id == folder_id) {
            return Ok(());
        }

        let path = self.role_folder_path(role_id);
        tokio::fs::create_dir_all(&path).await.map_err(|e| {
            MeshTransportError::Other(format!("create role folder {}: {e}", path.display()))
        })?;

        let mut folder = Folder::new(&folder_id, path.to_string_lossy());
        folder.folder_type = FolderType::SendReceive;
        folder.rescan_interval_secs = 300;

        self.service
            .add_folder(folder)
            .await
            .map_err(map_sync_error)
    }

    fn spawn_event_loop(&self) {
        let service = Arc::clone(&self.service);
        let notify_tx = self.notify_tx.clone();
        tokio::spawn(async move {
            let mut subscriber = service.subscribe_events();
            while let Some(event) = subscriber.recv().await {
                match event {
                    SyncEvent::LocalIndexUpdated { folder, .. }
                    | SyncEvent::RemoteIndexReceived { folder, .. }
                    | SyncEvent::FolderScanCompleted { folder, .. } => {
                        if let Some(role) = folder.strip_prefix("claw_role_") {
                            let _ = notify_tx.send(RoleContextId::new(role));
                        }
                    }
                    _ => {}
                }
            }
        });
    }
}

#[async_trait::async_trait]
impl RoleContextTransport for SyncthingTransport {
    async fn publish(&self, role_id: &RoleContextId, event: &ClawContextEvent) -> Result<()> {
        self.ensure_folder(role_id).await?;

        let passphrase = self.passphrase_for(role_id).await;
        let path = self.event_path(role_id, &event.event_id, passphrase.is_some());
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                MeshTransportError::Other(format!("create events dir {}: {e}", parent.display()))
            })?;
        }

        let json = serde_json::to_string_pretty(event)
            .map_err(|e| MeshTransportError::Serialization(e.to_string()))?;
        let content = if let Some(passphrase) = passphrase {
            crypto::encrypt(role_id, &passphrase, &json)?
        } else {
            json
        };
        tokio::fs::write(&path, content).await.map_err(|e| {
            MeshTransportError::Other(format!("write event file {}: {e}", path.display()))
        })?;

        let folder_id = Self::folder_id(role_id);
        self.service
            .scan_folder_sub(&folder_id, "events")
            .await
            .map_err(map_sync_error)
    }

    async fn collect(&self, role_id: &RoleContextId) -> Result<Vec<ClawContextEvent>> {
        self.ensure_folder(role_id).await?;

        let events_dir = self.role_folder_path(role_id).join("events");
        if !events_dir.exists() {
            return Ok(Vec::new());
        }

        let mut entries = tokio::fs::read_dir(&events_dir).await.map_err(|e| {
            MeshTransportError::Other(format!("read events dir {}: {e}", events_dir.display()))
        })?;

        let passphrase = self.passphrase_for(role_id).await;
        let mut events = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            MeshTransportError::Other(format!(
                "read events dir entry {}: {e}",
                events_dir.display()
            ))
        })? {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str());
            let is_encrypted = ext == Some("enc");
            if ext != Some("json") && !is_encrypted {
                continue;
            }
            let content = match tokio::fs::read_to_string(&path).await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skipping unreadable mesh event file");
                    continue;
                }
            };

            let json = if is_encrypted {
                match passphrase {
                    Some(ref pw) => match crypto::decrypt(role_id, pw, &content) {
                        Ok(j) => j,
                        Err(e) => {
                            tracing::warn!(path = %path.display(), error = %e, "skipping encrypted mesh event file (wrong passphrase?)");
                            continue;
                        }
                    },
                    None => {
                        tracing::warn!(path = %path.display(), "skipping encrypted mesh event file (no passphrase set)");
                        continue;
                    }
                }
            } else {
                content
            };

            match serde_json::from_str::<ClawContextEvent>(&json) {
                Ok(event) => events.push(event),
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skipping malformed mesh event file");
                }
            }
        }

        Ok(events)
    }

    fn subscribe(&self) -> tokio::sync::mpsc::UnboundedReceiver<RoleContextId> {
        // Notifications are produced by the event loop via notify_tx; the
        // receiver is owned by the caller. Returning a closed channel here is a
        // safe fallback because callers should use the receiver returned from
        // `new()`.
        let (_, rx) = tokio::sync::mpsc::unbounded_channel();
        rx
    }
}

/// Read file blocks from the local role folder.
///
/// This provides the `SyncService` with data for remote peers during pull
/// operations. Paths are reconstructed from the folder id (`claw_role_{role}`)
/// and the file name stored in the Syncthing index.
struct LocalBlockSource {
    base_dir: PathBuf,
}

#[async_trait::async_trait]
impl BlockSource for LocalBlockSource {
    async fn request_block(
        &self,
        folder: &str,
        file: &str,
        block: &syncthing_core::types::BlockInfo,
        _block_no: usize,
    ) -> syncthing_sync::Result<Bytes> {
        let role = folder.strip_prefix("claw_role_").ok_or_else(|| {
            syncthing_sync::SyncError::Other(format!("invalid claw folder id: {folder}"))
        })?;
        let path = self.base_dir.join(role).join(file);
        let data = tokio::fs::read(&path).await?;
        let start = block.offset as usize;
        let end = (start + block.size as usize).min(data.len());
        if start > data.len() {
            return Err(syncthing_sync::SyncError::Other(format!(
                "block offset {} exceeds file size {} for {}",
                block.offset,
                data.len(),
                path.display()
            )));
        }
        Ok(Bytes::copy_from_slice(&data[start..end]))
    }
}

fn map_sync_error(e: syncthing_sync::SyncError) -> MeshTransportError {
    MeshTransportError::Other(e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::ContextEventKind;

    fn temp_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "clarity-claw-syncthing-test-{}-{}",
            name,
            std::process::id()
        ))
    }

    fn sample_event(event_id: &str) -> ClawContextEvent {
        ClawContextEvent {
            event_id: event_id.to_string(),
            origin_device: "device-a".to_string(),
            origin_clock: 1,
            kind: ContextEventKind::AppendMessage {
                role: "user".to_string(),
                content: "hello".to_string(),
            },
        }
    }

    #[tokio::test]
    async fn test_publish_and_collect_roundtrip() {
        let base = temp_dir("roundtrip");
        let _ = std::fs::remove_dir_all(&base);
        let (transport, _rx) = SyncthingTransport::new(&base, "test-device")
            .await
            .expect("transport starts");

        let role = RoleContextId::new("test-role");
        let event = sample_event("evt-1");
        transport.publish(&role, &event).await.expect("publish");

        let collected = transport.collect(&role).await.expect("collect");
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].event_id, "evt-1");

        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn test_encrypted_publish_and_collect_roundtrip() {
        let base = temp_dir("encrypted-roundtrip");
        let _ = std::fs::remove_dir_all(&base);
        let (transport, _rx) = SyncthingTransport::new(&base, "test-device")
            .await
            .expect("transport starts");

        let role = RoleContextId::new("encrypted-role");
        transport
            .set_role_passphrase(role.clone(), "mesh-secret".to_string())
            .await;

        let event = sample_event("evt-enc-1");
        transport
            .publish(&role, &event)
            .await
            .expect("publish encrypted");

        let collected = transport.collect(&role).await.expect("collect encrypted");
        assert_eq!(collected.len(), 1);
        assert_eq!(collected[0].event_id, "evt-enc-1");

        // The file on disk should be encrypted (enc3: prefix).
        let file_path = base
            .join("encrypted-role")
            .join("events")
            .join("evt-enc-1.enc");
        let on_disk = std::fs::read_to_string(&file_path).expect("read encrypted file");
        assert!(crypto::is_encrypted(&on_disk));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn test_encrypted_file_skipped_without_passphrase() {
        let base = temp_dir("encrypted-no-pass");
        let _ = std::fs::remove_dir_all(&base);
        let (transport, _rx) = SyncthingTransport::new(&base, "test-device")
            .await
            .expect("transport starts");

        let role = RoleContextId::new("skip-role");
        transport
            .set_role_passphrase(role.clone(), "mesh-secret".to_string())
            .await;
        let event = sample_event("evt-skip");
        transport
            .publish(&role, &event)
            .await
            .expect("publish encrypted");

        // Remove the passphrase and try to collect.
        transport
            .set_role_passphrase(role.clone(), String::new())
            .await;
        let collected = transport.collect(&role).await.expect("collect");
        assert!(collected.is_empty());

        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn test_mixed_plaintext_and_encrypted_events() {
        let base = temp_dir("mixed-events");
        let _ = std::fs::remove_dir_all(&base);
        let (transport, _rx) = SyncthingTransport::new(&base, "test-device")
            .await
            .expect("transport starts");

        let role = RoleContextId::new("mixed-role");
        let plain_event = sample_event("evt-plain");
        transport
            .publish(&role, &plain_event)
            .await
            .expect("publish plain");

        transport
            .set_role_passphrase(role.clone(), "mixed-secret".to_string())
            .await;
        let enc_event = sample_event("evt-enc");
        transport
            .publish(&role, &enc_event)
            .await
            .expect("publish encrypted");

        let collected = transport.collect(&role).await.expect("collect mixed");
        assert_eq!(collected.len(), 2);
        let ids: Vec<_> = collected.iter().map(|e| e.event_id.clone()).collect();
        assert!(ids.contains(&"evt-plain".to_string()));
        assert!(ids.contains(&"evt-enc".to_string()));

        let _ = std::fs::remove_dir_all(&base);
    }

    #[tokio::test]
    async fn test_block_source_reads_event_file() {
        let base = temp_dir("blocksource");
        let _ = std::fs::remove_dir_all(&base);
        std::fs::create_dir_all(base.join("role-x").join("events")).unwrap();
        let file_path = base.join("role-x").join("events").join("evt.json");
        std::fs::write(&file_path, b"hello mesh").unwrap();

        let source = LocalBlockSource {
            base_dir: base.clone(),
        };
        let block = syncthing_core::types::BlockInfo {
            size: 10,
            offset: 0,
            hash: vec![],
        };
        let bytes = source
            .request_block("claw_role_role-x", "events/evt.json", &block, 0)
            .await
            .expect("block read");
        assert_eq!(bytes, Bytes::from_static(b"hello mesh"));

        let _ = std::fs::remove_dir_all(&base);
    }
}
