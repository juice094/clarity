//! TierBus — hierarchical communication for the Agent OS.
//!
//! Provides three communication patterns:
//!
//! | Pattern | Direction | Permission |
//! |---------|-----------|------------|
//! | `ParentDirective` | Parent → Child | Parent can write; child receives read-only |
//! | `PeerAnnouncement` | Peer → Peer | Broadcast to bulletin board; read-only for all |
//! | `ChildQuery` | Child → Parent | Child can read parent's public state; parent can deny |
//!
//! ## Inequality rules
//!
//! - Parent can read/write Child's **full** state.
//! - Child can read Parent's **public** state only; writes are rejected.
//! - Peer-to-peer has no direct channel; only the bulletin board.

use chrono::{DateTime, Duration, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// TierMessage
// ============================================================================

/// Messages flowing through the TierBus.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum TierMessage {
    /// Parent injects directives into a child soul.
    ParentDirective {
        from: String,
        to: String,
        payload: DirectivePayload,
        priority: Priority,
    },

    /// Peer publishes to the shared bulletin board.
    PeerAnnouncement {
        from: String,
        topic: String,
        payload: AnnouncementPayload,
        /// Time-to-live for this announcement.
        ttl_seconds: u64,
    },

    /// Child queries parent's public state.
    ChildQuery {
        from: String,
        to: String,
        query: QueryPayload,
    },
}

/// Priority level for directives.
#[derive(
    Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash, Default,
)]
#[serde(rename_all = "lowercase")]
pub enum Priority {
    Background,
    #[default]
    Normal,
    Critical,
}

/// Payload of a parent directive.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct DirectivePayload {
    /// System prompt fragment to inject.
    pub system_prompt_fragment: Option<String>,
    /// Memory summary to make available.
    pub memory_summary: Option<String>,
    /// High-level objective.
    pub objective: Option<String>,
    /// Arbitrary metadata.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub meta: HashMap<String, serde_json::Value>,
}

/// Payload of a peer announcement.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct AnnouncementPayload {
    /// Human-readable content.
    pub content: String,
    /// Structured data.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub data: HashMap<String, serde_json::Value>,
}

/// Payload of a child query.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct QueryPayload {
    /// What information is being requested.
    pub question: String,
    /// Expected response schema (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expected_schema: Option<String>,
}

// ============================================================================
// TierBus
// ============================================================================

/// Hierarchical message bus with bulletin board.
pub struct TierBus {
    /// Parent → Child directive queues.
    directives: RwLock<HashMap<String, Vec<TierMessage>>>,

    /// Bulletin board: topic → (payload, expiry).
    bulletin_board: RwLock<HashMap<String, (AnnouncementPayload, DateTime<Utc>)>>,

    /// Parent public state: soul_id → serialized public data.
    public_state: RwLock<HashMap<String, serde_json::Value>>,

    /// Hierarchy: child_id → parent_id.
    hierarchy: RwLock<HashMap<String, String>>,
}

impl TierBus {
    /// Create a new empty TierBus.
    pub fn new() -> Self {
        Self {
            directives: RwLock::new(HashMap::new()),
            bulletin_board: RwLock::new(HashMap::new()),
            public_state: RwLock::new(HashMap::new()),
            hierarchy: RwLock::new(HashMap::new()),
        }
    }

    // ------------------------------------------------------------------
    // Hierarchy management
    // ------------------------------------------------------------------

    /// Register a parent-child relationship.
    pub fn register_parent(&self, child_id: impl Into<String>, parent_id: impl Into<String>) {
        self.hierarchy
            .write()
            .insert(child_id.into(), parent_id.into());
    }

    /// Get the parent of a soul, if any.
    pub fn parent_of(&self, soul_id: &str) -> Option<String> {
        self.hierarchy.read().get(soul_id).cloned()
    }

    /// Get all children of a parent.
    pub fn children_of(&self, parent_id: &str) -> Vec<String> {
        let h = self.hierarchy.read();
        h.iter()
            .filter(|(_, p)| *p == parent_id)
            .map(|(c, _)| c.clone())
            .collect()
    }

    // ------------------------------------------------------------------
    // ParentDirective
    // ------------------------------------------------------------------

    /// Send a directive from parent to child.
    ///
    /// Returns `false` if the sender is not the registered parent.
    pub fn send_directive(
        &self,
        from: &str,
        to: &str,
        payload: DirectivePayload,
        priority: Priority,
    ) -> bool {
        // Verify sender is the registered parent.
        if let Some(parent) = self.parent_of(to) {
            if parent != from {
                return false;
            }
        }

        let msg = TierMessage::ParentDirective {
            from: from.to_string(),
            to: to.to_string(),
            payload,
            priority,
        };

        let mut directives = self.directives.write();
        directives.entry(to.to_string()).or_default().push(msg);
        true
    }

