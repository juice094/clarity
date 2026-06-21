//! Transport abstraction for Claw Mesh role-context synchronization.
//!
//! A transport is responsible for moving `ClawContextEvent`s between a local
//! device and remote peers (Gateway or other Claw devices). The merger is
//! transport-agnostic and lives in [`super::merger`].

use clarity_contract::{ClawContextEvent, RoleContextId};
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur when publishing or collecting events.
#[derive(Debug, Error, Clone)]
pub enum MeshTransportError {
    /// The transport is not connected.
    #[error("mesh transport not connected")]
    NotConnected,
    /// Serialization or deserialization failed.
    #[error("mesh transport serialization error: {0}")]
    Serialization(String),
    /// A remote peer returned an error.
    #[error("mesh transport remote error: {0}")]
    Remote(String),
    /// A generic transport error.
    #[error("mesh transport error: {0}")]
    Other(String),
    /// A cryptographic operation failed (e.g. wrong passphrase or tampered data).
    #[error("mesh transport crypto error: {0}")]
    Crypto(String),
    /// The operation is not supported by this transport.
    #[error("mesh transport unsupported: {0}")]
    Unsupported(String),
}

/// Result type alias for transport operations.
pub type Result<T> = std::result::Result<T, MeshTransportError>;

/// Trait implemented by both Gateway-sync and syncthing-rust transports.
///
/// Implementations must be `Send + Sync` so the merger can run them on a
/// background task.
#[async_trait::async_trait]
pub trait RoleContextTransport: Send + Sync {
    /// Publish a local event to the transport.
    ///
    /// For Gateway this queues a message; for syncthing-rust this writes a file
    /// to the role's folder.
    async fn publish(&self, role_id: &RoleContextId, event: &ClawContextEvent) -> Result<()>;

    /// Collect all events currently available for a role.
    ///
    /// This may read local files, call a REST API, or drain a WebSocket
    /// channel. Events are returned in no particular order; the caller merges
    /// them with `merge_events`/`merge_into`.
    async fn collect(&self, role_id: &RoleContextId) -> Result<Vec<ClawContextEvent>>;

    /// Subscribe to transport-specific change notifications.
    ///
    /// The returned stream yields role ids that may have new events. Callers
    /// should then `collect` and merge.
    fn subscribe(&self) -> tokio::sync::mpsc::UnboundedReceiver<RoleContextId>;
}

/// A no-op transport used when mesh synchronization is disabled.
pub struct NullTransport {
    _rx: tokio::sync::mpsc::UnboundedReceiver<RoleContextId>,
}

impl NullTransport {
    /// Create a new null transport.
    pub fn new() -> (Self, tokio::sync::mpsc::UnboundedSender<RoleContextId>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        (Self { _rx: rx }, tx)
    }
}

#[async_trait::async_trait]
impl RoleContextTransport for NullTransport {
    async fn publish(&self, _role_id: &RoleContextId, _event: &ClawContextEvent) -> Result<()> {
        Ok(())
    }

    async fn collect(&self, _role_id: &RoleContextId) -> Result<Vec<ClawContextEvent>> {
        Ok(Vec::new())
    }

    fn subscribe(&self) -> tokio::sync::mpsc::UnboundedReceiver<RoleContextId> {
        // The null transport never emits notifications. Returning a closed
        // channel is safe: receivers will get None and can terminate.
        let (_, rx) = tokio::sync::mpsc::unbounded_channel();
        rx
    }
}

impl Default for NullTransport {
    fn default() -> Self {
        let (this, _) = Self::new();
        this
    }
}

/// Type-erased handle to a transport.
pub type TransportHandle = Arc<dyn RoleContextTransport>;
