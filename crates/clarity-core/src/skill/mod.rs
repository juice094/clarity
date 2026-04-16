//! # Skill System
//!
//! The skill system provides a modular way to extend Clarity's capabilities
//! with specialized functionality like task management, reasoning, and more.
//!
//! ## Example
//!
//! ```rust
//! use clarity_core::skill::{Skill, SkillRegistry, TodoSkill, ThinkSkill};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create a registry and register skills
//! let mut registry = SkillRegistry::new();
//! registry.register(TodoSkill::new())?;
//! registry.register(ThinkSkill::new())?;
//!
//! // Execute a skill
//! if let Some(skill) = registry.get("todo") {
//!     let result = skill.execute("add Buy milk").await?;
//!     println!("{}", result);
//! }
//! # Ok(())
//! # }
//! ```

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Errors that can occur during skill execution
#[derive(Error, Debug, Clone)]
pub enum SkillError {
    /// Invalid input provided to the skill
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    /// Skill execution failed
    #[error("Execution failed: {0}")]
    ExecutionFailed(String),

    /// Skill not found in registry
    #[error("Skill not found: {0}")]
    NotFound(String),

    /// Duplicate skill registration
    #[error("Duplicate skill: {0}")]
    Duplicate(String),

    /// I/O error during execution
    #[error("I/O error: {0}")]
    IoError(String),

    /// Serialization error
    #[error("Serialization error: {0}")]
    SerializationError(String),
}

impl SkillError {
    /// Create an invalid input error
    pub fn invalid_input<S: Into<String>>(msg: S) -> Self {
        Self::InvalidInput(msg.into())
    }

    /// Create an execution failed error
    pub fn execution_failed<S: Into<String>>(msg: S) -> Self {
        Self::ExecutionFailed(msg.into())
    }

    /// Create a not found error
    pub fn not_found<S: Into<String>>(name: S) -> Self {
        Self::NotFound(name.into())
    }

    /// Create a duplicate skill error
    pub fn duplicate<S: Into<String>>(name: S) -> Self {
        Self::Duplicate(name.into())
    }

    /// Create an I/O error from std::io::Error
    pub fn from_io(err: std::io::Error) -> Self {
        Self::IoError(err.to_string())
    }
}

/// Result type for skill operations
pub type SkillResult<T> = Result<T, SkillError>;

/// Core trait for all skills
///
/// Skills are modular capabilities that can be registered and executed
/// by the Clarity agent system.
#[async_trait]
pub trait Skill: Send + Sync {
    /// Returns the unique name of the skill
    fn name(&self) -> &str;

    /// Returns a human-readable description of what the skill does
    fn description(&self) -> &str;

    /// Execute the skill with the given input
    ///
    /// # Arguments
    ///
    /// * `input` - The input string containing commands and arguments
    ///
    /// # Returns
    ///
    /// Returns the result of the execution as a string, or a SkillError
    async fn execute(&self, input: &str) -> SkillResult<String>;
}

/// A type-erased boxed skill for storage in collections
pub type BoxedSkill = Arc<dyn Skill>;

/// Registry for managing and accessing skills
///
/// The SkillRegistry maintains a collection of registered skills and
/// provides methods to register, retrieve, and list them.
#[derive(Default)]
pub struct SkillRegistry {
    skills: HashMap<String, BoxedSkill>,
}

impl std::fmt::Debug for SkillRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SkillRegistry")
            .field("skills", &self.skills.keys().collect::<Vec<_>>())
            .field("count", &self.skills.len())
            .finish()
    }
}

impl SkillRegistry {
    /// Create a new empty skill registry
    pub fn new() -> Self {
        Self {
            skills: HashMap::new(),
        }
    }

    /// Register a new skill
    ///
    /// # Arguments
    ///
    /// * `skill` - The skill to register
    ///
    /// # Errors
    ///
    /// Returns `SkillError::Duplicate` if a skill with the same name is already registered
    pub fn register<S>(&mut self, skill: S) -> SkillResult<()>
    where
        S: Skill + 'static,
    {
        let name = skill.name().to_string();
        if self.skills.contains_key(&name) {
            return Err(SkillError::duplicate(format!(
                "Skill '{}' is already registered",
                name
            )));
        }
        self.skills.insert(name, Arc::new(skill));
        Ok(())
    }

    /// Unregister a skill by name
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the skill to unregister
    ///
    /// # Returns
    ///
    /// Returns `true` if the skill was found and removed, `false` otherwise
    pub fn unregister(&mut self, name: &str) -> bool {
        self.skills.remove(name).is_some()
    }

    /// Get a skill by name
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the skill to retrieve
    ///
    /// # Returns
    ///
    /// Returns `Some(skill)` if found, `None` otherwise
    pub fn get(&self, name: &str) -> Option<BoxedSkill> {
        self.skills.get(name).cloned()
    }

    /// Check if a skill is registered
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the skill to check
    pub fn contains(&self, name: &str) -> bool {
        self.skills.contains_key(name)
    }

    /// List all registered skill names
    pub fn list(&self) -> Vec<String> {
        self.skills.keys().cloned().collect()
    }

    /// Get the number of registered skills
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Get all skills with their descriptions
    pub fn get_all_descriptions(&self) -> Vec<(String, String)> {
        self.skills
            .values()
            .map(|skill| (skill.name().to_string(), skill.description().to_string()))
            .collect()
    }

