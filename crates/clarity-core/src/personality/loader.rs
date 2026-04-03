//! Personality Loader
//!
//! Loads personality templates with fallback priority:
//! 1. Agent directory custom templates (user-defined)
//! 2. Locale-specific templates
//! 3. Default templates
//! 4. Embedded default templates (fallback)

use super::types::{Personality, PersonalityConfig, YuanType};
use std::path::{Path, PathBuf};
use tracing::{debug, info};

/// Loader for personality templates
#[derive(Debug, Clone)]
pub struct PersonalityLoader {
    /// Base directory for templates
    templates_dir: PathBuf,
}

impl Default for PersonalityLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl PersonalityLoader {
    /// Create a new loader with default templates directory
    pub fn new() -> Self {
        // Default to the templates directory relative to the executable
        let templates_dir = Self::default_templates_dir();
        Self { templates_dir }
    }

    /// Create a loader with a custom templates directory
    pub fn with_templates_dir(templates_dir: impl Into<PathBuf>) -> Self {
        Self {
            templates_dir: templates_dir.into(),
        }
    }

    /// Get the default templates directory
    fn default_templates_dir() -> PathBuf {
        // Try to find templates relative to the current directory
        // In development, this would be the project root
        // In production, this could be relative to the executable
        PathBuf::from("templates")
    }

    /// Load personality based on configuration
    ///
    /// Loading priority:
    /// 1. AgentDir/identity.md (user custom)
    /// 2. templates/identity-templates/{locale}/{yuan}.md
    /// 3. templates/identity-templates/{yuan}.md
    /// 4. Default embedded templates
    pub fn load(&self, config: &PersonalityConfig) -> anyhow::Result<Personality> {
        info!(
            "Loading personality for agent '{}' (user: {}, yuan: {}, locale: {})",
            config.agent_name,
            config.user_name,
            config.yuan_type.display_name(),
            config.locale
        );

        let identity = self.load_identity(config)?;
        let yuan = self.load_yuan(config)?;
        let ishiki = self.load_ishiki(config)?;

        let personality = Personality {
            identity,
            yuan,
            ishiki,
        };

        debug!("Personality loaded successfully");
        Ok(personality)
    }

    /// Load identity template
    fn load_identity(&self, config: &PersonalityConfig) -> anyhow::Result<String> {
        let yuan_name = config.yuan_type.template_name();

        // Priority 1: Custom agent directory
        if let Some(agent_dir) = &config.agent_dir {
            let custom_path = agent_dir.join("identity.md");
            if custom_path.exists() {
                debug!("Loading custom identity from: {:?}", custom_path);
                return self.read_and_fill(&custom_path, config);
            }
        }

        // Priority 2: Locale-specific template
        let locale_path = self
            .templates_dir
            .join("identity-templates")
            .join(&config.locale)
            .join(format!("{}.md", yuan_name));
        if locale_path.exists() {
            debug!("Loading locale identity from: {:?}", locale_path);
            return self.read_and_fill(&locale_path, config);
        }

        // Priority 3: Generic template
        let generic_path = self
            .templates_dir
            .join("identity-templates")
            .join(format!("{}.md", yuan_name));
        if generic_path.exists() {
            debug!("Loading generic identity from: {:?}", generic_path);
            return self.read_and_fill(&generic_path, config);
        }

        // Priority 4: Default embedded template
        debug!("Using default embedded identity template");
        let default_content = Self::default_identity_template(config.yuan_type);
        Ok(self.fill_variables(&default_content, config))
    }

    /// Load yuan template
    fn load_yuan(&self, config: &PersonalityConfig) -> anyhow::Result<String> {
        let yuan_name = config.yuan_type.template_name();

        // Priority 1: Custom agent directory
        if let Some(agent_dir) = &config.agent_dir {
            let custom_path = agent_dir.join("yuan.md");
            if custom_path.exists() {
                debug!("Loading custom yuan from: {:?}", custom_path);
                return self.read_and_fill(&custom_path, config);
            }
        }

        // Priority 2: Template file
        let template_path = self
            .templates_dir
            .join("yuan")
            .join(format!("{}.md", yuan_name));
        if template_path.exists() {
            debug!("Loading yuan from: {:?}", template_path);
            return self.read_and_fill(&template_path, config);
        }

        // Priority 3: Default embedded template
        debug!("Using default embedded yuan template");
        let default_content = config.yuan_type.default_yuan_content();
        Ok(self.fill_variables(default_content, config))
    }

