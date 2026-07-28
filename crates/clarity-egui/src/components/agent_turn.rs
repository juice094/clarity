//! AgentTurn — aggregated rendering unit for a single ReAct cycle.
//!
//! Collects adjacent Agent messages (think → tool_calls → final_response)
//! into one coherent visual unit with a single avatar header.

use crate::ui::types::{ContentBlock, Message, ToolCallStatus};

// ============================================================================
// Data model
// ============================================================================

/// An aggregated rendering unit for a single agent ReAct turn.
#[derive(Clone)]
pub struct AgentTurn {
    /// Turn-level meta (duration / tokens / tool count) shown in the header line.
    pub header: TurnHeader,
    pub thinking: Option<ThinkingBlock>,
    pub tool_calls: Vec<ToolCallRow>,
    pub final_response: Option<Message>,
    /// Reserved for future UI state (e.g. turn-level collapse).
    #[allow(dead_code)]
    pub expanded: bool,
    /// Cached height from last render (for virtual-list estimation).
    pub cached_height: Option<f32>,
}

/// Holds turn header state.
#[derive(Clone, Debug)]
pub struct TurnHeader {
    /// Wall-clock duration of the turn, derived from first/last message
    /// timestamps (approximate — the last message is stamped on arrival).
    pub duration_ms: u64,
    /// Token usage attributed to this turn.
    ///
    /// Filled from the wire `Usage` event via `Message::turn_id` and
    /// `Session::turn_usage`. If the turn predates attribution or no Usage
    /// event arrived, this stays 0 and the header omits the token piece.
    pub token_count: usize,
    /// Number of tool calls in this turn.
    pub tool_count: usize,
}

/// Holds thinking block state.
#[derive(Clone, Debug)]
pub struct ThinkingBlock {
    pub steps: Vec<String>,
    pub token_hint: usize,
}

/// Holds tool call row state.
#[derive(Clone, Debug)]
pub struct ToolCallRow {
    pub name: String,
    pub status: ToolCallStatus,
    pub result_preview: String,
    /// Whether the in-place detail view (full arguments + output) is open.
    pub expanded: bool,
    /// Full argument payload (JSON string) for the expanded detail view.
    pub arguments: Option<String>,
    /// Full tool output for the expanded detail view.
    pub full_output: String,
}

// ============================================================================
// Construction
// ============================================================================

impl AgentTurn {
    /// Build an `AgentTurn` from a contiguous slice of Agent messages.
    ///
    /// `turn_usage` maps backend turn ids to total token counts; it is used to
    /// attribute per-turn usage to the rendered header.
    pub fn from_messages(
        messages: &[Message],
        turn_usage: &std::collections::HashMap<String, u32>,
    ) -> Self {
        let mut thinking = None;
        let mut tool_calls = Vec::new();

        // Extract think blocks and tool results from all messages in the turn.
        for msg in messages.iter() {
            for block in &msg.blocks {
                match block {
                    ContentBlock::Think { steps } => {
                        let hint = steps.iter().map(|s| s.split_whitespace().count()).sum();
                        thinking = Some(ThinkingBlock {
                            steps: steps.clone(),
                            token_hint: hint,
                        });
                    }
                    ContentBlock::ToolResult {
                        name, args, output, ..
                    } => {
                        tool_calls.push(ToolCallRow {
                            name: name.clone(),
                            status: infer_tool_status(output),
                            result_preview: crate::ui::truncate::truncate(output, 120),
                            expanded: false,
                            arguments: args.clone(),
                            full_output: output.clone(),
                        });
                    }
                    _ => {}
                }
            }
        }

        // Final response = the last message that carries substantive reply content.
        let final_response = messages
            .iter()
            .rev()
            .find(|m| {
                !m.content.trim().is_empty()
                    || m.blocks.iter().any(|b| {
                        matches!(
                            b,
                            ContentBlock::Text { .. }
                                | ContentBlock::Code { .. }
                                | ContentBlock::Plan { .. }
                                | ContentBlock::FilePreview { .. }
                        )
                    })
            })
            .cloned();

        // Turn duration ≈ wall time between the first and last message of the
        // turn (messages are stamped on arrival).
        let duration_ms = match (messages.first(), messages.last()) {
            (Some(first), Some(last)) => last
                .timestamp
                .saturating_duration_since(first.timestamp)
                .as_millis() as u64,
            _ => 0,
        };

        // Per-turn token attribution: any message in the turn carries the
        // backend turn id; look up the total token count reported by Usage.
        let token_count = messages
            .iter()
            .find(|m| !m.turn_id.is_empty())
            .and_then(|m| turn_usage.get(&m.turn_id).copied())
            .unwrap_or(0) as usize;

        Self {
            header: TurnHeader {
                duration_ms,
                token_count,
                tool_count: tool_calls.len(),
            },
            thinking,
            tool_calls,
            final_response,
            expanded: true,
            cached_height: None,
        }
    }

