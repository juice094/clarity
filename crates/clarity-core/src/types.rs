//! Core shared types for clarity-core
//!
//! Types in this module are used across multiple layers (agent, llm, approval, tools)
//! and are kept here to avoid circular dependencies.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Normalize a path by resolving `.` and `..` components.
/// Does not require the path to exist and does not add UNC prefixes.
pub fn normalize_path(path: &std::path::Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            std::path::Component::Prefix(p) => result.push(p.as_os_str()),
            std::path::Component::RootDir => result.push(component),
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                result.pop();
            }
            std::path::Component::Normal(name) => {
                result.push(name);
            }
        }
    }
    result
}

/// A tool call from the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: FunctionCall,
}

/// Function call details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String, // JSON string
}
