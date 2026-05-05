//! Domain-specific state stores — Zustand-style slice pattern for egui.
//!
//! Each store owns a vertical slice of UI state.  Panels receive only the
//! store(s) they need, enforcing data boundaries and making dependencies
/// explicit.
use crate::ui::types::*;
use std::collections::HashMap;
use std::time::Instant;

// ============================================================================
// Session Store — session list, active session, drafts, categories
// ============================================================================

pub struct SessionStore {
    pub sessions: Vec<Session>,
    pub active_session_id: String,
    /// Per-session draft buffer. Key = session_id.
    pub drafts: HashMap<String, String>,
    /// Active session category: emotion / knowledge / engineering / tools.
    pub active_category: String,
}

impl SessionStore {
    #[allow(dead_code)]
    pub fn active_session(&self) -> Option<&Session> {
        self.sessions
            .iter()
            .find(|s| s.id == self.active_session_id)
    }

    pub fn active_session_mut(&mut self) -> Option<&mut Session> {
        self.sessions
            .iter_mut()
            .find(|s| s.id == self.active_session_id)
    }
}

// ============================================================================
// Chat Store — input, messages, loading state, tool calls, plans
// ============================================================================

pub struct ChatStore {
    pub input: String,
    pub attachments: Vec<Attachment>,
    pub is_loading: bool,
    pub agent_status: AgentStatus,
    pub gateway_status: crate::ui::types::GatewayStatus,
    pub tool_calls: Vec<ToolCallInfo>,
    pub compacting: bool,
    /// Queued message to auto-send when current streaming finishes.
    pub pending_send: Option<(String, Vec<Attachment>)>,
    /// Visual flag: user pressed Enter while streaming; cancellation is in-flight.
    pub stopping: bool,
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
}

// ============================================================================
// Settings Store — provider config, model selection, add-provider form
// ============================================================================

#[derive(Clone, Debug)]
pub enum KimiCodeLoginState {
    Idle,
    Requesting,
    Waiting {
        user_code: String,
        #[allow(dead_code)]
        verification_uri: String,
        verification_uri_complete: String,
    },
    Polling,
    Success,
    Error(String),
}

pub struct SettingsStore {
    pub settings_open: bool,
    pub settings_edit: crate::settings::GuiSettings,
    #[allow(dead_code)]
    pub settings_vm: clarity_core::view_models::settings::SettingsViewModel,
    pub settings_active_tab: u8,
    pub show_add_provider: bool,
    pub add_provider_name: String,
    pub add_provider_url: String,
    pub add_provider_key: String,
    pub add_provider_format: String,
    pub provider_registry: crate::provider::ProviderRegistry,
    pub testing_provider: Option<String>,
    pub refreshing_provider: Option<String>,
    /// Kimi Code OAuth login modal open state.
    pub kimi_code_login_open: bool,
    /// Current state of the Kimi Code OAuth login flow.
    pub kimi_code_login_state: KimiCodeLoginState,
}

// ============================================================================
// Task Store — background task list, creation modal
// ============================================================================

pub struct TaskStore {
    pub task_panel_open: bool,
    pub tasks: Vec<clarity_core::background::TaskInfo>,
    pub last_task_refresh: Instant,
    pub task_create_modal_open: bool,
    pub task_create_name: String,
    pub task_create_desc: String,
    pub task_create_prompt: String,
    pub task_create_priority: u8,
}

// ============================================================================
// Team Store — team coordination list, creation modal
// ============================================================================

#[derive(Clone, Debug)]
pub struct TeamMember {
    pub name: String,
    pub description: String,
    pub agent_type: String,
}

#[derive(Clone, Debug)]
pub struct Team {
    pub name: String,
    pub goal: String,
    pub members: Vec<TeamMember>,
    pub max_concurrency: usize,
    pub timeout_secs: u64,
}

