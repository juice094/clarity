//! Personality System for Clarity
//!
//! A three-layer personality system inspired by OpenHanako:
//! - **Identity**: Short identity description (one-liner)
//! - **Yuan**: Capability/thinking structure (MOOD/PULSE/Contemplation)
//! - **Ishiki**: Detailed personality definition (behavior norms, tone guidelines)
//!
//! ## Usage
//!
//! ```rust,no_run
//! use clarity_core::personality::{PersonalityLoader, PersonalityConfig, YuanType};
//! use clarity_core::personality::SystemPromptBuilder;
//!
//! fn setup_personality() {
//!     // Load personality
//!     let loader = PersonalityLoader::new();
//!     let config = PersonalityConfig::new()
//!         .with_agent_name("Clarity")
//!         .with_user_name("User")
//!         .with_yuan_type(YuanType::Hanako)
//!         .with_locale("zh-CN");
//!
//!     let personality = loader.load(&config).expect("Failed to load personality");
//!
//!     // Build system prompt
//!     let system_prompt = SystemPromptBuilder::new(personality)
//!         .with_memory("Previous conversations...")
//!         .with_skills(vec!["File operations".to_string()])
//!         .build();
//! }
//! ```

pub mod builder;
pub mod domain;
pub mod loader;
pub mod types;

// Re-export main types
pub use builder::{presets, SystemPromptBuilder};
pub use domain::{parse_domain_persona, parse_domain_persona_str, DomainPersonaConfig};
pub use loader::PersonalityLoader;
pub use types::{Personality, PersonalityConfig, YuanType};
