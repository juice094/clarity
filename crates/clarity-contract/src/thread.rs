//! Thread and session identifiers.
//!
//! These identifiers are ported from the OpenAI Codex `codex_protocol` crate and
//! adapted for the Clarity codebase. Codex is licensed under Apache-2.0; see
//! `crates/clarity-contract/NOTICES.md` for attribution.

use std::fmt::Display;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Unique identifier for a persistent conversation thread.
///
/// Backed by a UUIDv7 so that lexicographic ordering approximates creation time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ThreadId {
    pub(crate) uuid: Uuid,
}

impl ThreadId {
    /// Create a new thread identifier.
    pub fn new() -> Self {
        Self {
            uuid: Uuid::now_v7(),
        }
    }

    /// Parse a thread identifier from a string representation of a UUID.
    pub fn from_string(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self {
            uuid: Uuid::parse_str(s)?,
        })
    }
}

impl TryFrom<&str> for ThreadId {
    type Error = uuid::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_string(value)
    }
}

impl TryFrom<String> for ThreadId {
    type Error = uuid::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_string(value.as_str())
    }
}

impl From<ThreadId> for String {
    fn from(value: ThreadId) -> Self {
        value.to_string()
    }
}

impl Default for ThreadId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for ThreadId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.uuid, f)
    }
}

impl Serialize for ThreadId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&self.uuid)
    }
}

impl<'de> Deserialize<'de> for ThreadId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&value).map_err(serde::de::Error::custom)?;
        Ok(Self { uuid })
    }
}

/// Unique identifier for a runtime session.
///
/// In Codex a session spans the root thread and all spawned sub-threads. In
/// Clarity this is currently kept as a thin wrapper around the same UUID as the
/// root thread, but the type is preserved for future separation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SessionId {
    pub(crate) uuid: Uuid,
}

impl SessionId {
    /// Create a new session identifier.
    pub fn new() -> Self {
        Self {
            uuid: Uuid::now_v7(),
        }
    }

    /// Parse a session identifier from a string representation of a UUID.
    pub fn from_string(s: &str) -> Result<Self, uuid::Error> {
        Ok(Self {
            uuid: Uuid::parse_str(s)?,
        })
    }
}

impl TryFrom<&str> for SessionId {
    type Error = uuid::Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::from_string(value)
    }
}

impl TryFrom<String> for SessionId {
    type Error = uuid::Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_string(value.as_str())
    }
}

impl From<SessionId> for String {
    fn from(value: SessionId) -> Self {
        value.to_string()
    }
}

impl From<ThreadId> for SessionId {
    fn from(value: ThreadId) -> Self {
        Self { uuid: value.uuid }
    }
}

impl From<SessionId> for ThreadId {
    fn from(value: SessionId) -> Self {
        ThreadId { uuid: value.uuid }
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.uuid, f)
    }
}

impl Serialize for SessionId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.collect_str(&self.uuid)
    }
}

impl<'de> Deserialize<'de> for SessionId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        let uuid = Uuid::parse_str(&value).map_err(serde::de::Error::custom)?;
        Ok(Self { uuid })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thread_id_default_is_not_zeroes() {
        let id = ThreadId::default();
        assert_ne!(id.uuid, Uuid::nil());
    }

    #[test]
    fn session_and_thread_ids_roundtrip() {
        let thread_id = ThreadId::new();
        let session_id = SessionId::from(thread_id);
        assert_eq!(ThreadId::from(session_id), thread_id);
        let s: String = thread_id.into();
        let parsed = ThreadId::from_string(&s).expect("parse thread id");
        assert_eq!(parsed, thread_id);
    }
}
