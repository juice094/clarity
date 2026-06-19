//! UI Store
//!
//! toasts, sidebar, theme, scroll, preview, approvals

use crate::ui::types::*;
use std::time::Instant;

/// Holds ui UI state.
pub struct UiStore {
    /// Deprecated: replaced by `view_state.left_rail_expanded` in S6.
    /// Kept as a placeholder comment; field removed to enforce single source of truth.
    pub network_banner: Option<String>,
    pub frame_count: u64,
    pub last_fps_time: f64,
    pub fps: f64,
    #[allow(dead_code)]
    pub start: Instant,
    pub locale: crate::i18n::Locale,
    pub theme: crate::theme::Theme,
    /// Max content width for the chat area (user-adjustable).
    pub content_max_width: f32,
    /// Current width of the resizable right rail, if it has been measured.
    pub right_rail_width: Option<f32>,
    /// Last frame's scroll offset for virtual list culling.
    pub last_scroll_offset: f32,
    /// Preview item in chat area (file or web page).
    pub preview_item: Option<PreviewItem>,
    /// Timestamp of the most recent input modification.
    pub last_input_modified: Instant,
    /// Pending approval requests from the agent runtime.
    pub pending_approvals: Vec<clarity_core::approval::ApprovalRequest>,
    pub toasts: Vec<Toast>,
    /// Set to `true` when a shortcut requests focus on the chat input.
    /// Cleared by `render_input` after requesting focus.
    pub focus_input_requested: bool,
    /// Toggle Kimi Desktop v3-style conversation rendering.
    pub kimi_conversation_style: bool,
    /// S7 Phase 2D: global selected line index in the flat line stream (line-mode only).
    pub line_cursor_selected: Option<usize>,
    /// S7 Phase 2D: total flat line count last frame (used to clamp cursor).
    pub line_cursor_total_lines: usize,
    /// Cached shell prompt prefix: "cwd branch" (e.g. "clarity main").
    /// Refreshed periodically by App::refresh_shell_prompt.
    pub shell_prompt: String,
    /// OpenClaw: active project name in Work mode.
    pub active_project: Option<String>,
    /// OpenClaw: bot instance registry.
    pub bot_instances: Vec<BotInstance>,
    /// OpenClaw: currently active bot instance id.
    pub active_bot_id: String,
    /// Tracks the active session id from the previous frame to detect switches.
    pub last_active_session_id: String,
    /// Set when a session switch or other state change requires an immediate repaint.
    pub request_repaint: bool,
    /// Claw Gateway: session history lines loaded from the Gateway.
    pub claw_history: Vec<String>,
    /// Pretext PoC: whether the text-measurement probe window is open.
    pub pretext_probe_open: bool,
    /// Pretext PoC: wrap-preview max width in the probe window.
    pub pretext_probe_wrap_width: f32,
    /// Pretext Phase 1: use pretext for `estimate_height` in the message list.
    pub pretext_estimate_enabled: bool,
}

/// Bot instance descriptor (OpenClaw — aligned to Kimi Desktop bot management).
#[derive(Clone, Debug)]
pub struct BotInstance {
    pub id: String,
    pub name: String,
    pub device_id: String,
    pub status: BotStatus,
    pub version: String,
    pub last_backup: String,
}

/// Lifecycle status variants for bot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BotStatus {
    Online,
    Offline,
    Syncing,
}
