//! Domain-specific persona configuration parser.
//!
//! Supports TOML files that define vertical-domain personas
//! (e.g. agricultural expert, medical assistant) with custom
//! tools and system prompt templates.

use std::collections::HashMap;
use std::path::Path;

/// Top-level domain persona configuration.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DomainPersonaConfig {
    pub persona: BasePersona,
    #[serde(default)]
    pub tools: Option<Vec<DomainToolSchema>>,
    #[serde(default)]
    pub system_prompt: Option<SystemPromptConfig>,
}

/// Core identity fields for a persona.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BasePersona {
    pub agent_name: String,
    pub user_name: String,
    pub yuan_type: String,
    pub locale: String,
    #[serde(default)]
    pub template_variables: HashMap<String, String>,
}

/// Schema for a domain-specific tool override.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct DomainToolSchema {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub parameters: HashMap<String, String>,
}

/// System prompt template configuration.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct SystemPromptConfig {
    pub template: String,
}

/// Parse a `DomainPersonaConfig` from a TOML file.
pub fn parse_domain_persona(
    path: impl AsRef<Path>,
) -> Result<DomainPersonaConfig, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let config: DomainPersonaConfig = toml::from_str(&content)?;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn make_sample_toml() -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(
            br#"
[persona]
agent_name = "AgriExpert"
user_name = "Farmer"
yuan_type = "domain"
locale = "zh-CN"

[persona.template_variables]
crop = "wheat"
region = "north"

[[tools]]
name = "diagnose"
description = "Diagnose crop disease"

[tools.parameters]
crop_type = "string"

[system_prompt]
template = "You are {{agent_name}}, helping {{user_name}} with {{crop}} in {{region}}."
"#,
        )
        .unwrap();
        file
    }

    #[test]
    fn test_parse_domain_persona() {
        let file = make_sample_toml();
        let config = parse_domain_persona(file.path()).unwrap();
        assert_eq!(config.persona.agent_name, "AgriExpert");
        assert_eq!(config.persona.user_name, "Farmer");
        assert_eq!(
            config.persona.template_variables.get("crop").unwrap(),
            "wheat"
        );
        assert_eq!(config.tools.as_ref().unwrap().len(), 1);
        assert!(
            config
                .system_prompt
                .as_ref()
                .unwrap()
                .template
                .contains("{{agent_name}}")
        );
    }
}
