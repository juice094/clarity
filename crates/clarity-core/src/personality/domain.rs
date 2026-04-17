//! Domain-specific Persona Configuration
//!
//! Supports parsing TOML files like `agri_expert.toml` that extend
//! the base `PersonalityConfig` with domain tools and custom templates.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// Wrapper for the top-level `[persona]` table in TOML
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainPersonaRoot {
    pub persona: DomainPersonaConfig,
}

/// Domain persona configuration inside the `[persona]` table
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainPersonaConfig {
    pub agent_name: String,
    pub user_name: String,
    pub yuan_type: String,
    pub locale: String,
    #[serde(default)]
    pub template_variables: HashMap<String, String>,
    #[serde(default)]
    pub tools: Option<DomainToolsConfig>,
    #[serde(default)]
    pub system_prompt: Option<SystemPromptConfig>,
}

/// Domain-specific tool schema list
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainToolsConfig {
    pub schemas: Vec<DomainToolSchema>,
}

/// Single tool schema entry for the domain
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainToolSchema {
    pub name: String,
    pub description: String,
    #[serde(default)]
    pub parameters: HashMap<String, String>,
}

/// System prompt template configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemPromptConfig {
    pub template: String,
}

/// Parse a domain persona TOML file
pub fn parse_domain_persona(path: impl AsRef<Path>) -> anyhow::Result<DomainPersonaConfig> {
    let content = std::fs::read_to_string(path)?;
    let root: DomainPersonaRoot = toml::from_str(&content)?;
    Ok(root.persona)
}

/// Parse a domain persona TOML string (useful for tests)
pub fn parse_domain_persona_str(content: &str) -> anyhow::Result<DomainPersonaConfig> {
    let root: DomainPersonaRoot = toml::from_str(content)?;
    Ok(root.persona)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_agri_expert_persona() {
        let toml = r#"
[persona]
agent_name = "AgriExpert"
user_name = "Farmer"
yuan_type = "Direct"
locale = "zh-CN"

[persona.template_variables]
crop = "水稻"
region = "江苏"
season = "春季"

[persona.tools]
schemas = [
    { name = "agri_query", description = "查询农业知识库", parameters = { crop = "string", symptoms = "string", limit = "integer" } },
    { name = "agri_eval", description = "运行农业 benchmark", parameters = { condition = "string", count = "integer" } },
    { name = "agri_report", description = "生成诊断报告" }
]

[persona.system_prompt]
template = """你是一位农业技术推广员，擅长{{crop}}栽培。
当前地区：{{region}}，季节：{{season}}。
请优先调用工具查询知识库，再给出诊断建议。"""
"#;

        let config = parse_domain_persona_str(toml).expect("should parse valid agri persona");

        assert_eq!(config.agent_name, "AgriExpert");
        assert_eq!(config.user_name, "Farmer");
        assert_eq!(config.yuan_type, "Direct");
        assert_eq!(config.locale, "zh-CN");

        assert_eq!(config.template_variables.get("crop"), Some(&"水稻".to_string()));
        assert_eq!(config.template_variables.get("region"), Some(&"江苏".to_string()));
        assert_eq!(config.template_variables.get("season"), Some(&"春季".to_string()));

        let tools = config.tools.expect("tools section should exist");
        assert_eq!(tools.schemas.len(), 3);

        let agri_query = &tools.schemas[0];
        assert_eq!(agri_query.name, "agri_query");
        assert_eq!(agri_query.description, "查询农业知识库");
        assert_eq!(agri_query.parameters.get("crop"), Some(&"string".to_string()));
        assert_eq!(agri_query.parameters.get("symptoms"), Some(&"string".to_string()));
        assert_eq!(agri_query.parameters.get("limit"), Some(&"integer".to_string()));

        let agri_report = &tools.schemas[2];
        assert_eq!(agri_report.name, "agri_report");
        assert_eq!(agri_report.description, "生成诊断报告");
        assert!(agri_report.parameters.is_empty());

        let sys_prompt = config.system_prompt.expect("system_prompt should exist");
        assert!(sys_prompt.template.contains("农业技术推广员"));
        assert!(sys_prompt.template.contains("{{crop}}"));
        assert!(sys_prompt.template.contains("{{region}}"));
    }

    #[test]
    fn test_parse_minimal_domain_persona() {
        let toml = r#"
[persona]
agent_name = "TestAgent"
user_name = "User"
yuan_type = "Hanako"
locale = "en"
"#;

        let config = parse_domain_persona_str(toml).expect("should parse minimal persona");
        assert_eq!(config.agent_name, "TestAgent");
        assert!(config.tools.is_none());
        assert!(config.system_prompt.is_none());
        assert!(config.template_variables.is_empty());
    }
}
