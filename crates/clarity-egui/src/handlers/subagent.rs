use std::time::Instant;

use crate::stores::SubAgentStore;
use crate::ui::types::{AgentStatusEntry, SubAgentProgress};

pub fn on_subagent_batch(
    subagent_store: &mut SubAgentStore,
    batch_id: String,
    status: serde_json::Value,
) {
    let total = status["total"].as_u64().unwrap_or(0) as usize;
    let completed = status["completed"].as_u64().unwrap_or(0) as usize;
    let failed = status["failed"].as_u64().unwrap_or(0) as usize;
    let status_str = status["status"].as_str().unwrap_or("Running").to_string();
    let elapsed = status["elapsed_ms"].as_u64().unwrap_or(0);

    let agent_statuses: Vec<AgentStatusEntry> = status["agent_statuses"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .map(|v| AgentStatusEntry {
                    agent_id: v["agent_id"].as_str().unwrap_or("").to_string(),
                    status: v["status"].as_str().unwrap_or("").to_string(),
                    summary: v["summary"].as_str().map(|s| s.to_string()),
                })
                .collect()
        })
        .unwrap_or_default();

    let entry = SubAgentProgress {
        batch_id: batch_id.clone(),
        total,
        completed,
        failed,
        status: status_str,
        elapsed_ms: elapsed,
        agent_statuses,
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
