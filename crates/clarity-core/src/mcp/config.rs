use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Standard MCP configuration format matching Claude Desktop's `mcp.json`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct McpConfig {
    #[serde(rename = "mcpServers", default)]
    pub servers: HashMap<String, McpServerEntry>,
}

/// Per-server configuration entry inside `mcpServers`.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct McpServerEntry {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables passed to the MCP server process.
    ///
    /// Example for devbase tier filtering:
    /// `"DEVBASE_MCP_TOOL_TIERS": "stable,beta"` exposes only stable
    /// and beta tools (13 total), filtering out experimental ones.
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub disabled: bool,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub transport: Option<String>,
    #[serde(default)]
    pub headers: HashMap<String, String>,
}

impl McpConfig {
    /// Load MCP configuration from the given JSON file.
    pub fn load(path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let path = path.as_ref();
        if !path.exists() {
            anyhow::bail!("MCP config file not found: {}", path.display());
        }
        let contents = std::fs::read_to_string(path)?;
        let config: McpConfig = serde_json::from_str(&contents)?;
        Ok(config)
    }

    /// Load from the default path: `~/.config/clarity/mcp.json`.
    pub fn load_default() -> anyhow::Result<Self> {
        Self::load(default_config_path()?)
    }
}

/// Returns the default MCP config path.
pub fn default_config_path() -> anyhow::Result<PathBuf> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| anyhow::anyhow!("Could not determine user config directory"))?;
    Ok(config_dir.join("clarity").join("mcp.json"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_valid_config() {
        let json = r#"
        {
            "mcpServers": {
                "filesystem": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-filesystem", "."],
                    "env": { "KEY": "value" },
                    "disabled": false
                }
            }
        }
        "#;
        let config: McpConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.servers.len(), 1);
        let fs = config.servers.get("filesystem").unwrap();
        assert_eq!(fs.command, "npx");
        assert_eq!(
            fs.args,
            vec!["-y", "@modelcontextprotocol/server-filesystem", "."]
        );
        assert_eq!(fs.env.get("KEY"), Some(&"value".to_string()));
        assert!(!fs.disabled);
    }

    #[test]
    fn test_parse_disabled_server() {
        let json = r#"
        {
            "mcpServers": {
                "git": {
                    "command": "npx",
                    "args": ["-y", "@modelcontextprotocol/server-git"],
                    "disabled": true
                }
            }
        }
        "#;
        let config: McpConfig = serde_json::from_str(json).unwrap();
        let git = config.servers.get("git").unwrap();
        assert!(git.disabled);
    }

    #[test]
    fn test_parse_sse_config() {
        let json = r#"
        {
            "mcpServers": {
                "remote": {
                    "transport": "sse",
                    "url": "http://localhost:3001/sse",
                    "headers": { "Authorization": "Bearer token" }
                }
            }
        }
        "#;
        let config: McpConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.servers.len(), 1);
        let remote = config.servers.get("remote").unwrap();
        assert_eq!(remote.transport.as_deref().unwrap(), "sse");
        assert_eq!(remote.url.as_deref().unwrap(), "http://localhost:3001/sse");
        assert_eq!(
            remote.headers.get("Authorization"),
            Some(&"Bearer token".to_string())
        );
        assert!(!remote.disabled);
    }

    #[test]
    fn test_parse_http_config() {
        let json = r#"
        {
            "mcpServers": {
                "api": {
                    "transport": "http",
                    "url": "https://api.example.com/mcp",
                    "headers": { "X-Api-Key": "secret" }
                }
            }
        }
        "#;
        let config: McpConfig = serde_json::from_str(json).unwrap();
        let api = config.servers.get("api").unwrap();
        assert_eq!(api.transport.as_deref().unwrap(), "http");
        assert_eq!(api.url.as_deref().unwrap(), "https://api.example.com/mcp");
        assert_eq!(api.headers.get("X-Api-Key"), Some(&"secret".to_string()));
    }

    #[test]
    fn test_load_from_file() {
        let json = r#"
        {
            "mcpServers": {
                "stub": {
                    "command": "echo",
                    "args": ["hello"]
                }
            }
        }
        "#;
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(json.as_bytes()).unwrap();
        let config = McpConfig::load(file.path()).unwrap();
        assert!(config.servers.contains_key("stub"));
    }

    #[test]
    fn test_load_missing_file() {
        let result = McpConfig::load("/nonexistent/path/mcp.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_default_path() {
        let path = default_config_path().unwrap();
        let path_str = path.to_string_lossy();
        assert!(path_str.contains("clarity") && path_str.contains("mcp.json"));
    }
}
