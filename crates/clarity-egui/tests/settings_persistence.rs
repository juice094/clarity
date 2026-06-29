#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Integration test: settings save → read back via the library target.
//!
//! Validates the API key encryption layer (transparent serde) and atomic
//! tmp→rename write behavior.

use clarity_egui::test_util::with_temp_dir;
use serde::{Deserialize, Serialize};

/// Minimal copy of `GuiSettings` for integration-testing the serde layer
/// without depending on the full `GuiSettings` (which reads env vars on load).
#[derive(Serialize, Deserialize, Clone, Debug, Default)]
struct TestSettings {
    pub provider: String,
    pub model: String,
    #[serde(
        default,
        serialize_with = "clarity_egui::settings::serialize_api_key",
        deserialize_with = "clarity_egui::settings::deserialize_api_key"
    )]
    pub api_key: Option<String>,
}

// Re-export reference for serde — the actual functions live in clarity_egui::settings.
// We test that the public serde helpers are accessible and produce enc2: format.

#[test]
fn integration_api_key_serializes_as_enc2() {
    let settings = TestSettings {
        provider: "openai".into(),
        model: "gpt-4o".into(),
        api_key: Some("sk-test-integration-key".into()),
    };

    let json = serde_json::to_string_pretty(&settings).expect("serialize");
    // The key should NOT appear in plaintext.
    assert!(
        !json.contains("sk-test-integration-key"),
        "api_key should be encrypted on disk, got: {}",
        json
    );
    // The key should be an enc2: blob.
    assert!(
        json.contains("enc2:"),
        "api_key should use enc2: prefix, got: {}",
        json
    );
}

#[test]
fn integration_api_key_none_serializes_as_null() {
    let settings = TestSettings {
        provider: "local".into(),
        model: String::new(),
        api_key: None,
    };

    let json = serde_json::to_string_pretty(&settings).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(
        parsed.get("api_key").map(|v| v.is_null()).unwrap_or(false),
        "None api_key should serialize as null"
    );
}

#[test]
fn integration_api_key_empty_is_none() {
    let settings = TestSettings {
        provider: "local".into(),
        model: String::new(),
        api_key: Some(String::new()),
    };

    let json = serde_json::to_string_pretty(&settings).expect("serialize");
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(
        parsed.get("api_key").map(|v| v.is_null()).unwrap_or(false),
        "empty api_key should serialize as null"
    );
}

#[test]
fn integration_api_key_decryption_roundtrip() {
    // Write with an API key → serialize (encrypts).
    let settings = TestSettings {
        provider: "anthropic".into(),
        model: "claude-sonnet".into(),
        api_key: Some("sk-ant-test-123".into()),
    };

    let json = serde_json::to_string_pretty(&settings).expect("serialize");

    // Read back → deserialize (decrypts).
    let restored: TestSettings = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(restored.provider, "anthropic");
    assert_eq!(restored.model, "claude-sonnet");
    // The key should decrypt back to the original plaintext.
    assert_eq!(
        restored.api_key.as_deref(),
        Some("sk-ant-test-123"),
        "decrypted api_key should match original"
    );
}

#[test]
fn integration_settings_atomic_write() {
    with_temp_dir("integration_settings_atomic", |tmp| {
        let path = tmp.join("gui-settings.json");
        let tmp_path = tmp.join("gui-settings.json.tmp");

        let settings = TestSettings {
            provider: "test".into(),
            model: "test-model".into(),
            api_key: None,
        };

        let json = serde_json::to_string_pretty(&settings).unwrap();

        // Simulate atomic write
        std::fs::write(&tmp_path, &json).unwrap();
        assert!(tmp_path.exists());
        assert!(!path.exists());

        std::fs::rename(&tmp_path, &path).unwrap();
        assert!(path.exists());
        assert!(!tmp_path.exists(), "tmp file should be gone after rename");

        // Read back
        let raw = std::fs::read_to_string(&path).unwrap();
        let restored: TestSettings = serde_json::from_str(&raw).unwrap();
        assert_eq!(restored.provider, "test");
    });
}