    /// Receive all pending directives for a soul.
    pub fn recv_directives(&self, soul_id: &str) -> Vec<TierMessage> {
        let mut directives = self.directives.write();
        directives.remove(soul_id).unwrap_or_default()
    }

    // ------------------------------------------------------------------
    // PeerAnnouncement (Bulletin Board)
    // ------------------------------------------------------------------

    /// Publish an announcement to the bulletin board.
    pub fn announce(
        &self,
        _from: impl Into<String>,
        topic: impl Into<String>,
        payload: AnnouncementPayload,
        ttl: Duration,
    ) {
        let expiry = Utc::now() + ttl;
        let mut board = self.bulletin_board.write();
        board.insert(topic.into(), (payload, expiry));
    }

    /// Read an announcement by topic (if not expired).
    pub fn read_announcement(&self, topic: &str) -> Option<AnnouncementPayload> {
        let board = self.bulletin_board.read();
        board.get(topic).and_then(|(payload, expiry)| {
            if Utc::now() < *expiry {
                Some(payload.clone())
            } else {
                None
            }
        })
    }

    /// List all active topics.
    pub fn active_topics(&self) -> Vec<String> {
        let board = self.bulletin_board.read();
        let now = Utc::now();
        board
            .iter()
            .filter(|(_, (_, expiry))| *expiry > now)
            .map(|(topic, _)| topic.clone())
            .collect()
    }

    /// Prune expired announcements.
    pub fn prune_expired(&self) {
        let mut board = self.bulletin_board.write();
        let now = Utc::now();
        board.retain(|_, (_, expiry)| *expiry > now);
    }

    // ------------------------------------------------------------------
    // ChildQuery
    // ------------------------------------------------------------------

    /// Publish public state for a soul (parent makes this available).
    pub fn set_public_state(&self, soul_id: impl Into<String>, state: serde_json::Value) {
        self.public_state.write().insert(soul_id.into(), state);
    }

    /// Query a parent's public state.
    ///
    /// Returns `None` if the target has no public state or the querier
    /// is not a registered child.
    pub fn query_parent_state(&self, child_id: &str, parent_id: &str) -> Option<serde_json::Value> {
        // Verify hierarchy.
        if let Some(registered_parent) = self.parent_of(child_id) {
            if registered_parent != parent_id {
                return None;
            }
        } else {
            return None;
        }

        self.public_state.read().get(parent_id).cloned()
    }
}

impl Default for TierBus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hierarchy() {
        let bus = TierBus::new();
        bus.register_parent("child", "parent");

        assert_eq!(bus.parent_of("child"), Some("parent".to_string()));
        assert_eq!(bus.children_of("parent"), vec!["child"]);
    }

    #[test]
    fn test_directive_parent_verification() {
        let bus = TierBus::new();
        bus.register_parent("child", "parent");

        // Legitimate parent → success.
        assert!(bus.send_directive(
            "parent",
            "child",
            DirectivePayload::default(),
            Priority::Normal
        ));

        // Impostor → rejected.
        assert!(!bus.send_directive(
            "impostor",
            "child",
            DirectivePayload::default(),
            Priority::Normal
        ));
    }

    #[test]
    fn test_directive_roundtrip() {
        let bus = TierBus::new();
        bus.register_parent("child", "parent");

        let payload = DirectivePayload {
            objective: Some("complete task X".to_string()),
            ..Default::default()
        };
        bus.send_directive("parent", "child", payload.clone(), Priority::Critical);

        let msgs = bus.recv_directives("child");
        assert_eq!(msgs.len(), 1);
        assert!(matches!(
            &msgs[0],
            TierMessage::ParentDirective {
                priority: Priority::Critical,
                ..
            }
        ));
    }

    #[test]
    fn test_bulletin_board_ttl() {
        let bus = TierBus::new();
        bus.announce(
            "peer-a",
            "status",
            AnnouncementPayload {
                content: "online".to_string(),
                ..Default::default()
            },
            Duration::hours(1),
        );

        assert!(bus.read_announcement("status").is_some());
        assert_eq!(bus.active_topics(), vec!["status"]);
    }

    #[test]
    fn test_bulletin_board_expiry() {
        let bus = TierBus::new();
        bus.announce(
            "peer-a",
            "temp",
            AnnouncementPayload::default(),
            Duration::seconds(-1), // already expired
        );

        assert!(bus.read_announcement("temp").is_none());
    }

    #[test]
    fn test_child_query_public_state() {
        let bus = TierBus::new();
        bus.register_parent("child", "parent");
        bus.set_public_state("parent", serde_json::json!({"status": "active"}));

        let state = bus.query_parent_state("child", "parent");
        assert_eq!(state, Some(serde_json::json!({"status": "active"})));

        // Non-child cannot query.
        let denied = bus.query_parent_state("stranger", "parent");
        assert_eq!(denied, None);
    }
}
