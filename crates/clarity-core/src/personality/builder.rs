//! System Prompt Builder
//!
//! Builds complete system prompts from personality and optional context:
//! - Memory (conversation history summary)
//! - User profile
//! - Skills definitions

use super::types::Personality;

/// Builder for constructing system prompts
#[derive(Debug, Clone)]
pub struct SystemPromptBuilder {
    personality: Personality,
    memory: Option<String>,
    user_profile: Option<String>,
    skills: Vec<String>,
    additional_context: Option<String>,
}

impl SystemPromptBuilder {
    /// Create a new builder with the given personality
    pub fn new(personality: Personality) -> Self {
        Self {
            personality,
            memory: None,
            user_profile: None,
            skills: Vec::new(),
            additional_context: None,
        }
    }

    /// Add memory context
    pub fn with_memory(mut self, memory: impl Into<String>) -> Self {
        self.memory = Some(memory.into());
        self
    }

    /// Add user profile
    pub fn with_user_profile(mut self, profile: impl Into<String>) -> Self {
        self.user_profile = Some(profile.into());
        self
    }

    /// Add skills definitions
    pub fn with_skills(mut self, skills: Vec<String>) -> Self {
        self.skills = skills;
        self
    }

    /// Add a single skill
    pub fn add_skill(mut self, skill: impl Into<String>) -> Self {
        self.skills.push(skill.into());
        self
    }

    /// Add additional context
    pub fn with_additional_context(mut self, context: impl Into<String>) -> Self {
        self.additional_context = Some(context.into());
        self
    }

    /// Build the complete system prompt
    ///
    /// Format:
    /// ```markdown
    /// # 人格
    /// {identity}
    /// {yuan}
    /// {ishiki}
    /// ---
    /// # 用户档案
    /// {user_profile}
    /// ---
    /// # 记忆
    /// {memory}
    /// ---
    /// # 技能
    /// {skills}
    /// ---
    /// # 额外上下文
    /// {additional_context}
    /// ```
    pub fn build(self) -> String {
        let mut sections = Vec::new();

        // Section 1: Personality (核心人格)
        let personality_section = self.build_personality_section();
        sections.push(("人格", personality_section));

        // Section 2: User Profile
        if let Some(profile) = self.user_profile {
            if !profile.trim().is_empty() {
                sections.push(("用户档案", profile));
            }
        }

        // Section 3: Memory
        if let Some(memory) = self.memory {
            if !memory.trim().is_empty() {
                sections.push(("记忆", memory));
            }
        }

        // Section 4: Skills
        if !self.skills.is_empty() {
            let skills_content = self.skills.join("\n\n");
            sections.push(("技能", skills_content));
        }

        // Section 5: Additional Context
        if let Some(context) = self.additional_context {
            if !context.trim().is_empty() {
                sections.push(("额外上下文", context));
            }
        }

        // Combine all sections
        let mut result = String::new();
        for (i, (title, content)) in sections.iter().enumerate() {
            if i > 0 {
                result.push_str("\n---\n\n");
            }
            result.push_str(&format!("# {}\n\n{}", title, content.trim()));
        }

        result
    }

    /// Build the personality section
    fn build_personality_section(&self) -> String {
        let mut parts = Vec::new();

        // Identity
        if !self.personality.identity.trim().is_empty() {
            parts.push(self.personality.identity.trim().to_string());
        }

        // Yuan
        if !self.personality.yuan.trim().is_empty() {
            parts.push(self.personality.yuan.trim().to_string());
        }

        // Ishiki
        if !self.personality.ishiki.trim().is_empty() {
            parts.push(self.personality.ishiki.trim().to_string());
        }

        parts.join("\n\n")
    }

    /// Build a simplified system prompt (personality only)
    pub fn build_simple(&self) -> String {
        self.build_personality_section()
    }

    /// Get the personality reference
    pub fn personality(&self) -> &Personality {
        &self.personality
    }

    /// Update the personality
    pub fn set_personality(&mut self, personality: Personality) {
        self.personality = personality;
    }
}

/// Preset builders for common use cases
pub mod presets {
    use super::*;

    /// Create a minimal system prompt with just personality
    pub fn minimal(personality: Personality) -> String {
        SystemPromptBuilder::new(personality).build_simple()
    }

    /// Create a system prompt with personality and skills
    pub fn with_skills(personality: Personality, skills: Vec<String>) -> String {
        SystemPromptBuilder::new(personality)
            .with_skills(skills)
            .build()
    }

    /// Create a full system prompt with all components
    pub fn full(
        personality: Personality,
        memory: String,
        user_profile: String,
        skills: Vec<String>,
    ) -> String {
        SystemPromptBuilder::new(personality)
            .with_memory(memory)
            .with_user_profile(user_profile)
            .with_skills(skills)
            .build()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_personality() -> Personality {
        Personality {
            identity: "# TestAgent\n\nYour personal assistant.".to_string(),
            yuan: "## MOOD\n\nBe thoughtful.".to_string(),
            ishiki: "- Be helpful\n- Be kind".to_string(),
        }
    }

    #[test]
    fn test_build_simple() {
        let personality = test_personality();
        let builder = SystemPromptBuilder::new(personality);
        let prompt = builder.build_simple();

        assert!(prompt.contains("TestAgent"));
        assert!(prompt.contains("MOOD"));
        assert!(prompt.contains("Be helpful"));
    }

    #[test]
    fn test_build_with_sections() {
        let personality = test_personality();
        let prompt = SystemPromptBuilder::new(personality.clone())
            .with_user_profile("User likes Rust")
            .with_memory("Previous conversation about programming")
            .with_skills(vec!["Code generation".to_string()])
            .build();

        assert!(prompt.contains("# 人格"));
        assert!(prompt.contains("# 用户档案"));
        assert!(prompt.contains("User likes Rust"));
        assert!(prompt.contains("# 记忆"));
        assert!(prompt.contains("Previous conversation"));
        assert!(prompt.contains("# 技能"));
        assert!(prompt.contains("Code generation"));
        assert!(prompt.contains("---"));
    }

    #[test]
    fn test_build_empty_skills_ignored() {
        let personality = test_personality();
        let prompt = SystemPromptBuilder::new(personality)
            .with_skills(vec![])
            .build();

        assert!(!prompt.contains("# 技能"));
    }

    #[test]
    fn test_preset_minimal() {
        let personality = test_personality();
        let prompt = presets::minimal(personality);

        assert!(prompt.contains("TestAgent"));
        assert!(!prompt.contains("# 人格"));
    }

    #[test]
    fn test_preset_full() {
        let personality = test_personality();
        let prompt = presets::full(
            personality,
            "Memory content".to_string(),
            "Profile content".to_string(),
            vec!["Skill 1".to_string()],
        );

        assert!(prompt.contains("# 人格"));
        assert!(prompt.contains("# 用户档案"));
        assert!(prompt.contains("Profile content"));
        assert!(prompt.contains("# 记忆"));
        assert!(prompt.contains("Memory content"));
        assert!(prompt.contains("# 技能"));
        assert!(prompt.contains("Skill 1"));
    }
}
