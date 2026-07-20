//! Shared settings data types used across Clarity front-ends and headless consumers.
//!
//! UI-coupled types such as `GuiSettings` and persistence logic remain in the egui crate.
//! This module only holds the plain data shapes so that headless / TUI / core consumers can
//! reference them without pulling in the egui dependency tree.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A named profile that overrides provider/model/approval_mode for a specific use-case.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct AgentProfile {
    pub model: String,
    pub provider: String,
    pub approval_mode: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub local_model_path: Option<String>,
}

/// Top-level structure of `profiles.toml`.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct ProfilesFile {
    #[serde(default)]
    pub profiles: HashMap<String, AgentProfile>,
}

/// A user-defined web bookmark shown in the left sidebar web section.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct WebLink {
    pub name: String,
    pub url: String,
}

/// A user-defined work template that launches a new session with a pre-filled prompt.
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
pub struct WorkTemplate {
    pub name: String,
    pub prompt: String,
}

/// Authentication mode for a user-configured OpenClaw Gateway connection.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpenClawAuthMode {
    /// Plain token auth. Suitable for local or permissive Gateways.
    #[default]
    TokenOnly,
    /// Remote admin/device token plus Ed25519 device attestation.
    TokenWithDevice,
    /// Device token returned by the Gateway after pairing.
    DevicePaired,
}

/// Which JSON-RPC send method an OpenClaw connection should use.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OpenClawSendMethod {
    /// `sessions.send` with `key` — typical remote OpenClaw path.
    #[default]
    SessionsSend,
    /// `chat.send` with `sessionKey` — KimiClaw-local/ACP-style path.
    ChatSend,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_file_construction() {
        let mut file = ProfilesFile::default();
        file.profiles.insert(
            "default".into(),
            AgentProfile {
                model: "gpt-4o".into(),
                provider: "openai".into(),
                approval_mode: "interactive".into(),
                api_key: None,
                local_model_path: None,
            },
        );
        file.profiles.insert(
            "local".into(),
            AgentProfile {
                model: "local-qwen".into(),
                provider: "local".into(),
                approval_mode: "yolo".into(),
                api_key: None,
                local_model_path: None,
            },
        );
        assert_eq!(file.profiles.len(), 2);
        let local = file.profiles.get("local").unwrap();
        assert_eq!(local.provider, "local");
        assert_eq!(local.model, "local-qwen");
    }

    #[test]
    fn openclaw_auth_mode_roundtrip() {
        let variants = [
            OpenClawAuthMode::TokenOnly,
            OpenClawAuthMode::TokenWithDevice,
            OpenClawAuthMode::DevicePaired,
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let back: OpenClawAuthMode = serde_json::from_str(&json).unwrap();
            assert_eq!(back, v);
        }
    }

    #[test]
    fn openclaw_send_method_roundtrip() {
        let variants = [
            OpenClawSendMethod::SessionsSend,
            OpenClawSendMethod::ChatSend,
        ];
        for v in variants {
            let json = serde_json::to_string(&v).unwrap();
            let back: OpenClawSendMethod = serde_json::from_str(&json).unwrap();
            assert_eq!(back, v);
        }
    }
}
