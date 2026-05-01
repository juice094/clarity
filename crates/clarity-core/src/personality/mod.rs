//! Personality system for Project Clarity.
//!
//! Provides runtime template-variable injection and domain-specific
//! persona configuration parsing (TOML).
//!
//! NOTE: This module is currently inactive and kept for future integration.
#![allow(dead_code, unused_imports)]

pub mod domain;
pub mod types;

pub use domain::{
    parse_domain_persona, BasePersona, DomainPersonaConfig, DomainToolSchema, SystemPromptConfig,
};
pub use types::PersonalityConfig;
