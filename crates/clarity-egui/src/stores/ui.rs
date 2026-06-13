//! UI Store
//!
//! toasts, sidebar, theme, scroll, preview, approvals

use crate::ui::types::*;
use std::time::Instant;

/// Holds ui UI state.
pub struct UiStore {
    pub sidebar_collapsed: bool,
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
    /// Last frame's scroll offset for virtual list culling.
    pub last_scroll_offset: f32,
    /// Preview item in chat area (file or web page).
    pub preview_item: Option<PreviewItem>,
    /// Whether the file preview drawer inside the workspace is open.
    pub preview_drawer_open: bool,
    /// Timestamp of the most recent input modification.
    pub last_input_modified: Instant,
    /// Web tabs managed in the left sidebar.
    pub web_tabs: Vec<WebTab>,
    /// Whether the web tabs section is expanded in the sidebar.
    pub web_tabs_expanded: bool,
    /// Whether to show the URL input row when web tabs are empty.
    pub web_tabs_add_visible: bool,
    /// Whether the thinking log section is expanded in the sidebar.
    pub thinking_log_expanded: bool,
    /// Whether the thinking log shows all tool calls or only the latest 3.
    pub thinking_log_show_all: bool,
    /// Pending approval requests from the agent runtime.
    pub pending_approvals: Vec<clarity_core::approval::ApprovalRequest>,
    pub toasts: Vec<Toast>,
    /// Tools section expanded in left sidebar.
    pub tools_expanded: bool,
    /// Subagents section expanded in left sidebar.
    pub subagents_expanded: bool,
    /// Session tab currently being renamed (double-click).
    pub editing_session_id: Option<String>,
    /// Buffer for the in-progress rename.
    pub editing_title: String,
    /// Set to `true` when a shortcut requests focus on the chat input.
    /// Cleared by `render_input` after requesting focus.
    pub focus_input_requested: bool,
    /// Toggle between legacy per-message bubbles and AgentTurn aggregation.
    pub agent_turn_style: bool,
    /// When agent_turn_style is true, use glass card variant instead of CLI style.
    pub agent_turn_glass: bool,
    /// Toggle Kimi Desktop v3-style conversation rendering.
    pub kimi_conversation_style: bool,
    /// Workspace panel Plan section expanded.
    pub workspace_plan_expanded: bool,
    /// User manually collapsed Plan section (blocks auto-expand).
    pub workspace_plan_manually_collapsed: bool,
    /// S7 Phase 2D: global selected line index in the flat line stream (line-mode only).
    pub line_cursor_selected: Option<usize>,
    /// S7 Phase 2D: total flat line count last frame (used to clamp cursor).
    pub line_cursor_total_lines: usize,
    /// Measured width of the titlebar right zone (window controls + capsules).
    /// Updated each frame by `render_titlebar_right` to eliminate hard-coded magic numbers.
    pub titlebar_right_width: f32,
    /// Cached shell prompt prefix: "cwd branch" (e.g. "clarity main").
    /// Refreshed periodically by App::refresh_shell_prompt.
    pub shell_prompt: String,
    /// S8 P3B.1: registry of all Clarity personas (Kin / Analyst / Programmer + future).
    /// Sourced from `clarity_core::endpoint::default_clarity_personas()` at startup.
    /// Read-only at runtime; mutation requires reloading.
    pub endpoint_registry: clarity_core::endpoint::EndpointRegistry,
    /// S8 P3B.1: currently active persona id (e.g. "kin", "analyst", "programmer").
    /// Persisted via `GuiSettings.active_persona_id` and surfaced in the titlebar.
    pub active_persona_id: String,
    /// S8 P3B.1: whether the persona switcher popup is open (titlebar dropdown state).
    pub persona_switcher_open: bool,
    /// OpenClaw: active project name in Work mode.
    pub active_project: Option<String>,
    /// OpenClaw: bot instance registry.
    pub bot_instances: Vec<BotInstance>,
    /// OpenClaw: currently active bot instance id.
    pub active_bot_id: String,
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
