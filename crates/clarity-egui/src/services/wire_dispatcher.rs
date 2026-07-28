//! Deserialize `clarity_wire::WireMessage` payloads and forward matching `UiEvent`s.

use crate::ui::types::UiEvent;
use std::collections::HashMap;
use std::sync::mpsc;

/// Deserialize a `clarity_wire::WireMessage` JSON payload and forward the
/// matching `UiEvent` to `tx`.
pub fn dispatch_wire_payload(
    payload: &serde_json::Value,
    session_id: &str,
    tx: &mpsc::Sender<UiEvent>,
) {
    let msg: clarity_wire::WireMessage = match serde_json::from_value(payload.clone()) {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!("Failed to deserialize WireMessage payload: {}", e);
            return;
        }
    };
    dispatch_wire_message(msg, session_id, tx);
}

/// Forward a decoded `clarity_wire::WireMessage` to `tx`.
pub fn dispatch_wire_message(
    msg: clarity_wire::WireMessage,
    session_id: &str,
    tx: &mpsc::Sender<UiEvent>,
) {
    let sid = session_id.to_string();
    let event = match msg {
        clarity_wire::WireMessage::ContentPart { text, .. } => Some(UiEvent::Chunk {
            session_id: sid,
            text,
        }),
        clarity_wire::WireMessage::ReasoningPart { text, .. } => Some(UiEvent::ReasoningChunk {
            session_id: sid,
            text,
        }),
        clarity_wire::WireMessage::DraftEvent { event, .. } => match event {
            clarity_wire::DraftEvent::Progress { text } => Some(UiEvent::DraftProgress {
                session_id: sid,
                text,
            }),
            clarity_wire::DraftEvent::Clear => Some(UiEvent::DraftClear { session_id: sid }),
            clarity_wire::DraftEvent::Content { text } => Some(UiEvent::DraftContent {
                session_id: sid,
                text,
            }),
        },
        clarity_wire::WireMessage::ToolCall {
            id,
            name,
            arguments,
            ..
        } => Some(UiEvent::ToolStart {
            session_id: sid,
            id,
            name,
            arguments,
        }),
        clarity_wire::WireMessage::ToolCallProgress {
            index,
            name,
            arguments_so_far,
            ..
        } => Some(UiEvent::ToolCallProgress {
            session_id: sid,
            index,
            name,
            arguments_so_far,
        }),
        clarity_wire::WireMessage::ToolResult {
            id,
            result,
            display_result,
            ..
        } => Some(UiEvent::ToolResult {
            session_id: sid,
            id,
            result,
            display_result,
        }),
        clarity_wire::WireMessage::StepBegin { tool_name, .. } => Some(UiEvent::StepBegin {
            session_id: sid,
            tool_name,
        }),
        clarity_wire::WireMessage::CompactionBegin { .. } => {
            Some(UiEvent::CompactionBegin { session_id: sid })
        }
        clarity_wire::WireMessage::CompactionEnd { .. } => {
            Some(UiEvent::CompactionEnd { session_id: sid })
        }
        clarity_wire::WireMessage::PlanStepBegin {
            step_id, tool_name, ..
        } => Some(UiEvent::PlanStepBegin {
            session_id: sid,
            step_id,
            tool_name,
        }),
        clarity_wire::WireMessage::PlanStepEnd {
            step_id, success, ..
        } => Some(UiEvent::PlanStepEnd {
            session_id: sid,
            step_id,
            success,
        }),
        clarity_wire::WireMessage::PlanStepSkipped { step_id, .. } => {
            Some(UiEvent::PlanStepSkipped {
                session_id: sid,
                step_id,
            })
        }
        clarity_wire::WireMessage::TurnBegin {
            turn_id,
            user_input,
            ..
        } => Some(UiEvent::TurnStart {
            session_id: sid,
            turn_id,
            user_input,
        }),
        clarity_wire::WireMessage::TurnEnd { .. } => Some(UiEvent::TurnEnd { session_id: sid }),
        clarity_wire::WireMessage::StatusUpdate { message, .. } => Some(UiEvent::StatusUpdate {
            session_id: sid,
            message,
        }),
        clarity_wire::WireMessage::ViewStateUpdate { turn, .. } => Some(UiEvent::ViewStateUpdate {
            session_id: sid,
            turn: turn.map(Into::into),
        }),
        clarity_wire::WireMessage::ThreadActive {
            thread_id, title, ..
        } => Some(UiEvent::ThreadActive { thread_id, title }),
        clarity_wire::WireMessage::ThreadList { threads, .. } => {
            let sessions = threads
                .into_iter()
                .map(|t| crate::ui::types::Session {
                    id: t.thread_id,
                    title: t.title.unwrap_or_default(),
                    category: "engineering".to_string(),
                    project_id: None,
                    context: crate::ui::types::SessionContext::default(),
                    lifecycle: crate::ui::types::SessionLifecycle::default(),
                    archived: false,
                    messages: Vec::new(),
                    updated_at: crate::session::now_millis(),
                    last_saved_at: crate::session::now_millis(),
                    turn_heights: Vec::new(),
                    estimate_buffer: Vec::new(),
                    line_offset_buffer: Vec::new(),
                    height_cache: None,
                    provider_state: HashMap::new(),
                    in_flight: false,
                    diff_stats: None,
                    turn_usage: HashMap::new(),
                })
                .collect();
            Some(UiEvent::ThreadList { threads: sessions })
        }
        clarity_wire::WireMessage::ThreadCreated {
            thread_id, title, ..
        } => Some(UiEvent::ThreadCreated {
            session: crate::ui::types::Session {
                id: thread_id,
                title: title.unwrap_or_default(),
                category: "engineering".to_string(),
                project_id: None,
                context: crate::ui::types::SessionContext::default(),
                lifecycle: crate::ui::types::SessionLifecycle::default(),
                archived: false,
                messages: Vec::new(),
                updated_at: crate::session::now_millis(),
                last_saved_at: crate::session::now_millis(),
                turn_heights: Vec::new(),
                estimate_buffer: Vec::new(),
                line_offset_buffer: Vec::new(),
                height_cache: None,
                provider_state: HashMap::new(),
                in_flight: false,
                diff_stats: None,
                turn_usage: HashMap::new(),
            },
        }),
        clarity_wire::WireMessage::ThreadUpdated {
            thread_id,
            title,
            archived,
            ..
        } => Some(UiEvent::ThreadUpdated {
            thread_id,
            title,
            archived,
        }),
        clarity_wire::WireMessage::Usage {
            turn_id,
            prompt_tokens,
            completion_tokens,
            total_tokens,
            ..
        } => Some(UiEvent::Usage {
            session_id: sid,
            turn_id,
            prompt_tokens,
            completion_tokens,
            total_tokens,
        }),
        clarity_wire::WireMessage::SubagentStage { agent_id, name, .. } => {
            Some(UiEvent::SubagentStage { agent_id, name })
        }
        clarity_wire::WireMessage::SubagentOutput { agent_id, text, .. } => {
            Some(UiEvent::SubagentOutput { agent_id, text })
        }
        clarity_wire::WireMessage::SubagentProgress {
            agent_id,
            steps,
            max_steps,
            ..
        } => Some(UiEvent::SubagentProgress {
            agent_id,
            steps,
            max_steps,
        }),
        clarity_wire::WireMessage::SubagentStatusChange {
            agent_id,
            agent_type,
            status,
            ..
        } => {
            // Mirror the legacy mpsc bridge: a terminal status additionally
            // produces SubagentComplete so stores can stamp completion time.
            let success = status == "Completed";
            let terminal = success || status == "Failed";
            let _ = tx.send(UiEvent::SubagentStatus {
                agent_id: agent_id.clone(),
                agent_type,
                status,
            });
            if terminal {
                let _ = tx.send(UiEvent::SubagentComplete { agent_id, success });
            }
            None
        }
        // Gateway 已将 BackgroundTaskManager 绑定到 app 级 event_wire，
        // 收到任务状态变更后通知 UI 刷新任务列表。
        clarity_wire::WireMessage::BackgroundTaskUpdate {
            task_id,
            task_name,
            status,
            ..
        } => Some(UiEvent::BackgroundTaskUpdate {
            task_id,
            task_name,
            status,
        }),
    };
    if let Some(ev) = event {
        if let Err(e) = tx.send(ev) {
            tracing::warn!("Failed to send wire event: {}", e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    /// Build a WireMessage from a JSON payload string.
    fn wire_msg(json: &str) -> clarity_wire::WireMessage {
        serde_json::from_str(json).expect("test wire message must parse")
    }

    #[test]
    fn content_part_maps_to_chunk() {
        let (tx, rx) = mpsc::channel();
        let msg = wire_msg(r#"{"type":"content_part","text":"hello","index":0}"#);
        dispatch_wire_message(msg, "sess-1", &tx);
        let event = rx.try_recv().unwrap();
        match event {
            UiEvent::Chunk { session_id, text } => {
                assert_eq!(session_id, "sess-1");
                assert_eq!(text, "hello");
            }
            other => panic!("Expected Chunk, got {:?}", other),
        }
    }

    #[test]
    fn reasoning_part_maps_to_reasoning_chunk() {
        let (tx, rx) = mpsc::channel();
        let msg = wire_msg(r#"{"type":"reasoning_part","text":"thinking...","index":0}"#);
        dispatch_wire_message(msg, "sess-2", &tx);
        let event = rx.try_recv().unwrap();
        match event {
            UiEvent::ReasoningChunk { session_id, text } => {
                assert_eq!(session_id, "sess-2");
                assert_eq!(text, "thinking...");
            }
            other => panic!("Expected ReasoningChunk, got {:?}", other),
        }
    }

    #[test]
    fn tool_call_maps_to_tool_start() {
        let (tx, rx) = mpsc::channel();
        let msg = wire_msg(
            r#"{"type":"tool_call","id":"t1","name":"read_file","arguments":{"path":"/tmp/x.rs"},"index":0}"#,
        );
        dispatch_wire_message(msg, "sess-3", &tx);
        let event = rx.try_recv().unwrap();
        match event {
            UiEvent::ToolStart {
                session_id,
                id,
                name,
                ..
            } => {
                assert_eq!(session_id, "sess-3");
                assert_eq!(id, "t1");
                assert_eq!(name, "read_file");
            }
            other => panic!("Expected ToolStart, got {:?}", other),
        }
    }

    #[test]
    fn tool_result_maps_correctly() {
        let (tx, rx) = mpsc::channel();
        let msg = wire_msg(
            r#"{"type":"tool_result","id":"t1","result":"ok","display_result":null,"index":0}"#,
        );
        dispatch_wire_message(msg, "sess-4", &tx);
        let event = rx.try_recv().unwrap();
        match event {
            UiEvent::ToolResult {
                session_id,
                id,
                result,
                ..
            } => {
                assert_eq!(session_id, "sess-4");
                assert_eq!(id, "t1");
                assert_eq!(result, "ok");
            }
            other => panic!("Expected ToolResult, got {:?}", other),
        }
    }

    #[test]
    fn turn_begin_and_end_events() {
        let (tx, rx) = mpsc::channel();
        let begin = wire_msg(r#"{"type":"turn_begin","user_input":"hi","index":0}"#);
        dispatch_wire_message(begin, "sess-5", &tx);
        match rx.try_recv().unwrap() {
            UiEvent::TurnStart {
                session_id,
                turn_id,
                user_input,
            } => {
                assert_eq!(session_id, "sess-5");
                assert_eq!(turn_id, "");
                assert_eq!(user_input, "hi");
            }
            other => panic!("Expected TurnStart, got {:?}", other),
        }

        let end = wire_msg(r#"{"type":"turn_end","index":0}"#);
        dispatch_wire_message(end, "sess-5", &tx);
        match rx.try_recv().unwrap() {
            UiEvent::TurnEnd { session_id } => {
                assert_eq!(session_id, "sess-5");
            }
            other => panic!("Expected TurnEnd, got {:?}", other),
        }
    }

    #[test]
    fn status_update_and_view_state_update() {
        let (tx, rx) = mpsc::channel();
        let status =
            wire_msg(r#"{"type":"status_update","message":"Executing tools...","index":0}"#);
        dispatch_wire_message(status, "sess-6", &tx);
        match rx.try_recv().unwrap() {
            UiEvent::StatusUpdate { message, .. } => {
                assert_eq!(message, "Executing tools...");
            }
            other => panic!("Expected StatusUpdate, got {:?}", other),
        }
    }

    #[test]
    fn usage_event_maps_correctly() {
        let (tx, rx) = mpsc::channel();
        let msg = wire_msg(
            r#"{"type":"usage","prompt_tokens":100,"completion_tokens":50,"total_tokens":150,"index":0}"#,
        );
        dispatch_wire_message(msg, "sess-7", &tx);
        let event = rx.try_recv().unwrap();
        match event {
            UiEvent::Usage {
                session_id,
                turn_id,
                prompt_tokens,
                completion_tokens,
                total_tokens,
            } => {
                assert_eq!(session_id, "sess-7");
                assert_eq!(turn_id, "");
                assert_eq!(prompt_tokens, 100);
                assert_eq!(completion_tokens, 50);
                assert_eq!(total_tokens, 150);
            }
            other => panic!("Expected Usage, got {:?}", other),
        }
    }

    #[test]
    fn step_begin_and_compaction_events() {
        let (tx, rx) = mpsc::channel();
        let step = wire_msg(r#"{"type":"step_begin","tool_name":"read_file","index":0}"#);
        dispatch_wire_message(step, "sess-8", &tx);
        match rx.try_recv().unwrap() {
            UiEvent::StepBegin { tool_name, .. } => {
                assert_eq!(tool_name, "read_file");
            }
            other => panic!("Expected StepBegin, got {:?}", other),
        }

        let compact = wire_msg(r#"{"type":"compaction_begin","index":0}"#);
        dispatch_wire_message(compact, "sess-8", &tx);
        match rx.try_recv().unwrap() {
            UiEvent::CompactionBegin { .. } => {}
            other => panic!("Expected CompactionBegin, got {:?}", other),
        }
    }

    #[test]
    fn plan_step_events() {
        let (tx, rx) = mpsc::channel();
        let plan_begin =
            wire_msg(r#"{"type":"plan_step_begin","step_id":"s1","tool_name":"plan","index":0}"#);
        dispatch_wire_message(plan_begin, "sess-9", &tx);
        match rx.try_recv().unwrap() {
            UiEvent::PlanStepBegin {
                step_id, tool_name, ..
            } => {
                assert_eq!(step_id, "s1");
                assert_eq!(tool_name, "plan");
            }
            other => panic!("Expected PlanStepBegin, got {:?}", other),
        }

        let plan_end =
            wire_msg(r#"{"type":"plan_step_end","step_id":"s1","success":true,"index":0}"#);
        dispatch_wire_message(plan_end, "sess-9", &tx);
        match rx.try_recv().unwrap() {
            UiEvent::PlanStepEnd {
                step_id, success, ..
            } => {
                assert_eq!(step_id, "s1");
                assert!(success);
            }
            other => panic!("Expected PlanStepEnd, got {:?}", other),
        }
    }

    #[test]
    fn thread_list_and_thread_created() {
        let (tx, rx) = mpsc::channel();

        let created = wire_msg(
            r#"{"type":"thread_created","thread_id":"t-new","title":"New Thread","index":0}"#,
        );
        dispatch_wire_message(created, "sess-10", &tx);
        match rx.try_recv().unwrap() {
            UiEvent::ThreadCreated { session } => {
                assert_eq!(session.id, "t-new");
                assert_eq!(session.title, "New Thread");
            }
            other => panic!("Expected ThreadCreated, got {:?}", other),
        }
    }

    #[test]
    fn thread_updated_and_draft_events() {
        let (tx, rx) = mpsc::channel();

        let updated = wire_msg(
            r#"{"type":"thread_updated","thread_id":"t-1","title":"Updated","archived":true,"index":0}"#,
        );
        dispatch_wire_message(updated, "sess-11", &tx);
        match rx.try_recv().unwrap() {
            UiEvent::ThreadUpdated {
                thread_id,
                title,
                archived,
            } => {
                assert_eq!(thread_id, "t-1");
                assert_eq!(title, Some("Updated".into()));
                assert_eq!(archived, Some(true));
            }
            other => panic!("Expected ThreadUpdated, got {:?}", other),
        }

        let draft =
            wire_msg(r#"{"type":"draft_event","event":{"kind":"progress","text":"working..."}}"#);
        dispatch_wire_message(draft, "sess-11", &tx);
        match rx.try_recv().unwrap() {
            UiEvent::DraftProgress { text, .. } => {
                assert_eq!(text, "working...");
            }
            other => panic!("Expected DraftProgress, got {:?}", other),
        }
    }

    #[test]
    fn subagent_events_map_to_ui_events() {
        let (tx, rx) = mpsc::channel();

        let stage =
            wire_msg(r#"{"type":"subagent_stage","agent_id":"a1","name":"runner_started"}"#);
        dispatch_wire_message(stage, "sess-12", &tx);
        match rx.try_recv().unwrap() {
            UiEvent::SubagentStage { agent_id, name } => {
                assert_eq!(agent_id, "a1");
                assert_eq!(name, "runner_started");
            }
            other => panic!("Expected SubagentStage, got {:?}", other),
        }

        let progress =
            wire_msg(r#"{"type":"subagent_progress","agent_id":"a1","steps":2,"max_steps":10}"#);
        dispatch_wire_message(progress, "sess-12", &tx);
        match rx.try_recv().unwrap() {
            UiEvent::SubagentProgress {
                agent_id,
                steps,
                max_steps,
            } => {
                assert_eq!(agent_id, "a1");
                assert_eq!(steps, 2);
                assert_eq!(max_steps, 10);
            }
            other => panic!("Expected SubagentProgress, got {:?}", other),
        }
    }

    #[test]
    fn subagent_terminal_status_emits_status_and_complete() {
        let (tx, rx) = mpsc::channel();

        let running = wire_msg(
            r#"{"type":"subagent_status_change","agent_id":"a1","agent_type":"coder","status":"Running"}"#,
        );
        dispatch_wire_message(running, "sess-13", &tx);
        match rx.try_recv().unwrap() {
            UiEvent::SubagentStatus { status, .. } => assert_eq!(status, "Running"),
            other => panic!("Expected SubagentStatus, got {:?}", other),
        }
        assert!(rx.try_recv().is_err(), "Running must not emit Complete");

        let completed = wire_msg(
            r#"{"type":"subagent_status_change","agent_id":"a1","agent_type":"coder","status":"Completed"}"#,
        );
        dispatch_wire_message(completed, "sess-13", &tx);
        match rx.try_recv().unwrap() {
            UiEvent::SubagentStatus { status, .. } => assert_eq!(status, "Completed"),
            other => panic!("Expected SubagentStatus, got {:?}", other),
        }
        match rx.try_recv().unwrap() {
            UiEvent::SubagentComplete { agent_id, success } => {
                assert_eq!(agent_id, "a1");
                assert!(success);
            }
            other => panic!("Expected SubagentComplete, got {:?}", other),
        }
    }

    #[test]
    fn background_task_update_maps_to_ui_event() {
        let (tx, rx) = mpsc::channel();
        let msg = wire_msg(
            r#"{"type":"background_task_update","task_id":"t1","task_name":"demo","status":"completed"}"#,
        );
        dispatch_wire_message(msg, "sess-14", &tx);
        match rx.try_recv().unwrap() {
            UiEvent::BackgroundTaskUpdate {
                task_id,
                task_name,
                status,
            } => {
                assert_eq!(task_id, "t1");
                assert_eq!(task_name, "demo");
                assert_eq!(status, "completed");
            }
            other => panic!("Expected BackgroundTaskUpdate, got {:?}", other),
        }
    }
}