    /// Rough height estimation for virtual-list culling.
    pub fn estimate_height(
        &self,
        content_max_width: f32,
        theme: &crate::theme::Theme,
        metrics: &crate::pretext::EguiFontMetrics,
    ) -> f32 {
        let mut h = 44.0; // header + spacing
        if self.thinking.is_some() {
            h += 28.0; // collapsed header
        }
        for _ in &self.tool_calls {
            h += 32.0; // each tool row
        }
        if let Some(ref msg) = self.final_response {
            h += crate::ui::render::estimate_height(msg, content_max_width, theme, metrics);
        }
        h += theme.space_16;
        h
    }
}

// ============================================================================
// Helpers
// ============================================================================

fn infer_tool_status(result: &str) -> ToolCallStatus {
    let lower = result.to_lowercase();
    if lower.contains("panic") || lower.contains("unreachable") || lower.contains("fatal") {
        ToolCallStatus::Error
    } else if lower.contains("error")
        || lower.contains("failed")
        || lower.contains("fail")
        || lower.contains("exception")
    {
        ToolCallStatus::Warning
    } else {
        ToolCallStatus::Success
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::types::Role;
    use std::time::{Duration, Instant};

    fn tool_result_message(
        name: &str,
        args: Option<&str>,
        output: &str,
        timestamp: Instant,
    ) -> Message {
        Message {
            role: Role::Agent,
            content: String::new(),
            blocks: vec![ContentBlock::ToolResult {
                name: name.to_string(),
                args: args.map(str::to_string),
                output: output.to_string(),
                truncated: false,
            }],
            timestamp,
            parsed: vec![],
            cached_height: None,
            is_error: false,
            lines: vec![],
            turn_id: String::new(),
        }
    }

    #[test]
    fn from_messages_captures_full_args_and_output() {
        let now = Instant::now();
        let messages = [tool_result_message(
            "read_file",
            Some("{\"path\":\"src/main.rs\"}"),
            "fn main() {}\n".repeat(500).as_str(),
            now,
        )];
        let turn = AgentTurn::from_messages(&messages, &std::collections::HashMap::new());
        assert_eq!(turn.tool_calls.len(), 1);
        let row = &turn.tool_calls[0];
        assert_eq!(row.arguments.as_deref(), Some("{\"path\":\"src/main.rs\"}"));
        assert_eq!(row.full_output.len(), 500 * "fn main() {}\n".len());
        assert!(
            row.result_preview.len() < row.full_output.len(),
            "preview stays truncated while full output is kept"
        );
        assert!(!row.expanded);
    }

    #[test]
    fn from_messages_wires_tool_count_and_duration() {
        let now = Instant::now();
        let messages = [
            tool_result_message("a", None, "ok", now - Duration::from_millis(1500)),
            tool_result_message("b", None, "ok", now),
        ];
        let turn = AgentTurn::from_messages(&messages, &std::collections::HashMap::new());
        assert_eq!(turn.header.tool_count, 2);
        assert!(
            turn.header.duration_ms >= 1500,
            "duration should cover the first→last message span, got {}",
            turn.header.duration_ms
        );
        // ponytail: per-turn token usage is not wired yet (see TurnHeader docs).
        assert_eq!(turn.header.token_count, 0);
    }
}
