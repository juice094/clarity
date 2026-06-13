//! ToolCall reconstruction
//!
//! rebuild ThinkingLog from persisted message blocks

use crate::ui::types::*;

/// Heuristic: infer terminal status from result text.
pub fn infer_tool_status(output: &str) -> ToolCallStatus {
    let lower = output.to_lowercase();
    if lower.contains("panic") || lower.contains("unreachable") || lower.contains("fatal") {
        return ToolCallStatus::Error;
    }
    if lower.contains("error")
        || lower.contains("failed")
        || lower.contains("fail")
        || lower.contains("exception")
    {
        return ToolCallStatus::Warning;
    }
    ToolCallStatus::Success
}

/// Rebuild `ToolCallInfo` list from session message `blocks`.
/// Used after loading a session or switching tabs so the Thinking Log
/// sidebar is repopulated without a separate persistence channel.
pub fn rebuild_tool_calls(messages: &[Message]) -> Vec<ToolCallInfo> {
    let mut calls = Vec::new();
    for msg in messages {
        for block in &msg.blocks {
            match block {
                ContentBlock::ToolCall { id, name, args } => {
                    calls.push(ToolCallInfo {
                        id: id.clone(),
                        name: name.clone(),
                        status: ToolCallStatus::Running,
                        result: Some(args.clone()),
                    });
                }
                ContentBlock::ToolResult { name, output, .. } => {
                    if let Some(tc) = calls
                        .iter_mut()
                        .rev()
                        .find(|t| t.name == *name && matches!(t.status, ToolCallStatus::Running))
                    {
                        tc.status = infer_tool_status(output);
                        tc.result = Some(output.clone());
                    }
                }
                _ => {}
            }
        }
    }
    calls
}
