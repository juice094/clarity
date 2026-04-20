//! Skill registry — read-only, thread-safe, shared across Agent instances.

use super::{Skill, SkillResult};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// Registry of loaded skills.
///
/// Immutable after construction — skills are loaded once at startup
/// and shared across all Agents.
#[derive(Debug, Clone, Default)]
pub struct SkillRegistry {
    skills: Arc<HashMap<String, Skill>>,
}

impl SkillRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a registry from a collection of skills.
    pub fn from_skills(skills: Vec<Skill>) -> Self {
        let mut map = HashMap::with_capacity(skills.len());
        for skill in skills {
            let id = skill.meta.id.clone();
            if map.insert(id.clone(), skill).is_some() {
                tracing::warn!(
                    "Duplicate skill id '{}' encountered during registry build; last one wins",
                    id
                );
            }
        }
        Self {
            skills: Arc::new(map),
        }
    }

    /// Load all `.md` files from a directory.
    pub fn load_from_dir(dir: &Path) -> SkillResult<Self> {
        let skills = super::SkillLoader::load_dir(dir)?;
        Ok(Self::from_skills(skills))
    }

    /// Get a skill by id.
    pub fn get(&self, id: &str) -> Option<&Skill> {
        self.skills.get(id)
    }

    /// Check if a skill exists.
    pub fn contains(&self, id: &str) -> bool {
        self.skills.contains_key(id)
    }

    /// List all skill ids.
    pub fn list_ids(&self) -> Vec<&str> {
        self.skills.keys().map(|s| s.as_str()).collect()
    }

    /// List summaries for all skills.
    pub fn list_summaries(&self) -> Vec<String> {
        self.skills.values().map(|s| s.summary()).collect()
    }

    /// Find skills whose id, name, description, or tags contain the query (case-insensitive).
    pub fn find_relevant(&self, query: &str) -> Vec<&Skill> {
        let q = query.to_lowercase();
        self.skills
            .values()
            .filter(|s| {
                s.meta.id.to_lowercase().contains(&q)
                    || s.meta.name.to_lowercase().contains(&q)
                    || s.meta.description.to_lowercase().contains(&q)
                    || s.meta.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .collect()
    }

    /// Build the context string for a specific skill.
    /// Returns `None` if the skill is not found.
    pub fn build_context(&self, skill_id: &str) -> Option<String> {
        self.get(skill_id).map(|s| s.build_context())
    }

    /// Return the number of registered skills.
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::{SkillMeta};

    fn make_skill(id: &str, name: &str, desc: &str, tags: &[&str]) -> Skill {
        Skill {
            meta: SkillMeta {
                id: id.to_string(),
                name: name.to_string(),
                version: "1.0.0".to_string(),
                description: desc.to_string(),
                tools: vec![],
                tags: tags.iter().map(|&s| s.to_string()).collect(),
            },
            body: format!("# {}\nBody.", name),
        }
    }

    #[test]
    fn test_registry_lookup() {
        let reg = SkillRegistry::from_skills(vec![
            make_skill("deploy", "Deploy", "Deploy stuff", &["ops"]),
            make_skill("review", "Review", "Code review", &["dev"]),
        ]);

        assert!(reg.contains("deploy"));
        assert!(!reg.contains("missing"));
        assert_eq!(reg.len(), 2);

        let skill = reg.get("review").unwrap();
        assert_eq!(skill.meta.name, "Review");
    }

    #[test]
    fn test_registry_find_relevant() {
        let reg = SkillRegistry::from_skills(vec![
            make_skill("deploy", "Deploy", "Deploy rust services", &["ops", "rust"]),
            make_skill("review", "Review", "Review code changes", &["dev"]),
        ]);

        let results = reg.find_relevant("rust");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].meta.id, "deploy");

        let results = reg.find_relevant("review");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].meta.id, "review");
    }

    #[test]
    fn test_registry_build_context() {
        let reg = SkillRegistry::from_skills(vec![make_skill(
            "test",
            "Test",
            "Testing",
            &[],
        )]);
        let ctx = reg.build_context("test").unwrap();
        assert!(ctx.contains("Test"));
        assert!(ctx.contains("Body."));
    }

    #[test]
    fn test_registry_empty() {
        let reg = SkillRegistry::new();
        assert!(reg.is_empty());
        assert!(reg.build_context("anything").is_none());
    }
}
