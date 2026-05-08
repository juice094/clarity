//! Skill registry — thread-safe, supports runtime discovery and activation.

use super::{Skill, SkillDiscovery, SkillResult};
use crate::error::AgentError;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;

/// Registry of loaded skills.
///
/// Backed by `Arc<RwLock<…>>` so skills can be discovered and inserted
/// concurrently at runtime while remaining cheap to clone.
#[derive(Debug, Clone, Default)]
pub struct SkillRegistry {
    skills: Arc<parking_lot::RwLock<HashMap<String, Skill>>>,
    active: Arc<parking_lot::RwLock<HashSet<String>>>,
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
            skills: Arc::new(parking_lot::RwLock::new(map)),
            active: Arc::new(parking_lot::RwLock::new(HashSet::new())),
        }
    }

    /// Load all `.md` files from a directory.
    pub fn load_from_dir(dir: &Path) -> SkillResult<Self> {
        let skills = super::SkillLoader::load_dir(dir)?;
        Ok(Self::from_skills(skills))
    }

    /// Get a skill by id.
    pub fn get(&self, id: &str) -> Option<Skill> {
        self.skills.read().get(id).cloned()
    }

    /// Check if a skill exists.
    pub fn contains(&self, id: &str) -> bool {
        self.skills.read().contains_key(id)
    }

    /// List all skill ids.
    pub fn list_ids(&self) -> Vec<String> {
        self.skills.read().keys().cloned().collect()
    }

    /// List summaries for all skills.
    pub fn list_summaries(&self) -> Vec<String> {
        self.skills
            .read()
            .values()
            .map(|s| s.summary())
            .collect()
    }

    /// Find skills whose id, name, description, or tags contain the query (case-insensitive).
    pub fn find_relevant(&self, query: &str) -> Vec<Skill> {
        let q = query.to_lowercase();
        self.skills
            .read()
            .values()
            .filter(|s| {
                s.meta.id.to_lowercase().contains(&q)
                    || s.meta.name.to_lowercase().contains(&q)
                    || s.meta.description.to_lowercase().contains(&q)
                    || s.meta.tags.iter().any(|t| t.to_lowercase().contains(&q))
            })
            .cloned()
            .collect()
    }

    /// Build the context string for a specific skill.
    /// Returns `None` if the skill is not found.
    pub fn build_context(&self, skill_id: &str) -> Option<String> {
        self.get(skill_id).map(|s| s.build_context())
    }

    /// Return the number of registered skills.
    pub fn len(&self) -> usize {
        self.skills.read().len()
    }

    /// Check if the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.skills.read().is_empty()
    }

    /// Discover skills by walking from `working_dir` up to the root,
    /// looking for `.clarity/skills/` and `.claude/skills/` directories.
    ///
    /// Newly discovered skills are inserted into the registry.
    /// Returns the ids of skills that were actually added (i.e. not already present).
    pub fn discover_for_path(&self, working_dir: &Path) -> Vec<String> {
        let mut current = Some(working_dir);
        let mut all_discovered = Vec::new();

        while let Some(dir) = current {
            all_discovered.extend(SkillDiscovery::scan_project_skills(dir));
            current = dir.parent();
        }

        let mut map = self.skills.write();
        let mut new_ids = Vec::new();
        for skill in all_discovered {
            let id = skill.meta.id.clone();
            if !map.contains_key(&id) {
                map.insert(id.clone(), skill);
                new_ids.push(id);
            }
        }
        new_ids
    }

    /// Activate skills whose `paths` frontmatter matches any of the given file paths.
    ///
    /// Matching skills are marked as active and their ids are returned.
    pub fn activate_by_path(&self, paths: &[std::path::PathBuf]) -> Vec<String> {
        let map = self.skills.read();
        let mut activated = Vec::new();

        for (id, skill) in map.iter() {
            if let Some(ref patterns) = skill.meta.paths {
                'pattern_loop: for pattern in patterns {
                    for path in paths {
                        if super::discovery::path_matches_pattern(path, pattern) {
                            activated.push(id.clone());
                            break 'pattern_loop;
                        }
                    }
                }
            }
        }

        drop(map);
        let mut active = self.active.write();
        for id in &activated {
            active.insert(id.clone());
        }

        activated
    }

    /// Check whether a skill is currently active.
    pub fn is_active(&self, id: &str) -> bool {
        self.active.read().contains(id)
    }

    /// Return a clone of the active skill ids set.
    pub fn active_ids(&self) -> HashSet<String> {
        self.active.read().clone()
    }

    /// Deactivate a skill by id.
    /// Returns `true` if the skill was active and is now deactivated.
    pub fn deactivate(&self, id: &str) -> bool {
        self.active.write().remove(id)
    }

    /// Toggle the active state of a skill.
    /// Returns the new active state.
    pub fn toggle_active(&self, id: &str) -> bool {
        let mut active = self.active.write();
        if active.contains(id) {
            active.remove(id);
            false
        } else {
            active.insert(id.to_string());
            true
        }
    }

    /// List all registered skills.
    pub fn list_skills(&self) -> Vec<super::Skill> {
        self.skills.read().values().cloned().collect()
    }

    /// Run a flow-type skill.
    ///
    /// Returns the final response from the flow execution.
    pub async fn run_flow(
        &self,
        agent: &crate::agent::Agent,
        skill_id: &str,
        args: &str,
    ) -> Result<String, AgentError> {
        let skill = self
            .get(skill_id)
            .ok_or_else(|| AgentError::registry(format!("Skill '{}' not found", skill_id)))?;

        if skill.meta.skill_type != "flow" {
            return Err(AgentError::registry(format!(
                "Skill '{}' is not a flow-type skill",
                skill_id
            )));
        }

        let flow = skill.flow.as_ref().ok_or_else(|| {
            AgentError::registry(format!("Skill '{}' has no parsed flow", skill_id))
        })?;

        agent.run_flow(flow, args).await
    }
}

