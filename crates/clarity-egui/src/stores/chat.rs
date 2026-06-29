//! Chat Store
//!
//! input, messages, loading state, tool calls, plans

use crate::ui::types::*;
use std::time::Instant;

/// Live token usage snapshot for the active session.
#[derive(Clone, Debug)]
#[allow(dead_code)] // Extension-point fields reserved for future pricing/tracking
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
    /// Model context window size. Defaults to 128K; set from provider
    /// config when available.
    pub context_limit: u64,
    /// When this snapshot was last updated.
    pub last_updated: Instant,
    // === Extension points ===
    /// Estimated cost in USD. Reserved for future pricing integration.
    pub cost_estimate: Option<f64>,
}

impl Default for TokenUsage {
    fn default() -> Self {
        Self {
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            context_limit: 128_000,
            last_updated: Instant::now(),
            cost_estimate: None,
        }
    }
}

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
    /// Structured token usage for the active session (richer than last_usage).
    pub token_usage: TokenUsage,
    /// Context items collected via # quick-add. Injected before user message on send.
    pub context_items: Vec<crate::ui::types::ContextItem>,
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
    /// Session id that owns the current Claw streaming turn. Used because
    /// WebSocket responses are drained centrally in main.rs and must be
    /// routed back to the originating session.
    pub claw_in_flight_session_id: Option<String>,
    /// Counter for incremental session persistence during streaming.
    pub chunks_since_save: usize,
    /// Input history for ↑↓ recall (TUI-style input box Phase 1).
    pub input_history: Vec<String>,
    /// Cursor into `input_history`. None = editing current draft.
    pub input_history_idx: Option<usize>,
    /// Whether the find-in-session bar is open (Ctrl+F).
    pub find_open: bool,
    /// Current find query string.
    pub find_query: String,
    /// Indices of messages that match the current query.
    pub find_matches: Vec<usize>,
    /// Index into `find_matches` of the currently highlighted match.
    pub find_current: usize,
}

impl Default for ChatStore {
    fn default() -> Self {
        Self {
            input: String::new(),
            attachments: Vec::new(),
            agent_status: AgentStatus::Online,
            gateway_status: GatewayStatus::Online,
            tool_calls: Vec::new(),
            pending_send: None,
            last_usage: None,
            token_usage: TokenUsage::default(),
            context_items: Vec::new(),
            pending_plan: None,
            plan_tracker: None,
            stick_to_bottom: true,
            editing_message_idx: None,
            edit_buffer: String::new(),
            last_snapshot: None,
            draft_status: DraftStatus::None,
            status_message: None,
            claw_in_flight_session_id: None,
            chunks_since_save: 0,
            input_history: Vec::new(),
            input_history_idx: None,
            find_open: false,
            find_query: String::new(),
            find_matches: Vec::new(),
            find_current: 0,
        }
    }
}
