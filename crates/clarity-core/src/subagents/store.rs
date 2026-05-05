//! Subagent state storage
//!
//! Manages persistent state for subagent instances.

use crate::llm::api::Message;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Status of a subagent instance
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubagentStatus {
    Idle,
    Running,
    Completed,
    Failed,
}

/// State of a subagent instance
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentState {
    pub agent_id: String,
    pub agent_type: String,
    pub status: SubagentStatus,
    pub history: Vec<Message>,
    pub created_at: u64,
    pub updated_at: u64,
}

impl SubagentState {
    pub fn new(agent_id: String, agent_type: String) -> Self {
        let now = now_timestamp();
        Self {
            agent_id,
            agent_type,
            status: SubagentStatus::Idle,
            history: Vec::new(),
            created_at: now,
            updated_at: now,
        }
    }
}

/// Storage for subagent states
pub struct SubagentStore {
    root_dir: PathBuf,
    in_memory: HashMap<String, SubagentState>,
}

impl SubagentStore {
    /// Create new store with root directory
    pub fn new(root_dir: impl AsRef<Path>) -> Self {
        Self {
            root_dir: root_dir.as_ref().to_path_buf(),
            in_memory: HashMap::new(),
        }
    }

    /// Create state for a new subagent
    pub fn create(&mut self, agent_id: String, agent_type: String) -> &SubagentState {
        let state = SubagentState::new(agent_id.clone(), agent_type);
        self.in_memory.insert(agent_id.clone(), state);
        // SAFE: state was just inserted with the same agent_id on the line above.
        self.in_memory.get(&agent_id).unwrap()
    }

    /// Get state by agent_id
    pub fn get(&self, agent_id: &str) -> Option<&SubagentState> {
        self.in_memory.get(agent_id)
    }

    /// Get mutable state
    pub fn get_mut(&mut self, agent_id: &str) -> Option<&mut SubagentState> {
        self.in_memory.get_mut(agent_id)
    }

    /// Update state
    pub fn update(&mut self, state: SubagentState) {
        self.in_memory.insert(state.agent_id.clone(), state);
    }

    /// Update status
    pub fn update_status(&mut self, agent_id: &str, status: SubagentStatus) {
        if let Some(state) = self.in_memory.get_mut(agent_id) {
            state.status = status;
            state.updated_at = now_timestamp();
        }
    }

    /// Add message to history
    pub fn add_message(&mut self, agent_id: &str, message: Message) {
        if let Some(state) = self.in_memory.get_mut(agent_id) {
            state.history.push(message);
            state.updated_at = now_timestamp();
        }
    }

    /// List all states
    pub fn list(&self) -> Vec<&SubagentState> {
        self.in_memory.values().collect()
    }

    /// List by status
    pub fn list_by_status(&self, status: SubagentStatus) -> Vec<&SubagentState> {
        self.in_memory
            .values()
            .filter(|s| s.status == status)
            .collect()
    }

    /// Delete state
    pub fn delete(&mut self, agent_id: &str) -> Option<SubagentState> {
        self.in_memory.remove(agent_id)
    }

    /// Save to disk (async for future file persistence)
    pub async fn persist(&self, _agent_id: &str) -> anyhow::Result<()> {
        // For now, just ensure directory exists
        // Full file persistence can be added later
        tokio::fs::create_dir_all(&self.root_dir).await?;
        Ok(())
    }

    // ------------------------------------------------------------------
    // J8: SubagentManager ↔ Jumpy Predictor integration stubs
    // ------------------------------------------------------------------

    /// Current semantic tags (stub — returns empty until tagging is implemented).
    pub fn current_tags(&self) -> Vec<String> {
        Vec::new()
    }

    /// Working memory key-value store (stub — returns empty).
    pub fn working_memory(&self) -> HashMap<String, String> {
        HashMap::new()
    }

    /// Currently active / modified files (stub — returns empty).
    pub fn active_files(&self) -> Vec<String> {
        Vec::new()
    }

    /// High-level context summary (stub — returns empty).
    pub fn context_summary(&self) -> String {
        String::new()
    }

    /// Estimated progress toward goal [0.0, 1.0] (stub — returns 0.0).
    pub fn progress(&self) -> f32 {
        0.0
    }
}

fn now_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_and_get() {
        let mut store = SubagentStore::new("/tmp/test");
        let state = store.create("agent-1".to_string(), "coder".to_string());

        assert_eq!(state.agent_id, "agent-1");
        assert_eq!(state.agent_type, "coder");
        assert_eq!(state.status, SubagentStatus::Idle);

        let retrieved = store.get("agent-1").unwrap();
        assert_eq!(retrieved.agent_id, "agent-1");
    }

    #[test]
    fn test_update_status() {
        let mut store = SubagentStore::new("/tmp/test");
        store.create("agent-1".to_string(), "coder".to_string());

        store.update_status("agent-1", SubagentStatus::Running);

        let state = store.get("agent-1").unwrap();
        assert_eq!(state.status, SubagentStatus::Running);
    }

    #[test]
    fn test_list_by_status() {
        let mut store = SubagentStore::new("/tmp/test");
        store.create("agent-1".to_string(), "coder".to_string());
        store.create("agent-2".to_string(), "explore".to_string());
        store.update_status("agent-1", SubagentStatus::Running);

        let running = store.list_by_status(SubagentStatus::Running);
        assert_eq!(running.len(), 1);
        assert_eq!(running[0].agent_id, "agent-1");
    }

    #[test]
    fn test_delete() {
        let mut store = SubagentStore::new("/tmp/test");
        store.create("agent-1".to_string(), "coder".to_string());

        let deleted = store.delete("agent-1");
        assert!(deleted.is_some());
        assert!(store.get("agent-1").is_none());
    }
}
