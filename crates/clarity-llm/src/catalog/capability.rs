//! Model-catalog pull capability — the single source of truth for
//! "can this provider's model list be fetched from an API?".
//!
//! Only three channel kinds expose a listable endpoint today:
//! - OpenAI-compatible `GET /v1/models` (`openai_chat` / `llama_server`)
//! - Ollama `GET /api/tags` (`ollama`)
//!
//! Everything else is deny-by-default: Anthropic has no public listing
//! endpoint, `deepseek-device` rides the App PoW channel, `openclaw` is a
//! gateway-relayed device, and local GGUF models are files, not an API.
//! OAuth device-flow providers (e.g. `kimi-code`) authenticate through a
//! coding-specific token that does not expose a listable `/v1/models`, so
//! they are excluded at the family level even though their wire protocol is
//! OpenAI-compatible.

use crate::model_registry::{AuthType, ProtocolType};
use crate::registry_table;

/// Whether a runtime `api_format` string supports remote catalog pull.
///
/// Mirrors the `api_format` values of [`crate::runtime::RuntimeProviderConfig`]
/// and the fetcher dispatch in
/// [`ModelCatalogService::refresh_provider`](super::service::ModelCatalogService::refresh_provider).
pub fn api_format_supports_catalog(api_format: &str) -> bool {
    matches!(api_format, "openai_chat" | "ollama" | "llama_server")
}

/// Whether a [`ProtocolType`] supports remote catalog pull.
pub fn protocol_supports_catalog(protocol: &ProtocolType) -> bool {
    matches!(
        protocol,
        ProtocolType::OpenAiChat | ProtocolType::Ollama | ProtocolType::LlamaServer
    )
}

/// Whether a configured provider supports remote catalog pull.
///
/// Combines protocol capability with the auth channel: OAuth device-flow
/// providers (`kimi-code`) speak OpenAI-compatible chat but their token
/// endpoint does not expose a listable `/v1/models`.
pub fn provider_config_supports_catalog(protocol: &ProtocolType, auth_type: &AuthType) -> bool {
    protocol_supports_catalog(protocol) && !matches!(auth_type, AuthType::OAuth)
}

/// Whether a canonical provider family supports remote catalog pull.
///
/// Unknown families (including gateway-relayed devices such as `openclaw`)
/// are denied by default.
pub fn family_supports_catalog(family: &str) -> bool {
    registry_table::family_defaults(family)
        .map(|d| provider_config_supports_catalog(&d.protocol, &d.auth_type))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_format_matrix() {
        for supported in ["openai_chat", "ollama", "llama_server"] {
            assert!(
                api_format_supports_catalog(supported),
                "{supported} should support catalog pull"
            );
        }
        for unsupported in [
            "anthropic_messages",
            "deepseek_device",
            "kalosm_local",
            "local_gguf",
            "openclaw",
            "unknown_format",
            "",
        ] {
            assert!(
                !api_format_supports_catalog(unsupported),
                "{unsupported} should NOT support catalog pull"
            );
        }
    }

    #[test]
    fn protocol_matrix() {
        for supported in [
            ProtocolType::OpenAiChat,
            ProtocolType::Ollama,
            ProtocolType::LlamaServer,
        ] {
            assert!(protocol_supports_catalog(&supported));
        }
        for unsupported in [
            ProtocolType::AnthropicMessages,
            ProtocolType::DeepSeekDevice,
        ] {
            assert!(!protocol_supports_catalog(&unsupported));
        }
    }

    #[test]
    fn oauth_excludes_openai_compatible_protocol() {
        assert!(!provider_config_supports_catalog(
            &ProtocolType::OpenAiChat,
            &AuthType::OAuth
        ));
        assert!(provider_config_supports_catalog(
            &ProtocolType::OpenAiChat,
            &AuthType::ApiKey
        ));
        assert!(provider_config_supports_catalog(
            &ProtocolType::Ollama,
            &AuthType::None
        ));
    }

    #[test]
    fn family_matrix() {
        for supported in [
            "openai",
            "deepseek",
            "kimi",
            "moonshot",
            "ollama",
            "llama-server",
        ] {
            assert!(
                family_supports_catalog(supported),
                "{supported} should support catalog pull"
            );
        }
        for unsupported in [
            "kimi-code",       // OAuth device flow: no listable /v1/models
            "anthropic",       // no public listing endpoint
            "deepseek-device", // App PoW channel
            "openclaw",        // gateway-relayed device, unknown to registry
            "no-such-family",
        ] {
            assert!(
                !family_supports_catalog(unsupported),
                "{unsupported} should NOT support catalog pull"
            );
        }
    }

    #[cfg(feature = "local-llm")]
    #[test]
    fn local_gguf_family_unsupported() {
        assert!(!family_supports_catalog("local"));
    }
}
