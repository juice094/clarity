//! Agent definition parsing for KimiCLI-style `agent.yaml` files.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use tracing::{debug, warn};

/// Parsed agent definition from `agent.yaml`.
#[derive(Debug, Clone)]
pub struct AgentDefinition {
    pub version: u32,
    pub name: Option<String>,
    pub system_prompt_path: Option<PathBuf>,
    pub system_prompt_args: HashMap<String, String>,
    pub tools: Vec<String>,
    pub subagents: HashMap<String, SubagentRef>,
}

/// Reference to a sub-agent definition.
#[derive(Debug, Clone)]
pub struct SubagentRef {
    pub path: PathBuf,
    pub description: String,
}

// ------------------------------------------------------------------
// Serde helpers for YAML parsing
// ------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct RawAgentYaml {
    version: u32,
    agent: RawAgent,
}

#[derive(Debug, Deserialize)]
struct RawAgent {
    name: Option<String>,
    system_prompt_path: Option<String>,
    #[serde(default)]
    system_prompt_args: HashMap<String, String>,
    #[serde(default)]
    tools: Vec<String>,
    #[serde(default)]
    subagents: HashMap<String, RawSubagentRef>,
}

#[derive(Debug, Deserialize)]
struct RawSubagentRef {
    path: String,
    description: String,
}

// ------------------------------------------------------------------
// Loading
// ------------------------------------------------------------------

/// Load an `AgentDefinition` from `dir/agent.yaml`.
pub fn load_agent_definition(dir: &Path) -> Result<AgentDefinition, crate::error::AgentError> {
    let path = dir.join("agent.yaml");
    debug!("Loading agent definition from: {}", path.display());

    let contents = std::fs::read_to_string(&path).map_err(|e| {
        crate::error::AgentError::Registry(format!(
            "Failed to read {}: {}",
            path.display(),
            e
        ))
    })?;

    let raw: RawAgentYaml = serde_yaml::from_str(&contents).map_err(|e| {
        crate::error::AgentError::Registry(format!(
            "Failed to parse {}: {}",
            path.display(),
            e
        ))
    })?;

    let agent = raw.agent;
    let system_prompt_path = agent.system_prompt_path.map(|p| dir.join(p));
    let subagents = agent
        .subagents
        .into_iter()
        .map(|(name, raw)| {
            (
                name,
                SubagentRef {
                    path: dir.join(raw.path),
                    description: raw.description,
                },
            )
        })
        .collect();

    Ok(AgentDefinition {
        version: raw.version,
        name: agent.name.filter(|n| !n.is_empty()),
        system_prompt_path,
        system_prompt_args: agent.system_prompt_args,
        tools: agent.tools,
        subagents,
    })
}

// ------------------------------------------------------------------
// Applying to AgentConfig
// ------------------------------------------------------------------

