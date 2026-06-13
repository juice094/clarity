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
