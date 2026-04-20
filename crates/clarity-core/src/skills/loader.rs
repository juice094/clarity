//! SKILL.md file loader.
//!
//! Parses Markdown files with YAML frontmatter delimited by `---`.

use super::{Skill, SkillError, SkillMeta, SkillResult};
use std::path::Path;

/// Loads skills from the filesystem.
#[derive(Debug, Clone, Default)]
pub struct SkillLoader;

impl SkillLoader {
    /// Load a single SKILL.md from a file path.
    pub fn load_file(path: &Path) -> SkillResult<Skill> {
        let content = std::fs::read_to_string(path)?;
        Self::parse(&content)
    }

    /// Load all `.md` files from a directory (non-recursive).
    pub fn load_dir(dir: &Path) -> SkillResult<Vec<Skill>> {
        let mut skills = Vec::new();
        if !dir.is_dir() {
            return Ok(skills);
        }
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("md") {
                match Self::load_file(&path) {
                    Ok(skill) => skills.push(skill),
                    Err(e) => {
                        tracing::warn!(
                            "Failed to load skill from {}: {}",
                            path.display(),
                            e
                        );
                    }
                }
            }
        }
        Ok(skills)
    }

    /// Parse a SKILL.md string.
    ///
    /// Expects YAML frontmatter delimited by `---` at the top:
    ///
    /// ```markdown
    /// ---
    /// id: example
    /// name: Example Skill
    /// ---
    ///
    /// # Body
    /// ```
    pub fn parse(content: &str) -> SkillResult<Skill> {
        let trimmed = content.trim_start();

        // Must start with `---`
        if !trimmed.starts_with("---") {
            return Err(SkillError::InvalidFrontmatter(
                "Skill file must start with YAML frontmatter delimited by '---'".to_string(),
            ));
        }

        // Find the closing `---`
        let after_open = &trimmed[3..]; // skip first "---"
        let Some(close_idx) = after_open.find("\n---") else {
            return Err(SkillError::InvalidFrontmatter(
                "Missing closing '---' for YAML frontmatter".to_string(),
            ));
        };

        let yaml_str = &after_open[..close_idx];
        let body = &after_open[close_idx + 4..]; // skip "\n---"
        let body = body.trim_start();

        let meta: SkillMeta = serde_yaml::from_str(yaml_str)?;

        if meta.id.is_empty() {
            return Err(SkillError::MissingField("id".to_string()));
        }
        if meta.name.is_empty() {
            return Err(SkillError::MissingField("name".to_string()));
        }

        Ok(Skill {
            meta,
            body: body.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid() {
        let content = r#"---
id: deploy-rust
name: Deploy Rust
version: "1.0.0"
description: Deploy a Rust service safely
tools:
  - git_status
  - shell_build
tags:
  - deploy
---

## Step 1
Build the project.
"#;
        let skill = SkillLoader::parse(content).unwrap();
        assert_eq!(skill.meta.id, "deploy-rust");
        assert_eq!(skill.meta.name, "Deploy Rust");
        assert_eq!(skill.meta.version, "1.0.0");
        assert_eq!(skill.meta.tools, vec!["git_status", "shell_build"]);
        assert!(skill.body.contains("Step 1"));
    }

    #[test]
    fn test_parse_missing_frontmatter() {
        let content = "# Just markdown\nNo frontmatter.";
        assert!(SkillLoader::parse(content).is_err());
    }

    #[test]
    fn test_parse_missing_id() {
        let content = r#"---
name: "No ID"
---

body"#;
        let result = SkillLoader::parse(content);
        assert!(
            matches!(result, Err(SkillError::MissingField(_))),
            "Expected MissingField error, got: {:?}",
            result
        );
    }

    #[test]
    fn test_parse_minimal() {
        let content = r#"---
id: minimal
name: Minimal Skill
description: ""
---
"#;
        let skill = SkillLoader::parse(content).unwrap();
        assert_eq!(skill.meta.id, "minimal");
        assert!(skill.body.is_empty());
    }
}
