//! Chat Store
//!
//! input, messages, loading state, tool calls, plans

use crate::ui::types::*;

/// Holds chat UI state.
pub struct ChatStore {
    pub input: String,
    pub attachments: Vec<Attachment>,
    pub agent_status: AgentStatus,
    pub gateway_status: crate::ui::types::GatewayStatus,
    pub tool_calls: Vec<ToolCallInfo>,
    /// Queued message to auto-send when current streaming finishes.
    pub pending_send: Option<(String, Vec<Attachment>)>,
    /// Latest token usage for the active session.
    pub last_usage: Option<(u32, u32, u32)>,
    /// Pending plan for user review (Plan mode).
    pub pending_plan: Option<clarity_core::agent::Plan>,
    /// Live execution tracker for an active plan.
    pub plan_tracker: Option<PlanExecutionTracker>,
    /// Whether the chat scroll should stick to bottom (auto-released on manual scroll-up).
    pub stick_to_bottom: bool,
    /// Index of the message currently being edited inline (Sprint 33).
    pub editing_message_idx: Option<usize>,
    /// Temporary buffer for inline message editing.
    pub edit_buffer: String,
    /// Snapshot created by the most recently completed agent turn.
    pub last_snapshot: Option<clarity_core::agent::snapshot::SnapshotInfo>,
    /// Transient draft/thinking indicator for the current agent turn.
    /// Cleared automatically when the turn ends or real content arrives.
    pub draft_status: DraftStatus,
    /// Transient backend status message for the current agent turn.
    /// Examples: "Executing 3 tool(s)...", "Compacting context...".
    pub status_message: Option<String>,
    /// Input history for ↑↓ recall (TUI-style input box Phase 1).
    pub input_history: Vec<String>,
    /// Cursor into `input_history`. None = editing current draft.
    pub input_history_idx: Option<usize>,
}
