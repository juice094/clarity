//! Claw Mesh role-context synchronization contracts.
//!
//! These types describe the events that converge into a shared `RoleContext`
//! across multiple Claw device nodes. They are intentionally transport-agnostic:
//! they can be exchanged over the Gateway `/claw/sync` channel or through the
//! syncthing-rust virtual-file transport.

use serde::{Deserialize, Serialize};

/// Identifier for a shared role context.
///
/// In practice this maps to a single Claw role, e.g. `"operator"`. All devices
/// that share the same role id converge on the same context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct RoleContextId(pub String);

impl RoleContextId {
    /// Construct a role context id from a role name.
    pub fn new(role: impl Into<String>) -> Self {
        Self(role.into())
    }

    /// Syncthing folder id used when this role is synchronized as a folder.
    ///
    /// Uses an underscore separator so the id is safe to use in filenames on
    /// Windows (colons are reserved).
    pub fn syncthing_folder_id(&self) -> String {
        format!("claw_role_{}", self.0)
    }

    /// Local directory name component for this role's mesh data.
    pub fn local_dir_name(&self) -> String {
        format!("role-{}", self.0)
    }
}

impl AsRef<str> for RoleContextId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// A mutation to a role context.
///
/// Events are independent and idempotent. The merger applies them in
/// `(origin_clock, event_id)` order; duplicate `event_id`s are skipped.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClawContextEvent {
    /// Globally unique event id. Should be deterministic from the payload and
    /// origin metadata, e.g. `sha256(origin_device + origin_clock + payload)`.
    pub event_id: String,
    /// Device that produced the event.
    pub origin_device: String,
    /// Logical clock of the originating device when the event was created.
    pub origin_clock: u64,
    /// Event kind and payload.
    #[serde(flatten)]
    pub kind: ContextEventKind,
}

/// Payload variants for `ClawContextEvent`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ContextEventKind {
    /// Append a message to the role's shared conversation.
    AppendMessage {
        /// Message role (user / assistant / system).
        role: String,
        /// Message content.
        content: String,
    },
    /// Edit an existing message.
    EditMessage {
        /// `event_id` of the message to edit.
        target_event_id: String,
        /// New content.
        content: String,
    },
    /// Change the session lifecycle (temporary → persistent → archived).
    SetLifecycle {
        /// Target lifecycle value.
        lifecycle: String,
    },
    /// Update free-form metadata key/value pairs.
    UpdateMetadata {
        /// Metadata deltas.
        deltas: std::collections::HashMap<String, String>,
    },
    /// Archive or restore the role context.
    Archive {
        /// True to archive, false to restore.
        archived: bool,
    },
}

/// Request to fetch missing events for a role.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncRequest {
    /// Role context to sync.
    pub role_id: RoleContextId,
    /// Last event id already known by the requester; empty means "from beginning".
    pub since_event_id: Option<String>,
    /// Device id of the requester.
    pub device_id: String,
}

/// Response containing missing events and a pagination cursor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SyncResponse {
    /// Events missing on the requester, ordered by `(origin_clock, event_id)`.
    pub events: Vec<ClawContextEvent>,
    /// Cursor for the next sync request; None if the server has no more events.
    pub next_cursor: Option<String>,
    /// Devices currently online for this role.
    pub online_devices: Vec<String>,
}

/// A converged view of a role context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoleContext {
    /// Role identifier.
    pub role_id: RoleContextId,
    /// Ordered shared messages.
    pub messages: Vec<RoleContextMessage>,
    /// Current lifecycle value.
    pub lifecycle: String,
    /// Archive flag.
    pub archived: bool,
    /// Free-form metadata.
    pub metadata: std::collections::HashMap<String, String>,
}

impl Default for RoleContext {
    /// Default context with an empty role id.
    ///
    /// ponytail: empty role id is a placeholder; real contexts must be
    /// constructed with a concrete `RoleContextId`.
    fn default() -> Self {
        Self {
            role_id: RoleContextId(String::new()),
            messages: Vec::new(),
            lifecycle: String::new(),
            archived: false,
            metadata: std::collections::HashMap::new(),
        }
    }
}

/// A message entry in the converged role context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoleContextMessage {
    /// Event id that produced this message.
    pub event_id: String,
    /// Author device.
    pub origin_device: String,
    /// Logical clock for ordering.
    pub origin_clock: u64,
    /// Message role.
    pub role: String,
    /// Message content.
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_context_id_syncthing_folder() {
        let id = RoleContextId::new("operator");
        assert_eq!(id.syncthing_folder_id(), "claw_role_operator");
        assert_eq!(id.local_dir_name(), "role-operator");
    }

    #[test]
    fn event_serde_roundtrip() {
        let event = ClawContextEvent {
            event_id: "ev-1".into(),
            origin_device: "dev-a".into(),
            origin_clock: 42,
            kind: ContextEventKind::AppendMessage {
                role: "user".into(),
                content: "hello".into(),
            },
        };
        let json = serde_json::to_string(&event).unwrap();
        let restored: ClawContextEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(event, restored);
    }

    #[test]
    fn sync_response_roundtrip() {
        let resp = SyncResponse {
            events: vec![],
            next_cursor: Some("c-1".into()),
            online_devices: vec!["dev-a".into()],
        };
        let json = serde_json::to_string(&resp).unwrap();
        let restored: SyncResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(resp, restored);
    }
}
