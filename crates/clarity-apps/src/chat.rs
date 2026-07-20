//! Chat sub-application.
//!
//! P1c: `ChatApp` and its store now live in `clarity-apps`. The concrete egui
//! rendering still lives in `clarity-egui`; `clarity-apps` only provides the
//! app shell and delegates rendering to the host via [`clarity_shell::ChatRenderer`].
//!
//! ponytail: the chat-specific UI state types below were moved from
//! `clarity-egui::ui::types` so the app can own them without circular
//! dependencies. A future pass can merge overlapping status/attachment types
//! with `clarity-contract` if they become wire formats.

#![allow(missing_docs)]

use clarity_shell::{ClarityApp, ClarityAppContext, ClarityAppResponse};
use std::time::Instant;

// ============================================================================
// Status types
// ============================================================================

/// Lifecycle status variants for agent.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AgentStatus {
    Online,
    Busy,
    Unconfigured,
    Offline,
}

/// Lifecycle status variants for gateway.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum GatewayStatus {
    Online,
    Offline,
    Checking,
}

/// Transient draft indicator state for the current agent turn.
#[derive(Clone, Debug, Default)]
pub enum DraftStatus {
    /// No draft indicator is currently shown.
    #[default]
    None,
    /// Model is still preparing a response. `text` is a short label such as
    /// "thinking...", "analyzing...", or "searching...".
    Progress { text: String },
    /// Optional reasoning content emitted by the model before the final answer.
    /// The UI may choose to show this inline, collapsed, or not at all.
    Content { text: String },
}

// ============================================================================
// Tool call info
// ============================================================================

/// Lifecycle status variants for tool call.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToolCallStatus {
    Running,
    Success,
    Error,
    Warning,
}

/// Holds tool call info state.
#[derive(Clone, Debug)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
    pub status: ToolCallStatus,
    pub result: Option<String>,
}

impl ToolCallInfo {
    /// UI-layer heuristic: infer status from result text when the raw status
    /// is already terminal (non-Running).
    pub fn inferred_status(&self) -> ToolCallStatus {
        match self.status {
            ToolCallStatus::Running => ToolCallStatus::Running,
            _ => {
                if let Some(ref result) = self.result {
                    let lower = result.to_lowercase();
                    if lower.contains("panic")
                        || lower.contains("unreachable")
                        || lower.contains("fatal")
                    {
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
                } else {
                    self.status
                }
            }
        }
    }
}

// ============================================================================
// Attachments and context items
// ============================================================================

/// Holds attachment state.
#[derive(Clone, Debug)]
pub struct Attachment {
    pub path: std::path::PathBuf,
    pub name: String,
}

/// A context item collected via `#` quick-add in the input field.
/// Displayed as a chip/tag and injected as system context before the user
/// message is sent to the LLM.
#[derive(Clone, Debug)]
pub struct ContextItem {
    pub source: ContextSource,
    /// Short display label (e.g. "main.rs:42-58").
    pub display: String,
    /// Actual content injected into the prompt.
    pub payload: String,
}

/// Origin of a context item collected via `#` quick-add.
#[derive(Clone, Debug)]
pub enum ContextSource {
    /// A file (optionally with line range).
    File {
        path: String,
        start_line: Option<usize>,
        end_line: Option<usize>,
    },
    /// A code symbol (function, class, etc.) identified by LSP.
    Code { symbol: String, file: String },
    /// All content from a directory.
    Folder { path: String },
    /// Terminal command output.
    Terminal { command: String },
    /// Web page content.
    Web { url: String },
    // === Extension points (reserved for future backend features) ===
    /// Documentation reference. Reserved.
    Documentation { url: String },
    /// Semantic codebase search result. Reserved.
    Codebase { query: String },
    /// Git diff against a base branch. Reserved for GitHub integration.
    GitDiff { base_branch: String },
}

// ============================================================================
// Plan execution tracker
// ============================================================================

/// Live tracker for an executing plan.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct PlanExecutionTracker {
    pub title: String,
    pub steps: Vec<PlanStepTracker>,
}

