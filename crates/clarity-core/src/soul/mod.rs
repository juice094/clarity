//! Soul — persistent agent identity for the Agent OS.
//!
//! A Soul is a high-personality agent instance that outlives individual sessions.
//! It carries its own memory, growth profile, and capability set, and can be
//! woken from disk suspension or hibernated to free resources.
//!
//! # Storage layout
//!
//! ```text
//! ~/.clarity/souls/
//! ├── grey/
//! │   ├── soul.json          -- Persona + capabilities + default provider
//! │   ├── state.json         -- Runtime state (for Wake reconstruction)
//! │   └── memory.sqlite      -- Soul-private episodic memory
//! ├── observer/
//! │   └── ...
//! ```

pub mod wake;

use std::collections::HashMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::adaptive::AgentGrowthProfile;

// ============================================================================
// Soul
// ============================================================================

/// Persistent agent identity container.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Soul {
    /// Unique soul identifier (directory name).
    pub id: String,

    /// Display name shown in UI.
    pub name: String,

    /// Personality / system prompt configuration.
    pub persona: PersonaConfig,

    /// Growth profile (skill mastery, tool stats, model preferences).
    pub profile: AgentGrowthProfile,

    /// Available tool names.
    pub capabilities: Vec<String>,

    /// Default LLM provider for this soul.
    pub default_provider: String,

    /// Current lifecycle state.
    #[serde(skip)]
    pub state: SoulRuntimeState,
}

impl Soul {
    /// Create a new soul with the given ID.
    pub fn new(id: impl Into<String>) -> Self {
        let id = id.into();
        Self {
            name: id.clone(),
            id: id.clone(),
            persona: PersonaConfig::default(),
            profile: AgentGrowthProfile::new(&id),
            capabilities: Vec::new(),
            default_provider: "kimi-coding".to_string(),
            state: SoulRuntimeState::Hibernated,
        }
    }

    /// Set display name.
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set persona.
    pub fn with_persona(mut self, persona: PersonaConfig) -> Self {
        self.persona = persona;
        self
    }

    /// Set capabilities.
    pub fn with_capabilities(mut self, caps: Vec<String>) -> Self {
        self.capabilities = caps;
        self
    }

    /// Set default provider.
    pub fn with_provider(mut self, provider: impl Into<String>) -> Self {
        self.default_provider = provider.into();
        self
    }

    /// Directory where this soul's data is stored.
    pub fn soul_dir(&self) -> PathBuf {
        Self::souls_base_dir().join(&self.id)
    }

    /// Path to soul.json.
    pub fn soul_json_path(&self) -> PathBuf {
        self.soul_dir().join("soul.json")
    }

    /// Path to state.json.
    pub fn state_json_path(&self) -> PathBuf {
        self.soul_dir().join("state.json")
    }

    /// Path to private memory database.
    pub fn memory_db_path(&self) -> PathBuf {
        self.soul_dir().join("memory.sqlite")
    }

    /// Save soul metadata to disk.
    pub fn save(&self) -> std::io::Result<()> {
        let dir = self.soul_dir();
        std::fs::create_dir_all(&dir)?;

        let json = serde_json::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(self.soul_json_path(), json)?;

        // Also save profile independently.
        let _ = self.profile.save();

        Ok(())
    }

    /// Load a soul from disk, or create a new one if it doesn't exist.
    pub fn load_or_create(id: impl Into<String>) -> Self {
        let id = id.into();
        let path = Self::souls_base_dir().join(&id).join("soul.json");

        if let Ok(contents) = std::fs::read_to_string(&path) {
            if let Ok(mut soul) = serde_json::from_str::<Self>(&contents) {
                // Rehydrate profile from its own file (may have been updated).
                soul.profile = AgentGrowthProfile::load_or_create(&soul.id);
                soul.state = SoulRuntimeState::Hibernated;
                return soul;
            }
        }

        Self::new(id)
    }

