//! End-to-end tests for the subagent / background-task wire protocol events.
//!
//! P7 discipline: every new `WireMessage` variant must demonstrate a producer,
//! a consumer, and an end-to-end path. These tests prove:
//! - `SubagentRunner` bridges `SubagentProgressEvent` emissions onto a parent
//!   wire (`SubagentStage` / `SubagentStatusChange` / `SubagentProgress`).
//! - `BackgroundTaskManager` emits `BackgroundTaskUpdate` on status changes.
//! - The new variants round-trip through JSON (Gateway WS envelope shape).

use std::sync::Arc;

use clarity_contract::subagent::RunSpec;
use clarity_core::agent::MockLlm;
use clarity_core::background::{BackgroundTaskManager, TaskResult, TaskSpec};
use clarity_core::registry::ToolRegistry;
use clarity_wire::{Wire, WireMessage};

#[tokio::test]
async fn test_subagent_events_roundtrip_json() {
    // Protocol stability: every new variant survives a JSON round-trip with
    // its snake_case tag, which is what Gateway WS clients deserialize.
    for (msg, tag) in [
        (
            WireMessage::SubagentStage {
                turn_id: String::new(),
                agent_id: "a1".to_string(),
                name: "runner_started".to_string(),
            },
            "subagent_stage",
        ),
        (
            WireMessage::SubagentOutput {
                turn_id: String::new(),
                agent_id: "a1".to_string(),
                text: "chunk".to_string(),
            },
            "subagent_output",
        ),
        (
            WireMessage::SubagentStatusChange {
                turn_id: String::new(),
                agent_id: "a1".to_string(),
                agent_type: "coder".to_string(),
                status: "Running".to_string(),
            },
            "subagent_status_change",
        ),
        (
            WireMessage::SubagentProgress {
                turn_id: String::new(),
                agent_id: "a1".to_string(),
                steps: 1,
                max_steps: 5,
            },
            "subagent_progress",
        ),
        (
            WireMessage::BackgroundTaskUpdate {
                turn_id: String::new(),
                task_id: "task_1".to_string(),
                task_name: "demo".to_string(),
                status: "completed".to_string(),
            },
            "background_task_update",
        ),
    ] {
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["type"], tag);
        let decoded: WireMessage = serde_json::from_value(json).unwrap();
        assert_eq!(decoded, msg);
    }
}

#[tokio::test]
async fn test_subagent_run_produces_wire_events_end_to_end() {
    let work_dir = tempfile::tempdir().unwrap();
    let context_dir = tempfile::tempdir().unwrap();

    let runner = clarity_subagents::SubagentRunner::new(
        ToolRegistry::with_builtin_tools(),
        work_dir.path(),
        context_dir.path(),
    )
    .with_llm(Arc::new(MockLlm));

    let wire = Wire::new();
    let mut ui = wire.ui_side(false);

    let mut store = clarity_subagents::SubagentStore::new(context_dir.path());
    let spec = RunSpec::new("e2e", "Do something")
        .with_type("coder")
        .without_git_context();

    let result = runner
        .run(spec, &mut store, Some(&wire))
        .await
        .expect("subagent run should succeed with MockLlm");

    // Consumer side: collect every subagent event and verify the lifecycle
    // is namespaced by agent_id and reaches a terminal status.
    let mut saw_stage = false;
    let mut saw_progress = false;
    let mut statuses = Vec::new();
    while let Some(msg) = ui.try_recv() {
        match msg {
            WireMessage::SubagentStage { agent_id, .. } => {
                assert_eq!(agent_id, result.agent_id);
                saw_stage = true;
            }
            WireMessage::SubagentProgress { agent_id, .. } => {
                assert_eq!(agent_id, result.agent_id);
                saw_progress = true;
            }
            WireMessage::SubagentStatusChange {
                agent_id, status, ..
            } => {
                assert_eq!(agent_id, result.agent_id);
                statuses.push(status);
            }
            other => panic!("unexpected wire message: {:?}", other),
        }
    }
    assert!(saw_stage, "expected SubagentStage events");
    assert!(saw_progress, "expected SubagentProgress events");
    assert_eq!(statuses.first().map(String::as_str), Some("Running"));
    assert_eq!(statuses.last().map(String::as_str), Some("Completed"));
}

#[tokio::test]
async fn test_background_task_produces_wire_events_end_to_end() {
    let temp = tempfile::tempdir().unwrap();
    let wire = Arc::new(Wire::new());
    let mut ui = wire.ui_side(false);

    let manager = BackgroundTaskManager::new(
        temp.path().join("store"),
        temp.path().join("work"),
        temp.path().join("context"),
    )
    .with_wire(wire);

    let task_id = manager
        .spawn(TaskSpec::new("e2e_task", "prompt"), |_spec| async {
            Ok(TaskResult::success("done"))
        })
        .await
        .unwrap();
    manager.wait(&task_id).await.unwrap();

    let mut statuses = Vec::new();
    while let Some(msg) = ui.try_recv() {
        match msg {
            WireMessage::BackgroundTaskUpdate {
                task_id: id,
                task_name,
                status,
                ..
            } => {
                assert_eq!(id, task_id);
                assert_eq!(task_name, "e2e_task");
                statuses.push(status);
            }
            other => panic!("unexpected wire message: {:?}", other),
        }
    }
    assert_eq!(statuses, vec!["pending", "running", "completed"]);
}
