use std::time::Instant;

use crate::stores::SubAgentStore;
use crate::ui::types::{SingleSubagentProgress, SubAgentProgress};

/// Handles the subagent batch event.
pub fn on_subagent_batch(
    subagent_store: &mut SubAgentStore,
    batch_id: String,
    status: serde_json::Value,
) {
    let total = status["total"].as_u64().unwrap_or(0) as usize;
    let completed = status["completed"].as_u64().unwrap_or(0) as usize;
    let failed = status["failed"].as_u64().unwrap_or(0) as usize;
    let status_str = status["status"].as_str().unwrap_or("Running").to_string();

    let entry = SubAgentProgress {
        batch_id: batch_id.clone(),
        total,
        completed,
        failed,
        status: status_str,
        last_poll: Instant::now(),
    };
    if let Some(existing) = subagent_store
        .parallel_batches
        .iter_mut()
        .find(|b| b.batch_id == batch_id)
    {
        *existing = entry;
    } else {
        subagent_store.parallel_batches.push(entry);
    }
    subagent_store.last_parallel_poll = Instant::now();
}

// ── Single subagent progress handlers (IS-1 Sprint 30) ──

fn ensure_agent(
    subagent_store: &mut SubAgentStore,
    agent_id: String,
) -> &mut SingleSubagentProgress {
    subagent_store
        .running_agents
        .entry(agent_id)
        .or_insert_with(|| SingleSubagentProgress {
            agent_type: "unknown".to_string(),
            status: "Pending".to_string(),
            stages: vec![],
            output_lines: vec![],
            started_at: Instant::now(),
            completed_at: None,
            steps: 0,
            max_steps: 0,
        })
}

/// Handles the subagent stage event.
pub fn on_subagent_stage(subagent_store: &mut SubAgentStore, agent_id: String, name: String) {
    ensure_agent(subagent_store, agent_id).stages.push(name);
}

/// Handles the subagent output event.
pub fn on_subagent_output(subagent_store: &mut SubAgentStore, agent_id: String, text: String) {
    let agent = ensure_agent(subagent_store, agent_id);
    agent.output_lines.push(text);
    // Cap output buffer to prevent unbounded growth in UI state
    if agent.output_lines.len() > 200 {
        agent.output_lines.drain(0..agent.output_lines.len() - 200);
    }
}

/// Handles the subagent status event.
pub fn on_subagent_status(
    subagent_store: &mut SubAgentStore,
    agent_id: String,
    agent_type: String,
    status: String,
) {
    let agent = ensure_agent(subagent_store, agent_id);
    agent.agent_type = agent_type;
    agent.status = status;
}

/// Handles the subagent progress event.
pub fn on_subagent_progress(
    subagent_store: &mut SubAgentStore,
    agent_id: String,
    steps: usize,
    max_steps: usize,
) {
    let agent = ensure_agent(subagent_store, agent_id);
    agent.steps = steps;
    agent.max_steps = max_steps;
}

/// Handles the subagent complete event.
pub fn on_subagent_complete(subagent_store: &mut SubAgentStore, agent_id: String, success: bool) {
    if let Some(agent) = subagent_store.running_agents.get_mut(&agent_id) {
        agent.status = if success {
            "Completed".to_string()
        } else {
            "Failed".to_string()
        };
        agent.completed_at = Some(Instant::now());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_store() -> SubAgentStore {
        SubAgentStore::default()
    }

    #[test]
    fn on_subagent_batch_inserts_new_entry() {
        let mut store = make_store();
        let status =
            serde_json::json!({"total": 5, "completed": 2, "failed": 1, "status": "Running"});
        on_subagent_batch(&mut store, "batch-1".into(), status);
        assert_eq!(store.parallel_batches.len(), 1);
        let batch = &store.parallel_batches[0];
        assert_eq!(batch.batch_id, "batch-1");
        assert_eq!(batch.total, 5);
        assert_eq!(batch.completed, 2);
        assert_eq!(batch.failed, 1);
        assert_eq!(batch.status, "Running");
    }

    #[test]
    fn on_subagent_batch_updates_existing_entry() {
        let mut store = make_store();
        let status1 =
            serde_json::json!({"total": 5, "completed": 1, "failed": 0, "status": "Running"});
        on_subagent_batch(&mut store, "batch-1".into(), status1);
        let status2 =
            serde_json::json!({"total": 5, "completed": 3, "failed": 0, "status": "Running"});
        on_subagent_batch(&mut store, "batch-1".into(), status2);
        assert_eq!(store.parallel_batches.len(), 1);
        assert_eq!(store.parallel_batches[0].completed, 3);
    }

    #[test]
    fn on_subagent_stage_records_stages() {
        let mut store = make_store();
        on_subagent_stage(&mut store, "agent-1".into(), "Planning".into());
        on_subagent_stage(&mut store, "agent-1".into(), "Executing".into());
        let agent = store.running_agents.get("agent-1").unwrap();
        assert_eq!(agent.stages, vec!["Planning", "Executing"]);
    }

    #[test]
    fn on_subagent_output_caps_at_200_lines() {
        let mut store = make_store();
        for i in 0..250 {
            on_subagent_output(&mut store, "agent-1".into(), format!("line {}", i));
        }
        let agent = store.running_agents.get("agent-1").unwrap();
        assert_eq!(agent.output_lines.len(), 200);
        assert_eq!(agent.output_lines[0], "line 50");
        assert_eq!(agent.output_lines[199], "line 249");
    }

    #[test]
    fn on_subagent_status_updates_agent() {
        let mut store = make_store();
        on_subagent_status(
            &mut store,
            "agent-1".into(),
            "coder".into(),
            "Running".into(),
        );
        let agent = store.running_agents.get("agent-1").unwrap();
        assert_eq!(agent.agent_type, "coder");
        assert_eq!(agent.status, "Running");
    }

    #[test]
    fn on_subagent_progress_updates_steps() {
        let mut store = make_store();
        on_subagent_progress(&mut store, "agent-1".into(), 3, 10);
        let agent = store.running_agents.get("agent-1").unwrap();
        assert_eq!(agent.steps, 3);
        assert_eq!(agent.max_steps, 10);
    }

    #[test]
    fn on_subagent_complete_success() {
        let mut store = make_store();
        // First ensure the agent exists.
        on_subagent_status(
            &mut store,
            "agent-1".into(),
            "coder".into(),
            "Running".into(),
        );
        on_subagent_complete(&mut store, "agent-1".into(), true);
        let agent = store.running_agents.get("agent-1").unwrap();
        assert_eq!(agent.status, "Completed");
        assert!(agent.completed_at.is_some());
    }

    #[test]
    fn on_subagent_complete_failure() {
        let mut store = make_store();
        on_subagent_status(
            &mut store,
            "agent-1".into(),
            "coder".into(),
            "Running".into(),
        );
        on_subagent_complete(&mut store, "agent-1".into(), false);
        let agent = store.running_agents.get("agent-1").unwrap();
        assert_eq!(agent.status, "Failed");
        assert!(agent.completed_at.is_some());
    }

    #[test]
    fn on_subagent_complete_unknown_agent_is_noop() {
        let mut store = make_store();
        // Should not panic when agent doesn't exist.
        on_subagent_complete(&mut store, "nonexistent".into(), true);
        assert!(store.running_agents.is_empty());
    }
}
