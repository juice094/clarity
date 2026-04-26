//! Personality system for Project Clarity.
//!
//! Provides runtime template-variable injection and domain-specific
//! persona configuration parsing (TOML).

pub mod domain;
pub mod types;

pub use domain::{
    parse_domain_persona, BasePersona, DomainPersonaConfig, DomainToolSchema, SystemPromptConfig,
};
pub use types::PersonalityConfig;
