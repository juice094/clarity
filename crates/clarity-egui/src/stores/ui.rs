//! UI Store
//!
//! toasts, sidebar, theme, scroll, preview, approvals

use crate::ui::types::*;
use std::collections::HashMap;
use std::time::{Duration, Instant};

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
    /// Gateway-assigned session id when connected via the native WebSocket protocol.
    pub claw_gateway_session_id: String,
    /// Pretext PoC: whether the text-measurement probe window is open.
    pub pretext_probe_open: bool,
    /// Pretext PoC: wrap-preview max width in the probe window.
    pub pretext_probe_wrap_width: f32,
    /// Pretext Phase 1: use pretext for `estimate_height` in the message list.
    pub pretext_estimate_enabled: bool,
    /// Per-device connection failure backoff: device_id -> (failure_count, next_retry_at).
    pub claw_connect_backoff: HashMap<String, (usize, Instant)>,
    /// Last surfaced Claw error message and timestamp, used to suppress
    /// duplicate toast/chat spam while a device is flapping.
    pub last_claw_error: Option<(String, Instant)>,
    /// Input buffer for the role-context E2EE passphrase in the Claw settings panel.
    pub claw_role_passphrase_input: String,
}

impl Default for UiStore {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            network_banner: None,
            frame_count: 0,
            last_fps_time: 0.0,
            fps: 0.0,
            start: now,
            locale: crate::i18n::Locale::default(),
            theme: crate::theme::Theme::default(),
            content_max_width: 0.0,
            right_rail_width: None,
            last_scroll_offset: 0.0,
            preview_item: None,
            last_input_modified: now,
            pending_approvals: Vec::new(),
            toasts: Vec::new(),
            focus_input_requested: false,
            kimi_conversation_style: false,
            line_cursor_selected: None,
            line_cursor_total_lines: 0,
            shell_prompt: String::new(),
            active_project: None,
            bot_instances: Vec::new(),
            active_bot_id: String::new(),
            last_active_session_id: String::new(),
            request_repaint: false,
            claw_history: Vec::new(),
            claw_gateway_session_id: String::new(),
            pretext_probe_open: false,
            pretext_probe_wrap_width: 400.0,
            pretext_estimate_enabled: true,
            claw_connect_backoff: HashMap::new(),
            last_claw_error: None,
            claw_role_passphrase_input: String::new(),
        }
    }
}

impl UiStore {
    /// Check whether a device is currently in a connection backoff window.
    pub fn is_in_backoff(&self, device_id: &str) -> Option<Duration> {
        self.claw_connect_backoff
            .get(device_id)
            .and_then(|(_, instant)| instant.checked_duration_since(Instant::now()))
    }

    /// Record a connection failure for a device and return the failure count
    /// and the next retry instant.
    pub fn record_connect_failure(&mut self, device_id: &str) -> (usize, Instant) {
        let now = Instant::now();
        let (count, retry) = self
            .claw_connect_backoff
            .entry(device_id.into())
            .or_insert((0, now));
        *count = count.saturating_add(1);
        let delay_secs = (2usize.saturating_pow(*count as u32)).min(30);
        *retry = now + Duration::from_secs(delay_secs as u64);
        (*count, *retry)
    }

    /// Clear backoff for a device after a successful connection.
    pub fn clear_connect_backoff(&mut self, device_id: &str) {
        self.claw_connect_backoff.remove(device_id);
    }

    /// Decide whether a Claw error should be surfaced to the user right now.
    ///
    /// Repeated identical errors within a short window are suppressed to avoid
    /// toast/chat spam while a device is flapping or the Gateway is unreachable.
    pub fn should_surface_claw_error(&mut self, msg: &str) -> bool {
        let now = Instant::now();
        let suppress_window = Duration::from_secs(5);
        if let Some((last_msg, last_time)) = self.last_claw_error.as_ref() {
            if last_msg == msg && last_time.elapsed() < suppress_window {
                return false;
            }
        }
        self.last_claw_error = Some((msg.to_string(), now));
        true
    }
}

/// Bot instance descriptor (OpenClaw — aligned to Kimi Desktop bot management).
#[derive(Clone, Debug)]
pub struct BotInstance {
    pub id: String,
    pub name: String,
    pub device_id: String,
    /// Claw role used when creating sessions bound to this device.
    pub role: String,
    pub status: BotStatus,
    pub version: String,
    pub last_backup: String,
    /// Optional session key override for this bot's role.
    ///
    /// When set, new Claw sessions for this role use this key instead of the
    /// default `agent:main:<role>` key.
    pub session_key: Option<String>,
}

/// Lifecycle status variants for bot.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(dead_code)]
pub enum BotStatus {
    Online,
    Offline,
    Syncing,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connect_backoff_increases() {
        let mut store = UiStore::default();
        let (_, first) = store.record_connect_failure("dev");
        let first_delay = first.duration_since(Instant::now());
        let (_, second) = store.record_connect_failure("dev");
        let second_delay = second.duration_since(Instant::now());
        let (_, third) = store.record_connect_failure("dev");
        let third_delay = third.duration_since(Instant::now());

        assert!(second_delay >= first_delay, "delay should increase");
        assert!(third_delay >= second_delay, "delay should keep increasing");
        assert!(
            store
                .is_in_backoff("dev")
                .expect("device should be in backoff")
                .as_secs()
                <= 30,
            "delay should be capped at 30s"
        );
    }

    #[test]
    fn test_connect_backoff_cleared_on_success() {
        let mut store = UiStore::default();
        store.record_connect_failure("dev");
        assert!(store.is_in_backoff("dev").is_some());
        store.clear_connect_backoff("dev");
        assert!(store.is_in_backoff("dev").is_none());
    }

    #[test]
    fn test_connect_backoff_five_failures_offline() {
        let mut store = UiStore::default();
        let mut count = 0;
        let mut last = Instant::now();
        for _ in 0..5 {
            let (c, next) = store.record_connect_failure("dev");
            count = c;
            last = next;
        }
        assert_eq!(count, 5);
        let delay = last.duration_since(Instant::now());
        assert!((29..=30).contains(&delay.as_secs()), "cap delay at 30s");
    }

    #[test]
    fn test_should_surface_claw_error_suppresses_duplicates() {
        let mut store = UiStore::default();
        let msg = "OpenClaw connection error: refused";
        assert!(store.should_surface_claw_error(msg));
        assert!(!store.should_surface_claw_error(msg));
        assert!(store.should_surface_claw_error("different error"));
    }
}
