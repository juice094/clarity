//! Transport-agnostic Claw protocol abstraction.
//!
//! `ClawTransport` is the single contract surface used by all Clarity entry
//! points (egui, tui, claw, headless, mobile-core) to talk to a remote or
//! embedded Agent runtime. Concrete adapters implement this trait for the
//! native Gateway WebSocket and the OpenClaw JSON-RPC dialect.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::{ClawContextEvent, Message, retry::ConnectionMetrics};

/// Errors that can occur when driving a `ClawTransport`.
#[derive(Debug, Error, Clone, Serialize, Deserialize)]
pub enum TransportError {
    /// The transport is not connected.
    #[error("not connected")]
    NotConnected,
    /// Authentication or authorization failed.
    #[error("auth failed: {reason}")]
    AuthFailed {
        /// Human-readable reason.
        reason: String,
    },
    /// The remote endpoint returned an error.
    #[error("remote error: {code} - {message}")]
    Remote {
        /// Machine-readable error code.
        code: String,
        /// Human-readable message.
        message: String,
        /// Whether the caller should retry.
        #[serde(default)]
        retryable: bool,
    },
    /// Serialization failed.
    #[error("serialization failed: {0}")]
    Serialization(String),
    /// The requested capability is not supported by this transport.
    #[error("unsupported capability: {0}")]
    Unsupported(String),
    /// A generic transport-level failure.
    #[error("transport error: {0}")]
    Other(String),
}

impl TransportError {
    /// Convenience constructor for a generic error message.
    pub fn other(msg: impl Into<String>) -> Self {
        Self::Other(msg.into())
    }
}

/// Authentication material presented during handshake.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransportAuth {
    /// Admin or gateway token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    /// Bootstrap token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bootstrap_token: Option<String>,
    /// Device token obtained after pairing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_token: Option<String>,
    /// Password (when gateway uses password auth).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

/// Capabilities advertised by a transport after handshake.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TransportCaps {
    /// Transport-level methods supported by the peer, e.g. `chat.send`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub methods: Vec<String>,
    /// Unsolicited events supported by the peer, e.g. `chat`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub events: Vec<String>,
    /// Maximum payload size in bytes.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_payload: Option<usize>,
    /// Negotiated protocol version, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_version: Option<u8>,
    /// Extra transport-specific capabilities.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub extras: HashMap<String, serde_json::Value>,
}

impl TransportCaps {
    /// Check whether a method is supported.
    pub fn supports_method(&self, method: &str) -> bool {
        self.methods.iter().any(|m| m == method)
    }

    /// Check whether an event is supported.
    pub fn supports_event(&self, event: &str) -> bool {
        self.events.iter().any(|e| e == event)
    }
}

/// A single chat history entry returned by `ClawTransport::get_history`.
pub type HistoryMessage = Message;

/// Context for sending a user message through a transport.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MessageContext {
    /// Optional session key. `None` means the transport's default session.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_key: Option<String>,
    /// User message text.
    pub message: String,
    /// Optional free-form metadata (e.g. locale, mode).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

/// Events emitted by a transport's event stream.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TransportEvent {
    /// Transport connected and handshake completed.
    Connected {
        /// URL of the gateway or runtime endpoint.
        gateway_url: String,
        /// Session id assigned by the server, if any.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session_id: Option<String>,
    },
    /// A text chunk from an in-flight chat response.
    ChatChunk {
        /// Text delta.
        content: String,
    },
    /// A reasoning/thinking chunk from an in-flight chat response.
    ReasoningChunk {
        /// Reasoning text delta.
        content: String,
    },
    /// The current chat turn finished.
    Done,
    /// Chat history response.
    History {
        /// Ordered messages.
        messages: Vec<HistoryMessage>,
    },
    /// A device pairing event.
    DevicePaired {
        /// Device id.
        device_id: String,
        /// Whether the pairing request was approved.
        approved: bool,
        /// Device token for subsequent reconnects.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        token: Option<String>,
        /// Granted scopes.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        scopes: Vec<String>,
    },
    /// The transport is reconnecting after a failure.
    Reconnecting {
        /// Reason for the reconnect.
        reason: String,
        /// Seconds until the next attempt.
        seconds: u64,
    },
    /// Role context synchronization event.
    RoleContextSynced {
        /// Role identifier.
        role_id: String,
        /// New or updated events.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        events: Vec<ClawContextEvent>,
        /// Pagination cursor for the next sync request.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        next_cursor: Option<String>,
        /// Devices currently online for this role.
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        online_devices: Vec<String>,
    },
    /// Transport-level error.
    Error {
        /// Error message.
        message: String,
    },
    /// The peer explicitly closed the connection.
    Closed {
        /// Reason, if provided.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
    /// Raw wire payload for transports that stream native wire messages.
    WirePayload {
        /// Raw wire payload.
        payload: serde_json::Value,
    },
    /// A capability was requested that the peer does not support.
    Unsupported {
        /// Reason.
        reason: String,
    },
}

