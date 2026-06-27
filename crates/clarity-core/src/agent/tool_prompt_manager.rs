//! Per-turn tool schema snapshot for LLM requests.
//!
//! Avoids re-serializing the full tool JSON Schema on every ReAct iteration
//! inside a single turn. The snapshot is initialized once per turn and
//! incrementally updated when the circuit breaker disables a failing tool.

use serde_json::Value;
use sha2::{Digest, Sha256};

/// Stable snapshot of the working tool set for the current turn.
#[derive(Debug, Clone)]
pub(crate) struct ToolPromptManager {
    /// Stable hash of the current working tool set.
    schema_hash: u64,
    /// Pre-serialized tools value for the LLM request.
    tools_value: Value,
}

impl ToolPromptManager {
    /// Build a snapshot from the filtered tool schema.
    pub fn new(tools: &Value) -> Self {
        let tools_value = tools.clone();
        Self {
            schema_hash: hash_value(&tools_value),
            tools_value,
        }
    }

    /// Read-only access to the current LLM tool parameter.
    pub fn tools_value(&self) -> &Value {
        &self.tools_value
    }

    /// Remove a tool by name after a circuit-breaker failure.
    ///
    /// Returns `true` if a tool was removed.
    pub fn filter_tool(&mut self, name: &str) -> bool {
        let original_len = self.tools_value.as_array().map(|a| a.len()).unwrap_or(0);
        if let Some(arr) = self.tools_value.as_array_mut() {
            arr.retain(|v| {
                v.get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|tool_name| tool_name != name)
                    .unwrap_or(true)
            });
        }
        let removed = self.tools_value.as_array().map(|a| a.len()).unwrap_or(0) < original_len;
        if removed {
            self.schema_hash = hash_value(&self.tools_value);
        }
        removed
    }

    /// True if the provided schema differs from the current snapshot.
    ///
    /// Useful for detecting unexpected registry drift mid-turn.
    #[allow(dead_code)]
    pub fn is_stale(&self, tools: &Value) -> bool {
        self.schema_hash != hash_value(tools)
    }
}

fn hash_value(value: &Value) -> u64 {
    // Normalized form: deterministic JSON string hashed with SHA-256.
    let digest = Sha256::digest(value.to_string().as_bytes());
    u64::from_le_bytes(digest[..8].try_into().unwrap_or([0; 8]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_tools() -> Value {
        json!([
            { "function": { "name": "read", "description": "Read a file" } },
            { "function": { "name": "write", "description": "Write a file" } },
            { "function": { "name": "grep", "description": "Search text" } }
        ])
    }

    #[test]
    fn new_stores_tools_and_hash() {
        let tools = sample_tools();
        let manager = ToolPromptManager::new(&tools);
        assert_eq!(manager.tools_value(), &tools);
        assert!(!manager.is_stale(&tools));
    }

    #[test]
    fn filter_tool_removes_and_updates_hash() {
        let tools = sample_tools();
        let mut manager = ToolPromptManager::new(&tools);
        let original_hash = manager.schema_hash;

        assert!(manager.filter_tool("write"));
        assert_eq!(manager.tools_value().as_array().unwrap().len(), 2);
        assert!(
            !manager
                .tools_value()
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["function"]["name"] == "write")
        );
        assert_ne!(manager.schema_hash, original_hash);
    }

    #[test]
    fn filter_tool_missing_returns_false() {
        let tools = sample_tools();
        let mut manager = ToolPromptManager::new(&tools);
        let original_hash = manager.schema_hash;

        assert!(!manager.filter_tool("missing"));
        assert_eq!(manager.tools_value().as_array().unwrap().len(), 3);
        assert_eq!(manager.schema_hash, original_hash);
    }

    #[test]
    fn is_stale_detects_drift() {
        let tools = sample_tools();
        let manager = ToolPromptManager::new(&tools);
        let mut changed = tools.clone();
        changed.as_array_mut().unwrap().pop();
        assert!(manager.is_stale(&changed));
    }
}
