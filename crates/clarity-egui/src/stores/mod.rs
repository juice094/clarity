//! Domain-specific state stores — Zustand-style slice pattern for egui.
//!
//! Each store owns a vertical slice of UI state.  Panels receive only the
//! store(s) they need, enforcing data boundaries and making dependencies
//! explicit.

pub mod chat;
pub mod cron;
pub mod mcp;
pub mod onboarding;
pub mod plugin;
pub mod session;
pub mod settings;
pub mod snapshot;
pub mod subagent;
pub mod task;
pub mod team;
pub mod tool_call;
pub mod ui;

pub use chat::*;
pub use cron::*;
pub use mcp::*;
pub use onboarding::*;
pub use plugin::*;
pub use session::*;
pub use settings::*;
pub use snapshot::*;
pub use subagent::*;
pub use task::*;
pub use team::*;
pub use tool_call::*;
pub use ui::*;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::types::*;

    #[test]
    fn team_store_basic_construction() {
        let store = TeamStore {
            teams: vec![Team {
                name: "alpha".into(),
                goal: "test".into(),
                members: vec![TeamMember {
                    name: "m1".into(),
                    description: "d1".into(),
                    agent_type: "default".into(),
                }],
                max_concurrency: 2,
                timeout_secs: 60,
            }],
            create_name: String::new(),
            create_goal: String::new(),
            create_members: vec![],
            create_max_concurrency: 4,
            create_timeout_secs: 300,
        };
        assert_eq!(store.teams.len(), 1);
        assert_eq!(store.teams[0].members.len(), 1);
    }

    #[test]
    fn infer_tool_status_maps_correctly() {
        assert!(matches!(
            infer_tool_status("operation completed successfully"),
            ToolCallStatus::Success
        ));
        assert!(
            matches!(
                infer_tool_status("error: file not found"),
                ToolCallStatus::Warning
            ),
            "'error' keyword should map to Warning"
        );
        assert!(
            matches!(infer_tool_status("panic at line 42"), ToolCallStatus::Error),
            "'panic' keyword should map to Error"
        );
    }

    #[test]
    fn rebuild_tool_calls_pairs_call_with_result() {
        use crate::ui::types::Role;
        let messages = vec![
            Message {
                role: Role::Agent,
                content: String::new(),
                blocks: vec![ContentBlock::ToolCall {
                    id: "tc-1".into(),
                    name: "ReadFile".into(),
                    args: r#"{"path":"/tmp/test"}"#.into(),
                }],
                timestamp: std::time::Instant::now(),
                parsed: Vec::new(),
                cached_height: None,
                is_error: false,
                lines: Vec::new(),
            },
            Message {
                role: Role::User,
                content: String::new(),
                blocks: vec![ContentBlock::ToolResult {
                    name: "ReadFile".into(),
                    args: None,
                    output: "file content".into(),
                    truncated: false,
                }],
                timestamp: std::time::Instant::now(),
                parsed: Vec::new(),
                cached_height: None,
                is_error: false,
                lines: Vec::new(),
            },
        ];
        let calls = rebuild_tool_calls(&messages);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "ReadFile");
        assert!(
            matches!(calls[0].status, ToolCallStatus::Success),
            "ToolResult with benign output should yield Success"
        );
    }
}