/// A unified transport adapter for Claw protocols.
///
/// Implementations must be `Send + Sync` so that a `Box<dyn ClawTransport>`
/// can be held by the connection manager and shared across async tasks.
#[async_trait]
#[allow(clippy::too_many_arguments)]
pub trait ClawTransport: Send + Sync {
    /// Perform the transport-specific handshake and return negotiated caps.
    async fn handshake(&self, auth: TransportAuth) -> Result<TransportCaps, TransportError>;

    /// Send a user message. The implementation is responsible for streaming
    /// response events through `Self::events`.
    async fn send_message(&self, ctx: MessageContext) -> Result<(), TransportError>;

    /// Fetch chat history for the given session key (or default session).
    async fn get_history(
        &self,
        session_key: Option<String>,
    ) -> Result<Vec<HistoryMessage>, TransportError>;

    /// Request a sync of role context events.
    async fn sync_role_context(
        &self,
        role_id: String,
        since_event_id: Option<String>,
    ) -> Result<(), TransportError>;

    /// Abort the current in-flight chat turn, if any.
    async fn abort(&self) -> Result<(), TransportError>;

    /// Request device pairing with the remote gateway.
    ///
    /// Only transports that support device-scoped authentication (e.g. OpenClaw)
    /// implement this; the default returns `Unsupported`.
    #[allow(unused_variables)]
    async fn request_pairing(
        &self,
        device_id: String,
        public_key: String,
        client_id: String,
        client_mode: String,
        platform: String,
        role: String,
        scopes: Vec<String>,
    ) -> Result<(), TransportError> {
        Err(TransportError::Unsupported(
            "device pairing not supported by this transport".into(),
        ))
    }

    /// Return a stream of transport events. The stream must be multi-consumer
    /// safe at the transport level; callers that need broadcasting should
    /// fan-out themselves.
    fn events(&self) -> BoxStream<'static, TransportEvent>;

    /// Return the capabilities advertised by the peer.
    ///
    /// For transports that negotiate caps during `handshake`, this returns the
    /// negotiated value; otherwise it returns the static/local caps.
    fn capabilities(&self) -> TransportCaps;
}

/// Governance wrapper around any `ClawTransport`.
///
/// `GovernedTransport` enforces authentication, records connection metrics, and
/// emits audit log events for every method call. It is transport-agnostic and
/// can wrap both client-side adapters and server-side acceptors.
#[derive(Clone)]
pub struct GovernedTransport<T: ClawTransport> {
    inner: Arc<T>,
    auth: TransportAuth,
    metrics: Arc<ConnectionMetrics>,
}

impl<T: ClawTransport> GovernedTransport<T> {
    /// Wrap a transport with governance.
    ///
    /// `auth` is the credential set that will be validated at handshake time
    /// and passed through to the inner transport.
    pub fn new(inner: T, auth: TransportAuth) -> Self {
        Self {
            inner: Arc::new(inner),
            auth,
            metrics: Arc::new(ConnectionMetrics::default()),
        }
    }

    /// Wrap a transport with governance and an explicit metrics handle.
    pub fn with_metrics(inner: T, auth: TransportAuth, metrics: Arc<ConnectionMetrics>) -> Self {
        Self {
            inner: Arc::new(inner),
            auth,
            metrics,
        }
    }

    /// Return the shared metrics handle.
    pub fn metrics(&self) -> Arc<ConnectionMetrics> {
        self.metrics.clone()
    }