    /// Load ishiki template
    fn load_ishiki(&self, config: &PersonalityConfig) -> anyhow::Result<String> {
        let yuan_name = config.yuan_type.template_name();

        // Priority 1: Custom agent directory
        if let Some(agent_dir) = &config.agent_dir {
            let custom_path = agent_dir.join("ishiki.md");
            if custom_path.exists() {
                debug!("Loading custom ishiki from: {:?}", custom_path);
                return self.read_and_fill(&custom_path, config);
            }
        }

        // Priority 2: Locale-specific template
        let locale_path = self
            .templates_dir
            .join("ishiki-templates")
            .join(&config.locale)
            .join(format!("{}.md", yuan_name));
        if locale_path.exists() {
            debug!("Loading locale ishiki from: {:?}", locale_path);
            return self.read_and_fill(&locale_path, config);
        }

        // Priority 3: Generic template
        let generic_path = self
            .templates_dir
            .join("ishiki-templates")
            .join(format!("{}.md", yuan_name));
        if generic_path.exists() {
            debug!("Loading generic ishiki from: {:?}", generic_path);
            return self.read_and_fill(&generic_path, config);
        }

        // Priority 4: Default embedded template
        debug!("Using default embedded ishiki template");
        let default_content = config.yuan_type.default_ishiki_content();
        Ok(self.fill_variables(default_content, config))
    }

    /// Read template file and fill variables
    fn read_and_fill(&self, path: &Path, config: &PersonalityConfig) -> anyhow::Result<String> {
        let content = std::fs::read_to_string(path)?;
        Ok(self.fill_variables(&content, config))
    }

    /// Fill template variables
    /// Supported variables:
    /// - {{agentName}} / {{agent_name}} -> agent_name
    /// - {{userName}} / {{user_name}} -> user_name
    /// - {{yuanType}} / {{yuan_type}} -> yuan_type display name
    /// - {{locale}} -> locale
    pub fn fill_variables(&self, template: &str, config: &PersonalityConfig) -> String {
        template
            .replace("{{agentName}}", &config.agent_name)
            .replace("{{agent_name}}", &config.agent_name)
            .replace("{{userName}}", &config.user_name)
            .replace("{{user_name}}", &config.user_name)
            .replace("{{yuanType}}", config.yuan_type.display_name())
            .replace("{{yuan_type}}", config.yuan_type.display_name())
            .replace("{{locale}}", &config.locale)
    }

    /// Get default identity template as fallback
    fn default_identity_template(yuan_type: YuanType) -> String {
        match yuan_type {
            YuanType::Hanako => r#"# {{agentName}}

{{userName}}的个人助手。感性与理性兼备，既有温度也有判断力。"#.to_string(),
            YuanType::Butter => r#"# {{agentName}}

{{userName}}的个人助手。感性优先，富有共情能力和创造力。"#.to_string(),
            YuanType::Ming => r#"# {{agentName}}

{{userName}}的个人助手。理性优先，逻辑清晰，善于深度思考。"#.to_string(),
        }
    }

    /// Reload personality with new configuration
    pub fn reload(&self, config: &PersonalityConfig) -> anyhow::Result<Personality> {
        info!("Reloading personality with new configuration");
        self.load(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_variables() {
        let loader = PersonalityLoader::new();
        let config = PersonalityConfig::new()
            .with_agent_name("TestAgent")
            .with_user_name("TestUser")
            .with_yuan_type(YuanType::Hanako)
            .with_locale("en");

        let template = "Hello {{userName}}, I am {{agentName}}. Yuan: {{yuanType}}";
        let result = loader.fill_variables(template, &config);

        assert_eq!(result, "Hello TestUser, I am TestAgent. Yuan: Hanako");
    }

    #[test]
    fn test_yuan_type_from_str() {
        assert_eq!(
            "hanako".parse::<YuanType>().unwrap(),
            YuanType::Hanako
        );
        assert_eq!(
            "butter".parse::<YuanType>().unwrap(),
            YuanType::Butter
        );
        assert_eq!(
            "ming".parse::<YuanType>().unwrap(),
            YuanType::Ming
        );
        assert!("unknown".parse::<YuanType>().is_err());
    }

    #[test]
    fn test_default_identity_templates() {
        let hanako = PersonalityLoader::default_identity_template(YuanType::Hanako);
        assert!(hanako.contains("{{agentName}}"));
        assert!(hanako.contains("{{userName}}"));

        let butter = PersonalityLoader::default_identity_template(YuanType::Butter);
        assert!(butter.contains("感性优先"));

        let ming = PersonalityLoader::default_identity_template(YuanType::Ming);
        assert!(ming.contains("理性优先"));
    }
}