/// Apply an `AgentDefinition` to an existing `AgentConfig`.
///
/// - Sets `config.name`
/// - Loads `system_prompt` from the file referenced by `system_prompt_path`
/// - Sets `template_variables` from `system_prompt_args`
/// - Sets `prompts_dir` to the directory containing `agent.yaml`
pub fn apply_to_config(
    def: &AgentDefinition,
    config: &mut crate::agent::config::AgentConfig,
) -> Result<(), crate::error::AgentError> {
    // 1. Name
    if let Some(ref name) = def.name {
        config.name = Some(name.clone());
    }

    // 2. System prompt
    if let Some(ref prompt_path) = def.system_prompt_path {
        match super::config::load_prompt_from_file(prompt_path) {
            Some(prompt) => {
                config.system_prompt = prompt;
            }
            None => {
                warn!(
                    "System prompt file '{}' is empty or missing; keeping existing prompt.",
                    prompt_path.display()
                );
            }
        }
    }

    // 3. Template variables
    if !def.system_prompt_args.is_empty() {
        config.template_variables = def.system_prompt_args.clone();
    }

    // 4. Prompts directory (directory containing agent.yaml)
    if let Some(ref prompt_path) = def.system_prompt_path {
        if let Some(parent) = prompt_path.parent() {
            config.prompts_dir = Some(parent.to_path_buf());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_load_agent_definition_parsing() {
        let tmp = tempfile::tempdir().unwrap();
        let yaml = r#"
version: 1
agent:
  name: "test-agent"
  system_prompt_path: "prompts/system.md"
  system_prompt_args:
    key1: "value1"
    key2: "value2"
  tools:
    - "kimi_cli.tools.file:ReadFile"
    - "kimi_cli.tools.file:WriteFile"
  subagents:
    helper:
      path: "agents/helper.yaml"
      description: "A helper agent"
"#;
        let path = tmp.path().join("agent.yaml");
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(yaml.as_bytes()).unwrap();

        let def = load_agent_definition(tmp.path()).unwrap();

        assert_eq!(def.version, 1);
        assert_eq!(def.name, Some("test-agent".to_string()));
        assert_eq!(
            def.system_prompt_path,
            Some(tmp.path().join("prompts").join("system.md"))
        );
        assert_eq!(
            def.system_prompt_args.get("key1"),
            Some(&"value1".to_string())
        );
        assert_eq!(
            def.system_prompt_args.get("key2"),
            Some(&"value2".to_string())
        );
        assert_eq!(
            def.tools,
            vec![
                "kimi_cli.tools.file:ReadFile",
                "kimi_cli.tools.file:WriteFile"
            ]
        );
        assert_eq!(def.subagents.len(), 1);
        let helper = def.subagents.get("helper").unwrap();
        assert_eq!(helper.path, tmp.path().join("agents").join("helper.yaml"));
        assert_eq!(helper.description, "A helper agent");
    }

    #[test]
    fn test_load_agent_definition_missing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let result = load_agent_definition(tmp.path());
        assert!(result.is_err());
    }

    #[test]
    fn test_apply_to_config() {
        let tmp = tempfile::tempdir().unwrap();
        let prompt_path = tmp.path().join("system.md");
        std::fs::write(&prompt_path, "Hello {{name}}!").unwrap();

        let def = AgentDefinition {
            version: 1,
            name: Some("my-agent".to_string()),
            system_prompt_path: Some(prompt_path.clone()),
            system_prompt_args: {
                let mut m = std::collections::HashMap::new();
                m.insert("name".to_string(), "world".to_string());
                m
            },
            tools: vec![],
            subagents: std::collections::HashMap::new(),
        };

        let mut config = crate::agent::config::AgentConfig::default();
        // Ensure defaults are different
        assert_ne!(config.name, Some("my-agent".to_string()));
        assert_ne!(config.system_prompt, "Hello {{name}}!");
        assert!(config.template_variables.is_empty());

        apply_to_config(&def, &mut config).unwrap();

        assert_eq!(config.name, Some("my-agent".to_string()));
        assert_eq!(config.system_prompt, "Hello {{name}}!");
        assert_eq!(
            config.template_variables.get("name"),
            Some(&"world".to_string())
        );
        assert_eq!(config.prompts_dir, Some(tmp.path().to_path_buf()));
    }

    #[test]
    fn test_apply_to_config_missing_prompt_file() {
        let tmp = tempfile::tempdir().unwrap();
        let prompt_path = tmp.path().join("nonexistent.md");

        let def = AgentDefinition {
            version: 1,
            name: Some("my-agent".to_string()),
            system_prompt_path: Some(prompt_path),
            system_prompt_args: std::collections::HashMap::new(),
            tools: vec![],
            subagents: std::collections::HashMap::new(),
        };

        let original_prompt = crate::agent::config::AgentConfig::default().system_prompt;
        let mut config = crate::agent::config::AgentConfig::default();
        apply_to_config(&def, &mut config).unwrap();

        assert_eq!(config.name, Some("my-agent".to_string()));
        // Prompt file missing, should keep existing prompt
        assert_eq!(config.system_prompt, original_prompt);
    }
}
