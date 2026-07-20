//! Load KimiClaw plugin Agent definitions into the Clarity LaborMarket.
//!
//! KimiClaw stores per-agent metadata under
//! `~/.kimi_openclaw/plugins/kimi-claw/agents/{id}/agent.json` with an optional
//! sibling `system.md`. This module parses those files and produces
//! `AgentTypeDefinition` values that `clarity-subagents` already understands.
//!
//! ponytail: tool name mapping is hard-coded because KimiClaw uses PascalCase
//! names (`Read`, `Glob`) while Clarity uses snake_case (`file_read`, `glob`).
//! If either side renames a tool, update `normalize_tool_name`.

use clarity_contract::subagent::AgentTypeDefinition;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Raw `agent.json` shape produced by the KimiClaw plugin.
#[derive(Debug, Clone, Deserialize)]
struct KimiClawAgentJson {
    /// Agent identifier, also the directory name.
    pub id: String,
    /// Display name (reserved for future UI; not mapped today).
    #[allow(dead_code)]
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// Default model alias.
    pub model: Option<String>,
    /// Capability tags.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Allowed tool names in KimiClaw naming.
    #[serde(rename = "allowedTools", default)]
    pub allowed_tools: Vec<String>,
    /// Maximum tokens to request.
    #[serde(rename = "maxTokens")]
    pub max_tokens: Option<usize>,
    /// Per-agent timeout in seconds.
    #[serde(rename = "timeoutSeconds")]
    pub timeout_seconds: Option<u64>,
    /// Whether the agent is read-only.
    #[serde(rename = "readOnly", default)]
    pub read_only: bool,
    /// Sibling file that contains the system prompt.
    #[serde(rename = "systemPromptFile", default = "default_system_prompt_file")]
    pub system_prompt_file: String,
}

fn default_system_prompt_file() -> String {
    "system.md".to_string()
}

/// Canonical path to the KimiClaw agents directory.
pub fn kimiclaw_agents_dir<P: AsRef<Path>>(openclaw_home: P) -> PathBuf {
    openclaw_home
        .as_ref()
        .join("plugins")
        .join("kimi-claw")
        .join("agents")
}

/// Load all KimiClaw agent definitions found under the agents directory.
///
/// Agents without a readable `agent.json` are skipped. If `system.md` (or the
/// configured `systemPromptFile`) is missing, the agent description is used as
/// the system prompt.
pub fn load_kimiclaw_agents<P: AsRef<Path>>(openclaw_home: P) -> Vec<AgentTypeDefinition> {
    let agents_dir = kimiclaw_agents_dir(openclaw_home);
    let mut out = Vec::new();

    let entries = match std::fs::read_dir(&agents_dir) {
        Ok(entries) => entries,
        Err(_) => return out,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(def) = load_single_agent(&path) {
            out.push(def);
        }
    }

    out
}

fn load_single_agent(agent_dir: &Path) -> Option<AgentTypeDefinition> {
    let config_path = agent_dir.join("agent.json");
    let raw = std::fs::read_to_string(&config_path).ok()?;
    let json: KimiClawAgentJson = serde_json::from_str(&raw).ok()?;

    let system_prompt_path = agent_dir.join(&json.system_prompt_file);
    let system_prompt =
        std::fs::read_to_string(&system_prompt_path).unwrap_or_else(|_| json.description.clone());

    let allowed_tools = if json.allowed_tools.is_empty() {
        None
    } else {
        Some(
            json.allowed_tools
                .iter()
                .map(|t| normalize_tool_name(t))
                .collect(),
        )
    };

    Some(AgentTypeDefinition {
        name: json.id,
        description: json.description,
        system_prompt,
        allowed_tools,
        max_iterations: 10,
        model: json.model,
        capabilities: json.capabilities,
        timeout_seconds: json.timeout_seconds,
        read_only: json.read_only,
        max_tokens: json.max_tokens,
    })
}

/// Map KimiClaw-style tool names to Clarity tool names.
///
/// ponytail: case-insensitive lookup; unknown names are passed through so the
/// caller can decide whether to warn or ignore.
fn normalize_tool_name(name: &str) -> String {
    match name.to_ascii_lowercase().as_str() {
        "read" => "file_read".to_string(),
        "write" => "file_write".to_string(),
        "edit" => "file_edit".to_string(),
        "glob" => "glob".to_string(),
        "grep" => "grep".to_string(),
        "bash" => "bash".to_string(),
        "shell" => "bash".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_agent_dir(parent: &tempfile::TempDir, id: &str, json: &str) -> PathBuf {
        let dir = parent
            .path()
            .join("plugins")
            .join("kimi-claw")
            .join("agents")
            .join(id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("agent.json"), json).unwrap();
        dir
    }

    #[test]
    fn load_kimiclaw_agents_maps_all_fields() {
        let dir = tempfile::tempdir().unwrap();
        let explore_dir = create_agent_dir(
            &dir,
            "explore",
            r#"{
                "id": "explore",
                "name": "Explore",
                "description": "Fast read-only search agent",
                "model": "kimi-coding/kimi-for-coding",
                "capabilities": ["search", "read"],
                "allowedTools": ["Glob", "Grep", "Read"],
                "maxTokens": 8192,
                "timeoutSeconds": 120,
                "readOnly": true,
                "systemPromptFile": "system.md"
            }"#,
        );
        std::fs::write(
            explore_dir.join("system.md"),
            "# Explore Agent\nOnly search.",
        )
        .unwrap();

        let agents = load_kimiclaw_agents(dir.path());
        assert_eq!(agents.len(), 1);

        let explore = &agents[0];
        assert_eq!(explore.name, "explore");
        assert_eq!(explore.model, Some("kimi-coding/kimi-for-coding".into()));
        assert_eq!(explore.capabilities, vec!["search", "read"]);
        assert_eq!(
            explore.allowed_tools,
            Some(vec!["glob".into(), "grep".into(), "file_read".into()])
        );
        assert_eq!(explore.max_tokens, Some(8192));
        assert_eq!(explore.timeout_seconds, Some(120));
        assert!(explore.read_only);
        assert_eq!(explore.system_prompt, "# Explore Agent\nOnly search.");
    }

    #[test]
    fn load_kimiclaw_agents_falls_back_to_description_when_system_md_missing() {
        let dir = tempfile::tempdir().unwrap();
        create_agent_dir(
            &dir,
            "plan",
            r#"{
                "id": "plan",
                "name": "Plan",
                "description": "Planning agent",
                "allowedTools": ["Read"],
                "readOnly": true
            }"#,
        );

        let agents = load_kimiclaw_agents(dir.path());
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].system_prompt, "Planning agent");
    }

    #[test]
    fn load_kimiclaw_agents_returns_empty_when_dir_missing() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_kimiclaw_agents(dir.path()).is_empty());
    }

    #[test]
    fn normalize_tool_name_maps_kimiclaw_to_clarity() {
        assert_eq!(normalize_tool_name("Read"), "file_read");
        assert_eq!(normalize_tool_name("Write"), "file_write");
        assert_eq!(normalize_tool_name("Edit"), "file_edit");
        assert_eq!(normalize_tool_name("Glob"), "glob");
        assert_eq!(normalize_tool_name("Grep"), "grep");
        assert_eq!(normalize_tool_name("Bash"), "bash");
        assert_eq!(normalize_tool_name("Shell"), "bash");
        assert_eq!(normalize_tool_name("Custom"), "custom");
    }
}
