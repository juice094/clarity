//! # Skill System — Orchestration Layer
//!
//! Skills are **workflow knowledge packages** that teach the Agent how to
//! use Tools effectively to accomplish complex, multi-step tasks.
//!
//! They are **not** executable code. Instead, they are Markdown documents
//! with YAML frontmatter that provide:
//! - Structured instructions for the LLM
//! - Tool whitelist (least-privilege)
//! - Error handling paths
//! - Expected output formats
//!
//! ## File Format: SKILL.md
//!
//! ```markdown
//! ---
//! id: deploy-rust-service
//! name: Deploy Rust Service
//! version: "1.0.0"
//! description: Safe deployment workflow for Rust services
//! tools:
//!   - git_status
//!   - shell_build
//!   - shell_deploy
//! tags: [deploy, rust, production]
//! ---
//!
//! ## Prerequisites
//! ...
//!
//! ## Steps
//! ...
//! ```
//!
//! ## Relationship to other layers
//!
//! - **Skill** (this module) → orchestrates **Tool** (`crate::tools`)
//! - **Skill** does **not** directly interact with **MCP**; it works through ToolRegistry
//! - **Plugin** (`claw`/`cli`/`window`) binds Skills to specific entry points

pub mod discovery;
mod loader;
mod registry;

pub use discovery::SkillDiscovery;
pub use loader::SkillLoader;
pub use registry::SkillRegistry;

/// Error type for Skill operations.
#[derive(Debug, thiserror::Error)]
pub enum SkillError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Invalid frontmatter: {0}")]
    InvalidFrontmatter(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Skill not found: {0}")]
    NotFound(String),
}

/// Result type for Skill operations.
pub type SkillResult<T> = Result<T, SkillError>;

/// YAML frontmatter metadata for a Skill.
#[derive(Debug, Clone, Default, serde::Deserialize, serde::Serialize)]
pub struct SkillMeta {
    /// Unique identifier (kebab-case recommended)
    #[serde(default)]
    pub id: String,

    /// Human-readable name
    #[serde(default)]
    pub name: String,

    /// Semantic version
    #[serde(default)]
    pub version: String,

    /// One-line description
    #[serde(default)]
    pub description: String,

    /// Tool whitelist — only these tools are exposed when this skill is active
    #[serde(default)]
    pub tools: Vec<String>,

    /// Tags for discovery and categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// File path patterns that trigger activation of this skill (gitignore-style globs)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paths: Option<Vec<String>>,
}

/// A loaded skill template.
#[derive(Debug, Clone)]
pub struct Skill {
    pub meta: SkillMeta,
    /// Markdown body (everything after the frontmatter)
    pub body: String,
}

impl Skill {
    /// Build the full context string to inject into the system prompt.
    ///
    /// Includes metadata header + the full Markdown body.
    pub fn build_context(&self) -> String {
        let tags = if self.meta.tags.is_empty() {
            String::new()
        } else {
            format!(" (tags: {})", self.meta.tags.join(", "))
        };

        format!(
            r#"# Skill: {name}{tags}

{description}

Allowed tools: {tools}

---

{body}"#,
            name = self.meta.name,
            description = self.meta.description,
            tools = self.meta.tools.join(", "),
            body = self.body,
        )
    }

    /// Build a concise one-line summary for listing.
    pub fn summary(&self) -> String {
        format!("{} — {}", self.meta.id, self.meta.description)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_meta() -> SkillMeta {
        SkillMeta {
            id: "test-skill".to_string(),
            name: "Test Skill".to_string(),
            version: "1.0.0".to_string(),
            description: "A test skill for unit testing".to_string(),
            tools: vec!["tool_a".to_string(), "tool_b".to_string()],
            tags: vec!["test".to_string()],
            paths: None,
        }
    }

    #[test]
    fn test_skill_build_context() {
        let skill = Skill {
            meta: sample_meta(),
            body: "## Step 1\nDo something.".to_string(),
        };
        let ctx = skill.build_context();
        assert!(ctx.contains("Test Skill"));
        assert!(ctx.contains("tool_a"));
        assert!(ctx.contains("Step 1"));
    }

    #[test]
    fn test_skill_summary() {
        let skill = Skill {
            meta: sample_meta(),
            body: String::new(),
        };
        assert_eq!(
            skill.summary(),
            "test-skill — A test skill for unit testing"
        );
    }
}
