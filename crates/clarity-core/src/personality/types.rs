//! Personality System - Type Definitions
//!
//! Defines the core types for the three-layer personality system:
//! - Identity: Short identity description (one-liner)
//! - Yuan: Capability/thinking structure (MOOD/PULSE/Contemplation)
//! - Ishiki: Detailed personality definition (behavior norms, tone guidelines)

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Three-layer personality structure
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Personality {
    /// Identity layer: Short identity description (identity.md content)
    pub identity: String,
    /// Yuan layer: Capability/thinking structure (yuan.md content)
    pub yuan: String,
    /// Ishiki layer: Detailed personality definition (ishiki.md content)
    pub ishiki: String,
}

/// Type of Yuan (determines thinking/capability structure)
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum YuanType {
    /// Hanako: Balanced感性 and理性, MOOD module
    #[default]
    Hanako,
    /// Butter:感性优先, PULSE module
    Butter,
    /// Ming:理性优先, Contemplation module
    Ming,
}

impl YuanType {
    /// Get the template name for this Yuan type
    pub fn template_name(&self) -> &'static str {
        match self {
            YuanType::Hanako => "hanako",
            YuanType::Butter => "butter",
            YuanType::Ming => "ming",
        }
    }

    /// Get display name for this Yuan type
    pub fn display_name(&self) -> &'static str {
        match self {
            YuanType::Hanako => "Hanako",
            YuanType::Butter => "Butter",
            YuanType::Ming => "Ming",
        }
    }

    /// Get default yuan module description
    pub fn default_yuan_content(&self) -> &'static str {
        match self {
            YuanType::Hanako => include_str!("../../templates/yuan/hanako.md"),
            YuanType::Butter => include_str!("../../templates/yuan/butter.md"),
            YuanType::Ming => include_str!("../../templates/yuan/ming.md"),
        }
    }

    /// Get default ishiki content
    pub fn default_ishiki_content(&self) -> &'static str {
        match self {
            YuanType::Hanako => include_str!("../../templates/ishiki-templates/hanako.md"),
            YuanType::Butter => include_str!("../../templates/ishiki-templates/butter.md"),
            YuanType::Ming => include_str!("../../templates/ishiki-templates/ming.md"),
        }
    }
}

impl std::fmt::Display for YuanType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.template_name())
    }
}

impl std::str::FromStr for YuanType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "hanako" => Ok(YuanType::Hanako),
            "butter" => Ok(YuanType::Butter),
            "ming" => Ok(YuanType::Ming),
            _ => Err(format!("Unknown YuanType: {}", s)),
        }
    }
}

/// Configuration for personality loading
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonalityConfig {
    /// Name of the agent/assistant
    pub agent_name: String,
    /// Name of the user
    pub user_name: String,
    /// Type of Yuan (determines thinking structure)
    pub yuan_type: YuanType,
    /// Locale for templates (e.g., "zh-CN", "en")
    pub locale: String,
    /// Optional path to agent directory for custom templates
    pub agent_dir: Option<PathBuf>,
}

impl Default for PersonalityConfig {
    fn default() -> Self {
        Self {
            agent_name: "Clarity".to_string(),
            user_name: "User".to_string(),
            yuan_type: YuanType::default(),
            locale: "zh-CN".to_string(),
            agent_dir: None,
        }
    }
}

impl PersonalityConfig {
    /// Create a new config with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Set agent name
    pub fn with_agent_name(mut self, name: impl Into<String>) -> Self {
        self.agent_name = name.into();
        self
    }

    /// Set user name
    pub fn with_user_name(mut self, name: impl Into<String>) -> Self {
        self.user_name = name.into();
        self
    }

    /// Set Yuan type
    pub fn with_yuan_type(mut self, yuan_type: YuanType) -> Self {
        self.yuan_type = yuan_type;
        self
    }

    /// Set locale
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = locale.into();
        self
    }

    /// Set agent directory for custom templates
    pub fn with_agent_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.agent_dir = Some(dir.into());
        self
    }
}