/// Holds plan step tracker state.
#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct PlanStepTracker {
    pub id: String,
    pub description: String,
    pub status: PlanStepStatus,
}

/// Lifecycle status variants for plan step.
#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub enum PlanStepStatus {
    Pending,
    Running,
    Success,
    Failed,
    Skipped,
}

// ============================================================================
// Token usage
// ============================================================================

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

// ============================================================================
// Chat store
// ============================================================================

/// Holds chat UI state.
#[derive(Debug)]
pub struct ChatStore {
    pub input: String,
    pub attachments: Vec<Attachment>,
    pub agent_status: AgentStatus,
    pub gateway_status: GatewayStatus,
    pub tool_calls: Vec<ToolCallInfo>,
    /// Queued message to auto-send when current streaming finishes.
    pub pending_send: Option<(String, Vec<Attachment>)>,
    /// Latest token usage for the active session.
    pub last_usage: Option<(u32, u32, u32)>,
    /// Structured token usage for the active session (richer than last_usage).
    pub token_usage: TokenUsage,
    /// Context items collected via # quick-add. Injected before user message on send.
    pub context_items: Vec<ContextItem>,
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
    /// Snapshot of the query last used to populate `find_matches`. Used to
    /// avoid an O(n) scan over all messages every frame when the query
    /// hasn't changed.
    pub find_last_query: String,
    /// When the active turn's `in_flight` flag was set. Used to detect
    /// stuck turns that never receive a Done or Error event.
    pub in_flight_since: Option<std::time::Instant>,
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
            stick_to_bottom: false,
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
            find_last_query: String::new(),
            in_flight_since: None,
        }
    }
}

// ============================================================================
// Chat app
// ============================================================================

/// Chat / conversation sub-application.
#[derive(Debug, Default)]
pub struct ChatApp {
    /// Chat UI state owned by this sub-application.
    pub store: ChatStore,
}

impl ChatApp {
    /// Create a new chat app instance.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ClarityApp for ChatApp {
    fn id(&self) -> &'static str {
        "chat"
    }

    fn title(&self, ctx: &ClarityAppContext<'_>) -> String {
        ctx.state.t("Chat").to_string()
    }

    fn render(
        &mut self,
        ctx: &mut ClarityAppContext<'_>,
        ui: &mut egui::Ui,
        egui_ctx: &egui::Context,
    ) -> ClarityAppResponse {
        // ponytail: P1c Phase 1 — the concrete egui render body is still hosted
        // in `clarity-egui`. The shell calls back into the host via the
        // optional `ChatRenderer` extension point. When no renderer is
        // provided (e.g. headless tests) the app renders nothing.
        let renderer = ctx.state.chat_renderer();
        if let Some(renderer) = renderer {
            renderer.render_chat(self, ui, egui_ctx)
        } else {
            ClarityAppResponse::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_shell::{AppState, ClarityApp};
    use clarity_ui::theme::Theme;

    struct TestState;

    impl AppState for TestState {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
        fn theme(&self) -> &Theme {
            panic!("dummy state has no theme")
        }
        fn theme_mut(&mut self) -> &mut Theme {
            panic!("dummy state has no theme")
        }
    }

    fn test_context<'a>(theme: &'a mut Theme, state: &'a mut TestState) -> ClarityAppContext<'a> {
        ClarityAppContext {
            theme,
            app_name: "Clarity",
            app_version: "0.0.0",
            app_description: "Test",
            app_license: "AGPL-3.0-or-later",
            state,
        }
    }

    #[test]
    fn chat_app_id_and_title() {
        let mut theme = Theme::dark();
        let mut state = TestState;
        let ctx = &mut test_context(&mut theme, &mut state);
        let chat = ChatApp::new();
        assert_eq!(chat.id(), "chat");
        assert_eq!(chat.title(ctx), "Chat");
    }
}
