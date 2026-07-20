//! KimiClaw session route configuration loader.
//!
//! KimiClaw stores a mapping from session keys (e.g. `agent:main:main`) to
//! concrete chat/room identifiers in
//! `~/.kimi_openclaw/plugins/kimi-claw/agents/main/session-routes.json`.
//! This module loads that mapping and exposes a lookup API so the rest of the
//! Claw stack can stop hard-coding `agent:main:main`.
//!
//! ponytail: only supports the `main` agent routes today. If other agents
//! start carrying routes, generalize `resolve_route` to accept an agent id.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// A single route entry from `session-routes.json`.
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq)]
pub struct SessionRoute {
    /// Channel that owns this route, e.g. `kimi-claw`.
    pub channel: String,
    /// Recipient / target agent id, e.g. `main`.
    pub to: String,
    /// Account id this route belongs to.
    #[serde(rename = "accountId")]
    pub account_id: String,
    /// Protocol used for replies, e.g. `im`.
    #[serde(rename = "replyProtocol")]
    pub reply_protocol: String,
    /// Class of session: `main` or `room`.
    #[serde(rename = "sessionClass")]
    pub session_class: String,
    /// Chat id used by the IM/ACP layer.
    #[serde(rename = "chatId")]
    pub chat_id: String,
    /// Room id for group/room sessions; empty for one-on-one sessions.
    #[serde(rename = "roomId")]
    pub room_id: String,
    /// Source that produced this chat id (e.g. `header_default`, `room_event`).
    #[serde(rename = "chatIdSource")]
    pub chat_id_source: String,
    /// Last update timestamp (milliseconds since epoch).
    #[serde(rename = "updatedAt")]
    pub updated_at: u64,
}

/// Load the full route table for the `main` agent.
///
/// Returns an empty map if the route file does not exist or cannot be parsed.
/// Callers that need strict error handling can use [`load_session_routes`].
pub fn load_main_routes<P: AsRef<Path>>(openclaw_home: P) -> HashMap<String, SessionRoute> {
    load_session_routes(routes_path(openclaw_home)).unwrap_or_default()
}

/// Load the full route table from a specific path.
pub fn load_session_routes<P: AsRef<Path>>(
    path: P,
) -> anyhow::Result<HashMap<String, SessionRoute>> {
    let raw = std::fs::read_to_string(path.as_ref())?;
    let routes: HashMap<String, SessionRoute> = serde_json::from_str(&raw)?;
    Ok(routes)
}

/// Resolve a session key to its route entry.
///
/// Returns `None` if the route file is missing or the key is not present.
pub fn resolve_route<P: AsRef<Path>>(openclaw_home: P, session_key: &str) -> Option<SessionRoute> {
    load_main_routes(openclaw_home).remove(session_key)
}

/// Canonical path to the `main` agent route file under an OpenClaw home dir.
pub fn routes_path<P: AsRef<Path>>(openclaw_home: P) -> PathBuf {
    openclaw_home
        .as_ref()
        .join("plugins")
        .join("kimi-claw")
        .join("agents")
        .join("main")
        .join("session-routes.json")
}

/// Pick the best session key to use for a given role.
///
/// If a route exists for `agent:main:{role}` it is returned; otherwise the
/// legacy `agent:main:main` key is returned. This preserves backward
/// compatibility while allowing per-role routing.
pub fn default_session_key_for_role<P: AsRef<Path>>(openclaw_home: P, role: &str) -> String {
    let key = format!("agent:main:{role}");
    if resolve_route(openclaw_home, &key).is_some() {
        key
    } else {
        "agent:main:main".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_route_file() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session-routes.json");
        (dir, path)
    }

    #[test]
    fn load_routes_round_trip() {
        let (_dir, path) = temp_route_file();
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(
            br#"{
                "agent:main:main": {
                    "channel": "kimi-claw",
                    "to": "main",
                    "accountId": "main",
                    "replyProtocol": "im",
                    "sessionClass": "main",
                    "chatId": "chat-abc",
                    "roomId": "",
                    "chatIdSource": "header_default",
                    "updatedAt": 1783069458396
                },
                "agent:main:kimi-claw:room:room-xyz": {
                    "channel": "kimi-claw",
                    "to": "main",
                    "accountId": "main",
                    "replyProtocol": "im",
                    "sessionClass": "room",
                    "chatId": "room-xyz",
                    "roomId": "room-xyz",
                    "chatIdSource": "room_event",
                    "updatedAt": 1779809461330
                }
            }"#,
        )
        .unwrap();

        let routes = load_session_routes(&path).unwrap();
        assert_eq!(routes.len(), 2);

        let main = routes.get("agent:main:main").unwrap();
        assert_eq!(main.chat_id, "chat-abc");
        assert_eq!(main.session_class, "main");
        assert!(main.room_id.is_empty());

        let room = routes.get("agent:main:kimi-claw:room:room-xyz").unwrap();
        assert_eq!(room.room_id, "room-xyz");
        assert_eq!(room.session_class, "room");
    }

    #[test]
    fn resolve_route_finds_existing_key() {
        let (dir, _path) = temp_route_file();
        let agents_dir = dir
            .path()
            .join("plugins")
            .join("kimi-claw")
            .join("agents")
            .join("main");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(
            agents_dir.join("session-routes.json"),
            br#"{
                "agent:main:main": {
                    "channel": "kimi-claw",
                    "to": "main",
                    "accountId": "main",
                    "replyProtocol": "im",
                    "sessionClass": "main",
                    "chatId": "chat-abc",
                    "roomId": "",
                    "chatIdSource": "header_default",
                    "updatedAt": 1783069458396
                }
            }"#,
        )
        .unwrap();

        let route = resolve_route(dir.path(), "agent:main:main").unwrap();
        assert_eq!(route.chat_id, "chat-abc");
        assert!(resolve_route(dir.path(), "agent:main:missing").is_none());
    }

    #[test]
    fn default_session_key_prefers_role_route() {
        let (dir, _path) = temp_route_file();
        let agents_dir = dir
            .path()
            .join("plugins")
            .join("kimi-claw")
            .join("agents")
            .join("main");
        std::fs::create_dir_all(&agents_dir).unwrap();
        std::fs::write(
            agents_dir.join("session-routes.json"),
            br#"{
                "agent:main:operator": {
                    "channel": "kimi-claw",
                    "to": "main",
                    "accountId": "main",
                    "replyProtocol": "im",
                    "sessionClass": "main",
                    "chatId": "chat-op",
                    "roomId": "",
                    "chatIdSource": "header_default",
                    "updatedAt": 1
                }
            }"#,
        )
        .unwrap();

        assert_eq!(
            default_session_key_for_role(dir.path(), "operator"),
            "agent:main:operator"
        );
        assert_eq!(
            default_session_key_for_role(dir.path(), "unknown"),
            "agent:main:main"
        );
    }

    #[test]
    fn missing_file_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_main_routes(dir.path()).is_empty());
        assert!(resolve_route(dir.path(), "agent:main:main").is_none());
    }
}