pub struct TeamStore {
    pub team_panel_open: bool,
    pub teams: Vec<Team>,
    pub create_modal_open: bool,
    pub create_name: String,
    pub create_goal: String,
    pub create_members: Vec<TeamMember>,
    pub create_max_concurrency: usize,
    pub create_timeout_secs: u64,
}

// ============================================================================
// Cron Store — cron-scheduled task list, creation modal
// ============================================================================

pub struct CronStore {
    pub cron_expanded: bool,
    pub tasks: Vec<clarity_core::background::cron::CronTask>,
    #[allow(dead_code)]
    pub last_refresh: Instant,
    pub create_modal_open: bool,
    pub create_name: String,
    pub create_desc: String,
    pub create_prompt: String,
    pub create_expr: String,
    pub create_priority: u8,
}

// ============================================================================
// UI Store — toasts, sidebar, theme, scroll, preview, approvals
// ============================================================================

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
    /// Timestamp of the most recent input modification.
    pub last_input_modified: Instant,
    /// Web tabs managed in the left sidebar.
    pub web_tabs: Vec<WebTab>,
    /// Whether the web tabs section is expanded in the sidebar.
    pub web_tabs_expanded: bool,
    /// Whether the thinking log section is expanded in the sidebar.
    pub thinking_log_expanded: bool,
    /// Whether the thinking log shows all tool calls or only the latest 3.
    pub thinking_log_show_all: bool,
    /// Pending approval requests from the agent runtime.
    pub pending_approvals: Vec<clarity_core::approval::ApprovalRequest>,
    pub toasts: Vec<Toast>,
    /// Skill panel open state.
    pub skill_panel_open: bool,
    /// Right toolbar open state.
    #[allow(dead_code)]
    pub toolbar_open: bool,
    /// Tools section expanded in left sidebar.
    pub tools_expanded: bool,
    /// Subagents section expanded in left sidebar.
    pub subagents_expanded: bool,
    /// Session tab currently being renamed (double-click).
    pub editing_session_id: Option<String>,
    /// Buffer for the in-progress rename.
    pub editing_title: String,
    /// Toggle between legacy per-message bubbles and AgentTurn aggregation.
    pub agent_turn_style: bool,
    /// When agent_turn_style is true, use glass card variant instead of CLI style.
    pub agent_turn_glass: bool,
}

// ============================================================================
// SubAgent Store — parallel batch progress from Gateway
// ============================================================================

pub struct SubAgentStore {
    pub parallel_batches: Vec<SubAgentProgress>,
    pub last_parallel_poll: Instant,
    /// Live single-agent progress tracked via channel (IS-1 Sprint 30).
    pub running_agents: std::collections::HashMap<String, SingleSubagentProgress>,
    /// Last Gateway health check poll time.
    pub last_gateway_health_poll: Instant,
}

// ============================================================================
// MCP Store — MCP server config panel
// ============================================================================

pub struct McpStore {
    pub mcp_panel_open: bool,
    pub mcp_config: Option<clarity_core::mcp::config::McpConfig>,
    pub mcp_changed: bool,
    /// Names of currently connected MCP tools (for hot-reload unregister).
    pub connected_tools: Vec<String>,
    /// Last poll time for MCP config file watcher.
    pub last_mcp_poll: Instant,
    /// Last known mtime of mcp.json.
    pub last_mcp_mtime: Option<std::time::SystemTime>,
}

// ============================================================================
// Onboarding Store — first-run wizard state
// ============================================================================

pub struct OnboardingStore {
    pub onboarding_state: crate::onboarding::OnboardingState,
    pub onboarding_progress_rx:
        Option<std::sync::mpsc::Receiver<clarity_core::model_download::ModelDownloadProgress>>,
    /// Set once when auto-download is triggered to prevent re-triggering every frame.
    pub downloading_auto: bool,
    /// Cancellation token for the active download task (IS-1 Sprint 31).
    pub cancel_token: Option<tokio_util::sync::CancellationToken>,
}

// ============================================================================
// ToolCall reconstruction — rebuild ThinkingLog from persisted message blocks
// ============================================================================

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