#[cfg(test)]
mod tests {
    use super::super::SkillMeta;
    use super::*;

    fn make_skill(id: &str, name: &str, desc: &str, tags: &[&str]) -> Skill {
        Skill {
            meta: SkillMeta {
                id: id.to_string(),
                name: name.to_string(),
                version: "1.0.0".to_string(),
                description: desc.to_string(),
                tools: vec![],
                tags: tags.iter().map(|&s| s.to_string()).collect(),
                paths: None,
                skill_type: "standard".to_string(),
            },
            body: format!("# {}\nBody.", name),
            flow: None,
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
        let reg = SkillRegistry::from_skills(vec![make_skill("test", "Test", "Testing", &[])]);
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

    #[test]
    fn test_discover_for_path() {
        let tmp = tempfile::tempdir().unwrap();
        let skills_dir = tmp.path().join(".clarity").join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        std::fs::write(
            skills_dir.join("project.md"),
            "---\nid: project-skill\nname: Project Skill\n---\n\nBody.\n",
        )
        .unwrap();

        let reg = SkillRegistry::new();
        let ids = reg.discover_for_path(tmp.path());
        assert_eq!(ids, vec!["project-skill"]);
        assert!(reg.contains("project-skill"));
    }

    #[test]
    fn test_discover_for_path_ignores_duplicates() {
        let tmp = tempfile::tempdir().unwrap();
        let skills_dir = tmp.path().join(".clarity").join("skills");
        std::fs::create_dir_all(&skills_dir).unwrap();

        std::fs::write(
            skills_dir.join("dup.md"),
            "---\nid: dup\nname: Dup\n---\n\nBody.\n",
        )
        .unwrap();

        let reg = SkillRegistry::new();
        reg.discover_for_path(tmp.path());
        let ids = reg.discover_for_path(tmp.path());
        assert!(ids.is_empty());
    }

    #[test]
    fn test_discover_for_path_walks_upwards() {
        let tmp = tempfile::tempdir().unwrap();
        let root_skills = tmp.path().join(".claude").join("skills");
        std::fs::create_dir_all(&root_skills).unwrap();

        std::fs::write(
            root_skills.join("root.md"),
            "---\nid: root-skill\nname: Root Skill\n---\n\nBody.\n",
        )
        .unwrap();

        let nested = tmp.path().join("a").join("b").join("c");
        std::fs::create_dir_all(&nested).unwrap();

        let reg = SkillRegistry::new();
        let ids = reg.discover_for_path(&nested);
        assert!(ids.contains(&"root-skill".to_string()));
    }

    #[test]
    fn test_activate_by_path() {
        let reg = SkillRegistry::from_skills(vec![Skill {
            meta: SkillMeta {
                id: "rust".to_string(),
                name: "Rust".to_string(),
                version: "1.0.0".to_string(),
                description: "Rust skill".to_string(),
                tools: vec![],
                tags: vec![],
                paths: Some(vec!["*.rs".to_string(), "Cargo.toml".to_string()]),
                skill_type: "standard".to_string(),
            },
            body: "Body.".to_string(),
            flow: None,
        }]);

        let activated = reg.activate_by_path(&[
            std::path::PathBuf::from("src/main.rs"),
            std::path::PathBuf::from("README.md"),
        ]);

        assert_eq!(activated, vec!["rust"]);
        assert!(reg.is_active("rust"));
    }

    #[test]
    fn test_activate_by_path_no_match() {
        let reg = SkillRegistry::from_skills(vec![Skill {
            meta: SkillMeta {
                id: "rust".to_string(),
                name: "Rust".to_string(),
                version: "1.0.0".to_string(),
                description: "Rust skill".to_string(),
                tools: vec![],
                tags: vec![],
                paths: Some(vec!["*.rs".to_string()]),
                skill_type: "standard".to_string(),
            },
            body: "Body.".to_string(),
            flow: None,
        }]);

        let activated = reg.activate_by_path(&[std::path::PathBuf::from("README.md")]);
        assert!(activated.is_empty());
        assert!(!reg.is_active("rust"));
    }

    #[test]
    fn test_activate_by_path_skips_skills_without_paths() {
        let reg = SkillRegistry::from_skills(vec![make_skill("always", "Always", "Always", &[])]);
        let activated = reg.activate_by_path(&[std::path::PathBuf::from("foo.rs")]);
        assert!(activated.is_empty());
    }
}