    /// Execute a skill by name with the given input
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the skill to execute
    /// * `input` - The input to pass to the skill
    ///
    /// # Errors
    ///
    /// Returns `SkillError::NotFound` if the skill doesn't exist
    pub async fn execute(&self, name: &str, input: &str) -> SkillResult<String> {
        match self.get(name) {
            Some(skill) => skill.execute(input).await,
            None => Err(SkillError::not_found(name)),
        }
    }

    /// Clear all registered skills
    pub fn clear(&mut self) {
        self.skills.clear();
    }
}

// Submodules
mod think;
mod todo;

// Re-export skill implementations
pub use think::ThinkSkill;
pub use todo::TodoSkill;

#[cfg(test)]
mod tests {
    use super::*;

    // Test skill for unit testing
    struct TestSkill {
        name: String,
        description: String,
    }

    impl TestSkill {
        fn new(name: &str, description: &str) -> Self {
            Self {
                name: name.to_string(),
                description: description.to_string(),
            }
        }
    }

    #[async_trait]
    impl Skill for TestSkill {
        fn name(&self) -> &str {
            &self.name
        }

        fn description(&self) -> &str {
            &self.description
        }

        async fn execute(&self, input: &str) -> SkillResult<String> {
            Ok(format!("Executed {} with: {}", self.name, input))
        }
    }

    #[tokio::test]
    async fn test_skill_trait() {
        let skill = TestSkill::new("test", "A test skill");
        assert_eq!(skill.name(), "test");
        assert_eq!(skill.description(), "A test skill");

        let result = skill.execute("hello").await.unwrap();
        assert_eq!(result, "Executed test with: hello");
    }

    #[test]
    fn test_skill_registry_new() {
        let registry = SkillRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_skill_registry_register() {
        let mut registry = SkillRegistry::new();
        let skill = TestSkill::new("test", "A test skill");

        assert!(registry.register(skill).is_ok());
        assert_eq!(registry.len(), 1);
        assert!(registry.contains("test"));
    }

    #[test]
    fn test_skill_registry_duplicate() {
        let mut registry = SkillRegistry::new();
        let skill1 = TestSkill::new("test", "First skill");
        let skill2 = TestSkill::new("test", "Duplicate skill");

        assert!(registry.register(skill1).is_ok());
        let result = registry.register(skill2);

        assert!(matches!(result, Err(SkillError::Duplicate(_))));
    }

    #[test]
    fn test_skill_registry_get() {
        let mut registry = SkillRegistry::new();
        let skill = TestSkill::new("test", "A test skill");

        registry.register(skill).unwrap();

        let retrieved = registry.get("test");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name(), "test");

        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn test_skill_registry_list() {
        let mut registry = SkillRegistry::new();
        registry
            .register(TestSkill::new("skill1", "First"))
            .unwrap();
        registry
            .register(TestSkill::new("skill2", "Second"))
            .unwrap();

        let list = registry.list();
        assert_eq!(list.len(), 2);
        assert!(list.contains(&"skill1".to_string()));
        assert!(list.contains(&"skill2".to_string()));
    }

    #[test]
    fn test_skill_registry_descriptions() {
        let mut registry = SkillRegistry::new();
        registry
            .register(TestSkill::new("skill1", "First skill"))
            .unwrap();
        registry
            .register(TestSkill::new("skill2", "Second skill"))
            .unwrap();

        let descs = registry.get_all_descriptions();
        assert_eq!(descs.len(), 2);

        let map: std::collections::HashMap<_, _> = descs.into_iter().collect();
        assert_eq!(map.get("skill1").unwrap(), "First skill");
        assert_eq!(map.get("skill2").unwrap(), "Second skill");
    }

    #[tokio::test]
    async fn test_skill_registry_execute() {
        let mut registry = SkillRegistry::new();
        registry
            .register(TestSkill::new("test", "A test skill"))
            .unwrap();

        let result = registry.execute("test", "input").await.unwrap();
        assert_eq!(result, "Executed test with: input");

        let err = registry.execute("nonexistent", "input").await;
        assert!(matches!(err, Err(SkillError::NotFound(_))));
    }

    #[test]
    fn test_skill_registry_unregister() {
        let mut registry = SkillRegistry::new();
        registry
            .register(TestSkill::new("test", "A test skill"))
            .unwrap();

        assert!(registry.unregister("test"));
        assert!(!registry.contains("test"));
        assert!(!registry.unregister("test"));
    }

    #[test]
    fn test_skill_registry_clear() {
        let mut registry = SkillRegistry::new();
        registry
            .register(TestSkill::new("skill1", "First"))
            .unwrap();
        registry
            .register(TestSkill::new("skill2", "Second"))
            .unwrap();

        registry.clear();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    fn test_skill_error_variants() {
        let err1 = SkillError::invalid_input("bad input");
        assert!(matches!(err1, SkillError::InvalidInput(_)));
        assert!(err1.to_string().contains("Invalid input"));

        let err2 = SkillError::execution_failed("something went wrong");
        assert!(matches!(err2, SkillError::ExecutionFailed(_)));

        let err3 = SkillError::not_found("missing");
        assert!(matches!(err3, SkillError::NotFound(_)));

        let err4 = SkillError::duplicate("dup");
        assert!(matches!(err4, SkillError::Duplicate(_)));

        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err5 = SkillError::from_io(io_err);
        assert!(matches!(err5, SkillError::IoError(_)));
    }
}
