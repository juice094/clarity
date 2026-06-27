//! Built-in HTTP-based LLM providers.
//!
//! These providers implement the shared [`LlmProvider`](crate::api::LlmProvider)
//! trait. The generic OpenAI-compatible implementation is reused by Kimi and
//! OAuth-backed providers.

pub mod anthropic;
pub mod kimi;
pub mod oauth;
pub mod openai_compatible;

pub use anthropic::AnthropicLlm;
pub use kimi::KimiLlm;
pub use oauth::{KimiCodeLlm, OAuthLlm};
pub use openai_compatible::OpenAiCompatibleLlm;
