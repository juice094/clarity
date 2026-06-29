#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! Integration test: provider registry loading and type conversions.
//!
//! Validates `ProviderRegistry::load()` and `ProviderDefinition` methods
//! through the library's public API.

use clarity_egui::provider::{ApiFormat, AuthType, ProviderDefinition, ProviderRegistry};

#[test]
fn integration_builtin_providers_loaded() {
    let registry = ProviderRegistry::load();
    let providers = registry.list();
    assert!(
        providers.len() >= 5,
        "expected at least 5 built-in providers, got {}",
        providers.len()
    );

    // Core built-in providers must exist.
    assert!(
        registry.get("openai").is_some(),
        "openai provider must exist"
    );
    assert!(registry.get("local").is_some(), "local provider must exist");
}

#[test]
fn integration_api_format_from_str_roundtrip() {
    for fmt in [
        ApiFormat::OpenaiCompletions,
        ApiFormat::AnthropicMessages,
        ApiFormat::Kimi,
        ApiFormat::DeepSeekDevice,
    ] {
        let roundtripped = ApiFormat::from_str(fmt.as_str());
        assert_eq!(
            roundtripped, fmt,
            "ApiFormat::{fmt:?} as_str/from_str should roundtrip"
        );
    }
}

#[test]
fn integration_api_format_unknown_falls_back() {
    assert_eq!(
        ApiFormat::from_str("nonexistent-format"),
        ApiFormat::OpenaiCompletions
    );
    assert_eq!(ApiFormat::from_str(""), ApiFormat::OpenaiCompletions);
}

#[test]
fn integration_auth_type_serde_roundtrip() {
    for auth in [AuthType::ApiKey, AuthType::OAuth, AuthType::None] {
        let json = serde_json::to_string(&auth).unwrap();
        let restored: AuthType = serde_json::from_str(&json).unwrap();
        assert_eq!(auth, restored);
    }
}

#[test]
fn integration_provider_definition_display_fallback() {
    let def = ProviderDefinition {
        id: "custom-prov".into(),
        display_name: String::new(),
        base_url: "https://example.com".into(),
        api_format: ApiFormat::OpenaiCompletions,
        auth_type: AuthType::ApiKey,
        api_key_ref: String::new(),
        ..Default::default()
    };
    assert_eq!(def.display(), "custom-prov");
}

#[test]
fn integration_provider_definition_display_name() {
    let def = ProviderDefinition {
        id: "prov-id".into(),
        display_name: "Pretty Name".into(),
        base_url: "https://example.com".into(),
        api_format: ApiFormat::OpenaiCompletions,
        auth_type: AuthType::ApiKey,
        api_key_ref: String::new(),
        ..Default::default()
    };
    assert_eq!(def.display(), "Pretty Name");
}

#[test]
fn integration_chat_only_provider_no_tools() {
    let def = ProviderDefinition {
        id: "chat".into(),
        display_name: String::new(),
        base_url: "https://example.com".into(),
        api_format: ApiFormat::OpenaiCompletions,
        auth_type: AuthType::None,
        api_key_ref: String::new(),
        tags: vec!["chat-only".into()],
        ..Default::default()
    };
    assert!(!def.supports_tools());
    assert!(def.is_chat_only());
}

#[test]
fn integration_prompt_guided_overrides_chat_only() {
    let mut def = ProviderDefinition {
        id: "chat-tools".into(),
        display_name: String::new(),
        base_url: "https://example.com".into(),
        api_format: ApiFormat::OpenaiCompletions,
        auth_type: AuthType::None,
        api_key_ref: String::new(),
        tags: vec!["chat-only".into()],
        prompt_guided_tool_calling: true,
        ..Default::default()
    };
    assert!(def.supports_tools());
    // Still chat-only for session routing.
    assert!(def.is_chat_only());
    // But prompt-guided allows it in Work sessions.
    def.prompt_guided_tool_calling = false;
    assert!(!def.supports_tools());
}
