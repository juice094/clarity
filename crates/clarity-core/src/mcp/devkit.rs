//! Devkit result types for typed consumption of devbase MCP tool outputs.
//!
//! Provides strongly-typed structs matching the field contract agreed with
//! the devbase project for `devkit_project_context` and related tools.

use serde::{Deserialize, Serialize};

/// Result of `devkit_project_context` tool call.
///
/// Field contract (agreed with devbase v0.2.3):
/// - `repo` may be `null` when no repository is matched.
/// - `vault_notes[].source` is `"link"` (explicit association) or `"search"` (keyword match).
/// - `assets[].type` is optional; value `"folder"` or omitted.
/// - All array fields always return at least `[]`, never `null`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DevkitProjectContextResult {
    pub success: bool,
    pub project: String,
    #[serde(default)]
    pub repo: Option<DevkitRepo>,
    #[serde(default)]
    pub vault_notes: Vec<DevkitVaultNote>,
    #[serde(default)]
    pub assets: Vec<DevkitAsset>,
}

/// Repository metadata returned by devkit.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DevkitRepo {
    pub id: String,
    pub path: String,
    pub language: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub stars: u32,
}

/// A vault note referenced in the project context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DevkitVaultNote {
    pub id: String,
    pub title: String,
    /// `"link"` for explicit association, `"search"` for keyword match.
    pub source: String,
}

/// A project asset (file or folder).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DevkitAsset {
    pub name: String,
    pub path: String,
    /// `"folder"` or omitted.
    #[serde(default, rename = "type")]
    pub ty: Option<String>,
}

impl DevkitProjectContextResult {
    /// Parse from a raw `serde_json::Value` returned by `McpToolAdapter::execute()`.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSON does not match the expected field contract.
    pub fn from_value(value: &serde_json::Value) -> anyhow::Result<Self> {
        let text = value
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Expected string value from MCP tool result"))?;
        let result: Self = serde_json::from_str(text)
            .map_err(|e| anyhow::anyhow!("Failed to parse devkit_project_context result: {}", e))?;
        Ok(result)
    }

    /// Parse directly from a JSON string.
    pub fn parse(s: &str) -> anyhow::Result<Self> {
        let result: Self = serde_json::from_str(s)
            .map_err(|e| anyhow::anyhow!("Failed to parse devkit_project_context result: {}", e))?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_full_result() {
        let json = r#"
        {
            "success": true,
            "project": "devbase",
            "repo": {
                "id": "devbase",
                "path": "C:/Users/22414/Desktop/devbase",
                "language": "rust",
                "tags": ["cli", "mcp"],
                "stars": 42
            },
            "vault_notes": [
                {"id": "note-1", "title": "Architecture", "source": "link"},
                {"id": "note-2", "title": "Search result", "source": "search"}
            ],
            "assets": [
                {"name": "src", "path": "src", "type": "folder"},
                {"name": "Cargo.toml", "path": "Cargo.toml"}
            ]
        }
        "#;
        let result = DevkitProjectContextResult::parse(json).unwrap();
        assert!(result.success);
        assert_eq!(result.project, "devbase");
        let repo = result.repo.unwrap();
        assert_eq!(repo.id, "devbase");
        assert_eq!(repo.language, "rust");
        assert_eq!(repo.stars, 42);
        assert_eq!(result.vault_notes.len(), 2);
        assert_eq!(result.vault_notes[0].source, "link");
        assert_eq!(result.vault_notes[1].source, "search");
        assert_eq!(result.assets.len(), 2);
        assert_eq!(result.assets[0].ty.as_deref(), Some("folder"));
        assert!(result.assets[1].ty.is_none());
    }

    #[test]
    fn test_parse_minimal_result() {
        let json = r#"
        {
            "success": true,
            "project": "orphan",
            "repo": null,
            "vault_notes": [],
            "assets": []
        }
        "#;
        let result = DevkitProjectContextResult::parse(json).unwrap();
        assert!(result.success);
        assert!(result.repo.is_none());
        assert!(result.vault_notes.is_empty());
        assert!(result.assets.is_empty());
    }

    #[test]
    fn test_parse_from_value() {
        let json_str = r#"{"success":true,"project":"x","repo":null,"vault_notes":[],"assets":[]}"#;
        let value = serde_json::Value::String(json_str.to_string());
        let result = DevkitProjectContextResult::from_value(&value).unwrap();
        assert_eq!(result.project, "x");
    }

    #[test]
    fn test_parse_invalid_rejected() {
        let json = r#"{"success": true, "project": "x"}"#;
        // Missing arrays -> default to empty, should still parse
        let result = DevkitProjectContextResult::parse(json).unwrap();
        assert!(result.vault_notes.is_empty());
        assert!(result.assets.is_empty());
    }
}