    fn require_auth(&self) -> Result<(), TransportError> {
        if self.auth.token.is_none()
            && self.auth.bootstrap_token.is_none()
            && self.auth.device_token.is_none()
            && self.auth.password.is_none()
        {
            return Err(TransportError::AuthFailed {
                reason: "no credentials provided".into(),
            });
        }
        Ok(())
    }

    fn audit(&self, action: &str, result: &str) {
        tracing::info!(
            has_token = self.auth.token.is_some(),
            has_bootstrap = self.auth.bootstrap_token.is_some(),
            has_device = self.auth.device_token.is_some(),
            has_password = self.auth.password.is_some(),
            action = action,
            result = result,
            "ClawTransport audit"
        );
    }
}

#[async_trait]
impl<T: ClawTransport> ClawTransport for GovernedTransport<T> {
    async fn handshake(&self, _auth: TransportAuth) -> Result<TransportCaps, TransportError> {
        self.require_auth()?;
        let result = self.inner.handshake(self.auth.clone()).await;
        self.audit("handshake", if result.is_ok() { "ok" } else { "err" });
        if result.is_ok() {
            self.metrics
                .successful_probes
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        } else {
            self.metrics
                .errors
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        result
    }

    async fn send_message(&self, ctx: MessageContext) -> Result<(), TransportError> {
        self.require_auth()?;
        let bytes = ctx.message.len();
        let result = self.inner.send_message(ctx).await;
        self.audit("send_message", if result.is_ok() { "ok" } else { "err" });
        if result.is_ok() {
            self.metrics
                .messages_sent
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            self.metrics
                .bytes_sent
                .fetch_add(bytes as u64, std::sync::atomic::Ordering::Relaxed);
        } else {
            self.metrics
                .errors
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        result
    }

    async fn get_history(
        &self,
        session_key: Option<String>,
    ) -> Result<Vec<HistoryMessage>, TransportError> {
        self.require_auth()?;
        let result = self.inner.get_history(session_key).await;
        self.audit("get_history", if result.is_ok() { "ok" } else { "err" });
        if result.is_err() {
            self.metrics
                .errors
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        result
    }

    async fn sync_role_context(
        &self,
        role_id: String,
        since_event_id: Option<String>,
    ) -> Result<(), TransportError> {
        self.require_auth()?;
        let result = self.inner.sync_role_context(role_id, since_event_id).await;
        self.audit(
            "sync_role_context",
            if result.is_ok() { "ok" } else { "err" },
        );
        if result.is_err() {
            self.metrics
                .errors
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        result
    }

    async fn abort(&self) -> Result<(), TransportError> {
        self.require_auth()?;
        let result = self.inner.abort().await;
        self.audit("abort", if result.is_ok() { "ok" } else { "err" });
        if result.is_err() {
            self.metrics
                .errors
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        result
    }

    fn events(&self) -> BoxStream<'static, TransportEvent> {
        // ponytail: metrics for received events are updated by the consumer
        // because the stream may be fanned-out. If per-event accounting is
        // needed, wrap the stream here with StreamExt::map.
        self.inner.events()
    }

    fn capabilities(&self) -> TransportCaps {
        self.inner.capabilities()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caps_supports_method() {
        let caps = TransportCaps {
            methods: vec!["chat.send".into(), "chat.history".into()],
            ..Default::default()
        };
        assert!(caps.supports_method("chat.send"));
        assert!(!caps.supports_method("device.pair.request"));
    }

    #[test]
    fn transport_event_serializes_tag() {
        let ev = TransportEvent::ChatChunk {
            content: "hello".into(),
        };
        let json = serde_json::to_string(&ev).unwrap();
        assert!(json.contains("\"kind\":\"chat_chunk\""));
    }

    #[test]
    fn transport_error_roundtrip() {
        let err = TransportError::Remote {
            code: "E1".into(),
            message: "boom".into(),
            retryable: true,
        };
        let json = serde_json::to_string(&err).unwrap();
        let restored: TransportError = serde_json::from_str(&json).unwrap();
        match restored {
            TransportError::Remote {
                code,
                message,
                retryable,
            } => {
                assert_eq!(code, "E1");
                assert_eq!(message, "boom");
                assert!(retryable);
            }
            _ => panic!("expected remote error"),
        }
    }
}