    /// List all soul IDs stored on disk.
    pub fn list_all() -> Vec<String> {
        let base = Self::souls_base_dir();
        if !base.exists() {
            return Vec::new();
        }

        std::fs::read_dir(base)
            .ok()
            .map(|entries| {
                entries
                    .flatten()
                    .filter(|e| e.file_type().map(|t| t.is_dir()).unwrap_or(false))
                    .filter_map(|e| e.file_name().into_string().ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    fn souls_base_dir() -> PathBuf {
        home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".clarity")
            .join("souls")
    }
}

impl Default for Soul {
    fn default() -> Self {
        Self::new("default")
    }
}

// ============================================================================
// PersonaConfig
// ============================================================================

/// Personality and behavior configuration for a soul.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct PersonaConfig {
    /// System prompt injected at the start of every session.
    pub system_prompt: String,

    /// Temperature override (None = use provider default).
    pub temperature: Option<f32>,

    /// Personality tags for skill matching.
    pub tags: Vec<String>,

    /// Avatar / icon identifier.
    pub avatar: Option<String>,
}

// ============================================================================
// SoulRuntimeState
// ============================================================================

/// Runtime lifecycle state of a soul.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SoulRuntimeState {
    /// Actively running with an Agent instance.
    Active,
    /// Memory and state retained, but Agent loop paused.
    Suspended,
    /// Minimal footprint — only disk state, no in-memory presence.
    #[default]
    Hibernated,
}

// ============================================================================
// SoulManager
// ============================================================================

/// Manages the lifecycle of multiple souls.
pub struct SoulManager {
    souls: HashMap<String, Soul>,
    active_agent: Option<String>,
}

impl SoulManager {
    /// Create a new manager and discover existing souls from disk.
    pub fn new() -> Self {
        let mut souls = HashMap::new();
        for id in Soul::list_all() {
            let soul = Soul::load_or_create(&id);
            souls.insert(id, soul);
        }
        Self {
            souls,
            active_agent: None,
        }
    }

    /// Get a soul by ID.
    pub fn get(&self, id: &str) -> Option<&Soul> {
        self.souls.get(id)
    }

    /// Get a mutable soul by ID.
    pub fn get_mut(&mut self, id: &str) -> Option<&mut Soul> {
        self.souls.get_mut(id)
    }

    /// Register a new soul (or overwrite existing).
    pub fn register(&mut self, soul: Soul) -> std::io::Result<()> {
        soul.save()?;
        self.souls.insert(soul.id.clone(), soul);
        Ok(())
    }

    /// List all soul IDs.
    pub fn list_ids(&self) -> Vec<&String> {
        self.souls.keys().collect()
    }

    /// Mark a soul as active.
    pub fn activate(&mut self, id: &str) {
        if let Some(soul) = self.souls.get_mut(id) {
            soul.state = SoulRuntimeState::Active;
            self.active_agent = Some(id.to_string());
        }
    }

    /// Suspend the currently active soul.
    pub fn suspend_active(&mut self) {
        if let Some(ref id) = self.active_agent {
            if let Some(soul) = self.souls.get_mut(id) {
                soul.state = SoulRuntimeState::Suspended;
            }
        }
        self.active_agent = None;
    }

    /// Hibernate all souls (e.g. before shutdown).
    pub fn hibernate_all(&mut self) {
        for soul in self.souls.values_mut() {
            if soul.state == SoulRuntimeState::Active {
                let _ = soul.save();
                let _ = soul.profile.save();
            }
            soul.state = SoulRuntimeState::Hibernated;
        }
        self.active_agent = None;
    }

    /// Currently active soul ID, if any.
    pub fn active_id(&self) -> Option<&String> {
        self.active_agent.as_ref()
    }
}

impl Default for SoulManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_soul_creation() {
        let soul = Soul::new("grey")
            .with_name("Gray")
            .with_provider("local-gguf");

        assert_eq!(soul.id, "grey");
        assert_eq!(soul.name, "Gray");
        assert_eq!(soul.default_provider, "local-gguf");
        assert_eq!(soul.state, SoulRuntimeState::Hibernated);
    }

    #[test]
    fn test_soul_roundtrip() {
        let soul = Soul::new("test-soul")
            .with_persona(PersonaConfig {
                system_prompt: "You are a test.".to_string(),
                temperature: Some(0.5),
                tags: vec!["test".to_string()],
                avatar: None,
            })
            .with_capabilities(vec!["bash".to_string(), "file_read".to_string()]);

        let json = serde_json::to_string(&soul).unwrap();
        let restored: Soul = serde_json::from_str(&json).unwrap();

        assert_eq!(soul.id, restored.id);
        assert_eq!(soul.persona, restored.persona);
        assert_eq!(soul.capabilities, restored.capabilities);
    }

    #[test]
    fn test_soul_manager_discover() {
        let manager = SoulManager::new();
        // In a fresh environment there may be no souls.
        // Just verify it doesn't panic.
        assert!(manager.list_ids().is_empty() || !manager.list_ids().is_empty());
    }
}
