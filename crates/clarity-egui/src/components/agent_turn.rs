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

#[derive(Clone, Debug)]
pub struct TurnHeader {
    /// Reserved — will be wired to runtime telemetry.
    #[allow(dead_code)]
    pub duration_ms: u64,
    /// Reserved — will be wired to token-usage events.
    #[allow(dead_code)]
    pub token_count: usize,
    pub tool_count: usize,
}

#[derive(Clone, Debug)]
pub struct ThinkingBlock {
    pub steps: Vec<String>,
    pub token_hint: usize,
}

#[derive(Clone, Debug)]
pub struct ToolCallRow {
    pub name: String,
    pub status: ToolCallStatus,
    pub result_preview: String,
    /// Reserved for future expand-in-place detail view.
    #[allow(dead_code)]
    pub expanded: bool,
}

// ============================================================================
// Construction
// ============================================================================

impl AgentTurn {
    /// Build an `AgentTurn` from a contiguous slice of Agent messages.
    pub fn from_messages(messages: &[Message]) -> Self {
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
                    ContentBlock::ToolResult { name, output, .. } => {
                        tool_calls.push(ToolCallRow {
                            name: name.clone(),
                            status: infer_tool_status(output),
                            result_preview: truncate(output, 120),
                            expanded: false,
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

        Self {
            header: TurnHeader {
                duration_ms: 0,
                token_count: 0,
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
    pub fn estimate_height(&self, content_max_width: f32, theme: &crate::theme::Theme) -> f32 {
        let mut h = 44.0; // header + spacing
        if self.thinking.is_some() {
            h += 28.0; // collapsed header
        }
        for _ in &self.tool_calls {
            h += 32.0; // each tool row
        }
        if let Some(ref msg) = self.final_response {
            h += crate::ui::render::estimate_height(msg, content_max_width, theme);
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

/// Truncate a string to at most `max_chars` characters, appending "…" if truncated.
fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max_chars).collect::<String>())
    }
}
