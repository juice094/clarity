#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::items_after_test_module,
        missing_docs,
        unsafe_code
    )
)]
//! Clarity egui Desktop — Application entry point.
//!
//! ARCHITECTURE CONSTRAINT (Pretext-aligned):
//!   - `update()` is the HOT PATH: only iteration, arithmetic, and egui calls.
//!   - String parsing / markdown / I/O / JSON is FORBIDDEN in `update()`.
//!   - Virtual list: `last_scroll_offset` + `estimate_height()` → visible range.
//!   - `Message::prepare()` must be called ONCE after every content mutation.
//!
//! See `crates/clarity-egui/ARCHITECTURE.md` §1–§6.

use clarity_chrome::Chrome;
use eframe::egui;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use chrono::Utc;

mod app_context;
mod app_state;
pub(crate) mod apps;
mod chrome;
pub(crate) mod claw;
use crate::claw::normalize_gateway_url;
mod components;
mod design_system;
mod error;
mod handlers;
mod i18n;
mod layout;
mod llm_binder;
mod llm_loader;
mod llm_policy;
mod panels;
mod platform;
mod pretext;
mod provider;
mod render;
mod services;
mod session;
mod settings;
mod shortcuts;
mod stores;
mod theme;
mod ui;
mod widgets;
mod window_manager;

use crate::app_context::AppContext;
use crate::stores::{ChatStore, FocusTarget, SessionStore, SettingsStore, UiStore};
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::text_input::TextInput;
use ui::types::*;

// ============================================================================
// Clarity egui Desktop — Phase A: Design System Foundation
// ============================================================================

// (layout constants moved to Theme tokens)

/// Holds app state.
pub(crate) struct App {
    /// Shared runtime services and domain stores.
    pub(crate) context: AppContext,
    /// UI event receiver; only the root `App` drains this channel.
    pub(crate) ui_rx: Receiver<UiEvent>,
    /// Pretext UI unified view state (replaces boolean flag hell).
    pub(crate) view_state: clarity_core::ui::ViewState,
    /// Main view router (Chat / Settings / Dashboard history).
    pub(crate) main_router: clarity_core::ui::Router<clarity_core::ui::AppView>,
    /// Modal router (blocking dialogs).
    pub(crate) modal_router: clarity_core::ui::Router<clarity_core::ui::ModalType>,
    /// Right rail panel router (IDE-style side panel).
    pub(crate) right_rail_router: clarity_core::ui::Router<clarity_core::ui::RightRailPanel>,
    /// Keyboard shortcuts reference modal (Ctrl+/).
    pub(crate) shortcuts_help_open: bool,
    /// Pretext UI command palette (Ctrl+Shift+P).
    pub(crate) command_palette: crate::widgets::command_palette::CommandPalette,
    /// Pretext text measurement backend backed by egui fonts.
    pub(crate) pretext_metrics: crate::pretext::EguiFontMetrics,
    /// Panel transition animation state.
    pub(crate) panel_animation: crate::animation::PanelAnimationState,
    /// Main view transition animation.
    pub(crate) main_stage_transition: Option<crate::animation::MainStageTransition>,
    /// Previous main view, used to detect route changes.
    pub(crate) prev_main_view: clarity_core::ui::AppView,
    /// Sub-applications indexed by main view: 0=Chat, 1=Settings, 2=Dashboard.
    ///
    /// P1d: `ClarityAppEnum` unifies dispatch in `chrome::render_main_stage`.
    pub(crate) apps: [clarity_apps::ClarityAppEnum; 3],
    /// Generic chrome shell that orchestrates titlebar, rails, main stage, overlays and modals.
    pub(crate) chrome: Option<Chrome<App, crate::chrome::AppChromeRenderer>>,
    /// When true, the next close request should be honoured (Quit from tray menu).
    pub(crate) tray_quit_requested: bool,
    /// Last tray status to avoid redundant icon updates every frame.
    pub(crate) last_tray_status: Option<crate::services::tray::TrayIconStatus>,
    /// Last frame's screen width for responsive breakpoint detection.
    last_frame_width: Option<f32>,
}

mod animation;
mod app_logic;
mod onboarding;

impl App {
    // ── P1d accessors for the unified `apps` array ──
    // Index 0 is Chat, 1 is Settings, 2 is Dashboard. These helpers centralise
    // the invariant so the rest of the crate does not repeat array indices.

    pub(crate) fn chat_app(&self) -> &clarity_apps::ChatApp {
        match &self.apps[0] {
            clarity_apps::ClarityAppEnum::Chat(app) => app,
            _ => unreachable!("apps[0] is Chat"),
        }
    }

    pub(crate) fn chat_app_mut(&mut self) -> &mut clarity_apps::ChatApp {
        match &mut self.apps[0] {
            clarity_apps::ClarityAppEnum::Chat(app) => app,
            _ => unreachable!("apps[0] is Chat"),
        }
    }

    pub(crate) fn chat_store(&self) -> &clarity_apps::chat::ChatStore {
        &self.chat_app().store
    }

    pub(crate) fn chat_store_mut(&mut self) -> &mut clarity_apps::chat::ChatStore {
        &mut self.chat_app_mut().store
    }

    pub(crate) fn settings_app(&self) -> &clarity_apps::SettingsApp {
        match &self.apps[1] {
            clarity_apps::ClarityAppEnum::Settings(app) => app,
            _ => unreachable!("apps[1] is Settings"),
        }
    }

    pub(crate) fn settings_app_mut(&mut self) -> &mut clarity_apps::SettingsApp {
        match &mut self.apps[1] {
            clarity_apps::ClarityAppEnum::Settings(app) => app,
            _ => unreachable!("apps[1] is Settings"),
        }
    }

    pub(crate) fn settings_store(&self) -> &clarity_apps::SettingsStore {
        &self.settings_app().store
    }

    pub(crate) fn settings_store_mut(&mut self) -> &mut clarity_apps::SettingsStore {
        &mut self.settings_app_mut().store
    }

    pub(crate) fn dashboard_app(&self) -> &clarity_apps::DashboardApp {
        match &self.apps[2] {
            clarity_apps::ClarityAppEnum::Dashboard(app) => app,
            _ => unreachable!("apps[2] is Dashboard"),
        }
    }

    pub(crate) fn dashboard_app_mut(&mut self) -> &mut clarity_apps::DashboardApp {
        match &mut self.apps[2] {
            clarity_apps::ClarityAppEnum::Dashboard(app) => app,
            _ => unreachable!("apps[2] is Dashboard"),
        }
    }

    pub(crate) fn task_store(&self) -> &clarity_apps::TaskStore {
        &self.dashboard_app().task_store
    }

    pub(crate) fn task_store_mut(&mut self) -> &mut clarity_apps::TaskStore {
        &mut self.dashboard_app_mut().task_store
    }

    pub(crate) fn cron_store_mut(&mut self) -> &mut clarity_apps::CronStore {
        &mut self.dashboard_app_mut().cron_store
    }

    pub(crate) fn subagent_store(&self) -> &clarity_apps::SubAgentStore {
        &self.dashboard_app().subagent_store
    }

    pub(crate) fn subagent_store_mut(&mut self) -> &mut clarity_apps::SubAgentStore {
        &mut self.dashboard_app_mut().subagent_store
    }

    pub(crate) fn team_store_mut(&mut self) -> &mut clarity_apps::TeamStore {
        &mut self.dashboard_app_mut().team_store
    }

    // ── P1d split-borrow helpers ──
    // `chat_store_mut()` / `settings_store_mut()` borrow the whole `App` for the
    // lifetime of the returned reference, which collides with any simultaneous
    // borrow of `app.context`. These helpers return references from disjoint
    // fields (`context` vs `apps`) so event handlers can pass both stores at
    // once. They are intentionally scoped at call sites.

    /// Mutable session store + mutable chat store (disjoint fields).
    pub(crate) fn chat_session_both_mut(&mut self) -> (&mut SessionStore, &mut ChatStore) {
        let session_store = &mut self.context.session_store;
        let chat_store = match &mut self.apps[0] {
            clarity_apps::ClarityAppEnum::Chat(app) => &mut app.store,
            _ => unreachable!("apps[0] is Chat"),
        };
        (session_store, chat_store)
    }

    /// Immutable session store + mutable chat store (disjoint fields).
    pub(crate) fn chat_session_mut(&mut self) -> (&SessionStore, &mut ChatStore) {
        let session_store = &self.context.session_store;
        let chat_store = match &mut self.apps[0] {
            clarity_apps::ClarityAppEnum::Chat(app) => &mut app.store,
            _ => unreachable!("apps[0] is Chat"),
        };
        (session_store, chat_store)
    }

    /// Mutable chat store + mutable view state (disjoint fields).
    pub(crate) fn chat_and_view_state_mut(
        &mut self,
    ) -> (&mut ChatStore, &mut clarity_core::ui::ViewState) {
        let chat_store = match &mut self.apps[0] {
            clarity_apps::ClarityAppEnum::Chat(app) => &mut app.store,
            _ => unreachable!("apps[0] is Chat"),
        };
        let view_state = &mut self.view_state;
        (chat_store, view_state)
    }

    /// Mutable settings store + mutable UI store (disjoint fields).
    pub(crate) fn settings_and_ui_mut(&mut self) -> (&mut SettingsStore, &mut UiStore) {
        let settings_store = match &mut self.apps[1] {
            clarity_apps::ClarityAppEnum::Settings(app) => &mut app.store,
            _ => unreachable!("apps[1] is Settings"),
        };
        let ui_store = &mut self.context.ui_store;
        (settings_store, ui_store)
    }
}

/// Re-export the shell-level pairing state so the rest of the egui crate can
/// keep using the unqualified `PairingState` name.
pub(crate) use clarity_shell::PairingState;

/// Return true if the URL's host is localhost/127.0.0.1.
pub(crate) fn is_localhost_host(url: &str) -> bool {
    crate::claw::normalize_gateway_url(url)
        .trim_start_matches("ws://")
        .trim_start_matches("wss://")
        .trim_start_matches("http://")
        .trim_start_matches("https://")
        .split(':')
        .next()
        .map(|h| h == "localhost" || h == "127.0.0.1")
        .unwrap_or(false)
}

impl clarity_shell::AppState for App {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn theme(&self) -> &clarity_ui::theme::Theme {
        &self.context.ui_store.theme
    }
    fn theme_mut(&mut self) -> &mut clarity_ui::theme::Theme {
        &mut self.context.ui_store.theme
    }
    fn t(&self, key: &'static str) -> &'static str {
        App::t(self, key)
    }

    fn session_message_count(&self) -> usize {
        self.context
            .session_store
            .active_session()
            .map(|s| s.messages.len())
            .unwrap_or(0)
    }

    fn session_tool_call_count(&self) -> usize {
        self.chat_store().tool_calls.len()
    }

    fn session_token_count(&self) -> Option<u32> {
        self.chat_store().last_usage.map(|(_, _, t)| t)
    }

    fn agent_status_label(&self) -> &'static str {
        match self.chat_store().agent_status {
            AgentStatus::Online => "Online",
            AgentStatus::Busy => "Busy",
            AgentStatus::Offline => "Offline",
            AgentStatus::Unconfigured => "Unconfigured",
        }
    }

    fn agent_status_color(&self) -> egui::Color32 {
        let theme = self.theme();
        match self.chat_store().agent_status {
            AgentStatus::Online => theme.status_online,
            AgentStatus::Busy => theme.status_busy,
            AgentStatus::Offline => theme.status_offline,
            AgentStatus::Unconfigured => theme.text_dim,
        }
    }

    fn gateway_status_label(&self) -> &'static str {
        match self.chat_store().gateway_status {
            GatewayStatus::Online => "Online",
            GatewayStatus::Offline => "Offline",
            GatewayStatus::Checking => "Checking",
        }
    }

    fn gateway_status_color(&self) -> egui::Color32 {
        let theme = self.theme();
        match self.chat_store().gateway_status {
            GatewayStatus::Online => theme.status_online,
            GatewayStatus::Offline => theme.status_offline,
            GatewayStatus::Checking => theme.status_busy,
        }
    }

    fn active_provider(&self) -> &str {
        &self.settings_store().settings_edit.provider
    }

    fn active_model(&self) -> &str {
        &self.settings_store().settings_edit.model
    }

    fn fps(&self) -> f64 {
        self.context.ui_store.fps
    }

    fn chat_renderer(&mut self) -> Option<&mut dyn clarity_shell::ChatRenderer> {
        Some(self)
    }

    // ── Settings surface host hooks (P1c) ──

    fn navigate(&mut self, route: clarity_core::ui::Route) {
        App::navigate(self, route);
    }

    fn push_toast(&mut self, message: String, level: clarity_shell::ToastLevel) {
        let level = match level {
            clarity_shell::ToastLevel::Info => ToastLevel::Info,
            clarity_shell::ToastLevel::Warn => ToastLevel::Warn,
            clarity_shell::ToastLevel::Error => ToastLevel::Error,
        };
        App::push_toast(self, message, level);
    }

    fn open_modal(&mut self, modal: clarity_core::ui::ModalType) {
        App::open_modal(self, modal);
    }

    fn set_theme(&mut self, theme: clarity_ui::theme::Theme) {
        App::set_theme_with_transition(self, theme);
    }

    fn set_font_scale(&mut self, scale: f32) {
        App::set_font_scale(self, scale);
    }

    fn increase_font_scale(&mut self) {
        App::increase_font_scale(self);
    }

    fn decrease_font_scale(&mut self) {
        App::decrease_font_scale(self);
    }

    fn persist_layout_settings(&mut self) {
        App::persist_layout_settings(self);
    }

    fn auto_save_settings(&mut self) {
        App::auto_save_settings(self);
    }

    fn content_max_width(&self) -> f32 {
        self.context.ui_store.content_max_width
    }

    fn set_content_max_width(&mut self, width: f32) {
        self.context.ui_store.content_max_width = width;
    }

    fn debug_layout_overlay(&self) -> bool {
        self.view_state.debug_layout_overlay
    }

    fn set_debug_layout_overlay(&mut self, value: bool) {
        self.view_state.debug_layout_overlay = value;
    }

    fn locale(&self) -> clarity_ui::i18n::Locale {
        self.context.ui_store.locale
    }

    fn set_locale(&mut self, locale: clarity_ui::i18n::Locale) {
        self.context.ui_store.locale = locale;
    }

    fn set_pretext_probe_open(&mut self, open: bool) {
        self.context.ui_store.pretext_probe_open = open;
    }

    fn pretext_estimate_enabled(&self) -> bool {
        self.context.ui_store.pretext_estimate_enabled
    }

    fn set_pretext_estimate_enabled(&mut self, enabled: bool) {
        self.context.ui_store.pretext_estimate_enabled = enabled;
    }

    fn claw_pairing_state(&self) -> clarity_shell::PairingState {
        self.context.claw_pairing_state.clone()
    }

    fn start_openclaw_pairing(&mut self, index: usize) {
        App::start_openclaw_pairing(self, index);
    }

    fn cancel_openclaw_pairing(&mut self) {
        App::cancel_openclaw_pairing(self);
    }

    fn active_bot(&self) -> Option<clarity_shell::BotInfo> {
        self.context
            .ui_store
            .bot_instances
            .iter()
            .find(|b| b.id == self.context.ui_store.active_bot_id)
            .map(|b| clarity_shell::BotInfo {
                id: b.id.clone(),
                name: b.name.clone(),
                device_id: b.device_id.clone(),
                version: b.version.clone(),
                last_backup: b.last_backup.clone(),
                status: match b.status {
                    crate::stores::ui::BotStatus::Online => clarity_shell::BotStatus::Online,
                    crate::stores::ui::BotStatus::Offline => clarity_shell::BotStatus::Offline,
                    crate::stores::ui::BotStatus::Syncing => clarity_shell::BotStatus::Syncing,
                },
            })
    }

    fn spawn_provider_test(
        &self,
        provider_id: String,
        base_url: String,
        api_format: String,
        api_key: String,
        model: String,
    ) {
        let cfg = clarity_llm::runtime::RuntimeProviderConfig {
            provider_id: provider_id.clone(),
            base_url,
            api_format,
            api_key,
            model,
        };
        let tx = self.context.ui_tx.clone();
        let pid = provider_id;
        self.context.runtime.spawn(async move {
            let result = clarity_llm::runtime::test_connection(&cfg).await;
            let (success, error) = match result {
                Ok(()) => (true, None),
                Err(e) => (false, Some(e)),
            };
            let _ = tx.send(UiEvent::ProviderTestResult {
                provider_id: pid,
                success,
                error,
            });
        });
    }

    fn spawn_provider_refresh(
        &self,
        provider_id: String,
        base_url: String,
        api_format: String,
        api_key: String,
        model: String,
    ) {
        let cfg = clarity_llm::runtime::RuntimeProviderConfig {
            provider_id: provider_id.clone(),
            base_url,
            api_format,
            api_key,
            model,
        };
        let tx = self.context.ui_tx.clone();
        let pid = provider_id;
        self.context.runtime.spawn(async move {
            let models = clarity_llm::runtime::list_models(&cfg)
                .await
                .unwrap_or_default();
            let _ = tx.send(UiEvent::ProviderModelList {
                provider_id: pid,
                models,
            });
        });
    }
}

impl App {
    // ── Navigation API (typed Route + layer routers) ──

    /// Current main view route.
    pub(crate) fn current_main(&self) -> &clarity_core::ui::AppView {
        self.main_router
            .current()
            .unwrap_or(&clarity_core::ui::AppView::Chat)
    }

    /// Current modal route, if any.
    pub(crate) fn current_modal(&self) -> Option<&clarity_core::ui::ModalType> {
        self.modal_router.current()
    }

    /// Current right rail panel route, if any.
    pub(crate) fn current_right_rail(&self) -> Option<&clarity_core::ui::RightRailPanel> {
        self.right_rail_router.current()
    }

    /// True when the right rail is currently visible.
    pub(crate) fn is_right_rail_visible(&self) -> bool {
        self.current_right_rail()
            .map(|p| *p != clarity_core::ui::RightRailPanel::None)
            .unwrap_or(false)
    }

    /// Navigate to a typed route. Dispatches to the correct layer router.
    pub(crate) fn navigate(&mut self, route: clarity_core::ui::Route) {
        use clarity_core::ui::Route;
        match route {
            Route::Main(view) => self.main_router.navigate(view),
            Route::Modal(modal) => self.modal_router.navigate(modal),
            Route::RightRail(panel) => self.right_rail_router.navigate(panel),
        }
    }

    /// Global back navigation: close modal first, then right rail, then pop
    /// main view history. Returns the route that was popped, if any.
    pub(crate) fn go_back(&mut self) -> Option<clarity_core::ui::Route> {
        if let Some(modal) = self.modal_router.pop() {
            return Some(clarity_core::ui::Route::Modal(modal));
        }
        if let Some(panel) = self.right_rail_router.pop() {
            return Some(clarity_core::ui::Route::RightRail(panel));
        }
        self.main_router
            .go_back()
            .map(clarity_core::ui::Route::Main)
    }

    /// Open a modal dialog.
    pub(crate) fn open_modal(&mut self, modal: clarity_core::ui::ModalType) {
        self.modal_router.navigate(modal);
        self.view_state.focus = clarity_core::ui::FocusScope::Modal(modal);
    }

    /// Close the current modal and restore app-level focus.
    pub(crate) fn close_modal(&mut self) {
        self.modal_router.pop();
        self.view_state.focus = clarity_core::ui::FocusScope::App;
    }

    /// Collapse the right rail entirely.
    ///
    /// Also clears the dock so the post-render sync in `render_right_ide_panel`
    /// cannot resurrect the just-collapsed panel on the next animation frame.
    pub(crate) fn collapse_right_rail(&mut self) {
        self.right_rail_router.clear();
        self.context.ui_store.right_rail_dock = egui_dock::DockState::new(vec![]);
    }

    // ── Per-frame service methods (extracted from update() in Phase 2) ──

    /// Advance the frame counter and compute instantaneous FPS.
    fn tick_frame_counter(&mut self, ctx: &egui::Context) {
        let now = ctx.input(|i| i.time);
        self.context.ui_store.frame_count += 1;
        if now - self.context.ui_store.last_fps_time >= 1.0 {
            self.context.ui_store.fps = self.context.ui_store.frame_count as f64
                / (now - self.context.ui_store.last_fps_time);
            self.context.ui_store.frame_count = 0;
            self.context.ui_store.last_fps_time = now;
        }
    }

    /// Mirror agent runtime state into the UI status indicator.
    fn sync_agent_status(&mut self) {
        use clarity_core::agent::AgentState;
        self.chat_store_mut().agent_status = match self.context.state.agent.state() {
            AgentState::Unconfigured => AgentStatus::Unconfigured,
            AgentState::Idle => {
                if self.is_loading() {
                    AgentStatus::Busy
                } else {
                    AgentStatus::Online
                }
            }
            AgentState::Running { .. } => AgentStatus::Busy,
            AgentState::Stalled => AgentStatus::Offline,
        };
    }

    /// Sync the system-tray icon colour with the current runtime state.
    fn sync_tray_status(&mut self) {
        let agent_status = self.chat_store().agent_status;
        if let Some(ref mut tray) = self.context.tray_manager {
            let new_status = if !self.context.ui_store.pending_approvals.is_empty() {
                crate::services::tray::TrayIconStatus::Message
            } else {
                match agent_status {
                    AgentStatus::Online | AgentStatus::Unconfigured => {
                        crate::services::tray::TrayIconStatus::Idle
                    }
                    AgentStatus::Busy => crate::services::tray::TrayIconStatus::Active,
                    AgentStatus::Offline => crate::services::tray::TrayIconStatus::Error,
                }
            };
            if self.last_tray_status != Some(new_status) {
                tray.set_status(new_status);
                self.last_tray_status = Some(new_status);
            }
        }
    }

    /// Handle OS-level file drops into the window.
    fn handle_file_drops(&mut self, ctx: &egui::Context) {
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if dropped_files.is_empty() {
            return;
        }
        for file in dropped_files {
            if let Some(path) = file.path {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                self.chat_store_mut()
                    .attachments
                    .push(Attachment { path, name });
            }
        }
    }

    /// Periodic poll: tasks, parallel-batch status, Gateway health.
    fn poll_periodic_checks(&mut self) {
        let task_visible = self.current_right_rail()
            == Some(&clarity_core::ui::RightRailPanel::Task)
            && self.is_right_rail_visible();
        if task_visible && self.task_store().last_task_refresh.elapsed() > Duration::from_secs(3) {
            self.refresh_tasks();
        }
        if task_visible
            && !self.subagent_store().parallel_batches.is_empty()
            && self.subagent_store().last_parallel_poll.elapsed() > Duration::from_secs(2)
        {
            self.poll_parallel_batches();
        }
        if self.subagent_store().last_gateway_health_poll.elapsed() > Duration::from_secs(5) {
            self.subagent_store_mut().last_gateway_health_poll = Instant::now();
            self.poll_gateway_health();
        }
    }

    /// Apply responsive layout, theme, and approval modal state.
    fn apply_frame_state(&mut self, ctx: &egui::Context) {
        let _metrics = crate::layout::update_and_measure(self, ctx);
        ctx.style_mut_of(ctx.theme(), |style| {
            self.context.ui_store.theme.apply(style);
        });
        crate::design_system::install_theme(ctx, self.context.ui_store.theme.clone());
        // Mirror pending approvals into the modal state machine.
        if !self.context.ui_store.pending_approvals.is_empty()
            && !self.context.ui_store.kimi_conversation_style
        {
            if self.current_modal().is_none()
                || self.current_modal() == Some(&clarity_core::ui::ModalType::Approval)
            {
                self.open_modal(clarity_core::ui::ModalType::Approval);
            }
        } else if self.current_modal() == Some(&clarity_core::ui::ModalType::Approval) {
            self.close_modal();
        }

        // Detect main-stage route changes and start a slide transition.
        let current_main = *self.current_main();
        if current_main != self.prev_main_view {
            self.main_stage_transition = Some(crate::animation::MainStageTransition {
                from: self.prev_main_view,
                started: Instant::now(),
                duration: std::time::Duration::from_millis(250),
                direction: 1.0,
            });
            self.prev_main_view = current_main;
            ctx.request_repaint();
        }
    }

    /// Render a custom titlebar with window drag and control buttons.
    ///
    /// LAYOUT (two independent sub-layouts at the same vertical origin):
    ///   ┌─ left_to_right ──────────────────────────┐  ┌─ right_to_left ─┐
    ///   │ [☰] Clarity  [drag region ─── elastic]  │  │ [─] [□] [✕]    │
    ///   └──────────────────────────────────────────┘  └─────────────────┘
    ///
    /// ARCHITECTURE NOTE:
    ///   The drag region uses `allocate_exact_size` ONLY inside a horizontal
    ///   sub-layout, so `avail` is REMAINING WIDTH — not the full panel height.
    ///   This avoids the layout feedback loop where the drag region consumed
    ///   the entire panel, forcing content below and causing panel growth
    ///   every frame.
    ///
    ///   Button sub-layout (right_to_left) is rendered second, so its buttons
    ///   have higher z-order than the drag region — clicks on buttons are
    ///   NOT swallowed by the drag.
    /// Render a panel with panic isolation (error boundary).
    /// Mimics React ErrorBoundary: a child panel panic does not crash the entire app.
    /// Render a full-screen scrim that blocks background interaction when a
    /// modal is open. Delegates to the protocol-compliant helper in
    /// `clarity_ui::widgets::modal`.
    fn render_modal_scrim(&self, ctx: &egui::Context) -> egui::Response {
        clarity_ui::widgets::modal::modal_scrim(ctx)
    }

    fn render_safe<F>(&mut self, ui: &mut egui::Ui, name: &str, mut render: F)
    where
        F: FnMut(&mut Self, &mut egui::Ui),
    {
        if let Err(e) = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| render(self, ui)))
        {
            let payload = if let Some(s) = e.downcast_ref::<&str>() {
                s.to_string()
            } else if let Some(s) = e.downcast_ref::<String>() {
                s.clone()
            } else {
                "unknown panic".to_string()
            };
            let msg = format!("PANIC in panel '{}': {}", name, payload);
            tracing::error!("{}", msg);
            self.context
                .push_toast(format!("UI error in {} panel", name), ToastLevel::Error);
        }
    }

    /// Find a Claw session by its session_key.
    fn claw_session_id_by_key(&self, session_key: &str) -> Option<String> {
        self.context
            .session_store
            .sessions
            .iter()
            .find(|s| {
                matches!(
                    &s.context,
                    crate::ui::types::SessionContext::Claw {
                        session_key: key,
                        ..
                    } if key == session_key
                )
            })
            .map(|s| s.id.clone())
    }

    /// Find all Claw sessions belonging to a role.
    fn claw_session_ids_by_role(&self, role: &str) -> Vec<String> {
        self.context
            .session_store
            .sessions
            .iter()
            .filter(|s| {
                matches!(
                    &s.context,
                    crate::ui::types::SessionContext::Claw {
                        role: r,
                        ..
                    } if r == role
                )
            })
            .map(|s| s.id.clone())
            .collect()
    }

    /// Subscribe to and fetch history for the given Claw session_key over an
    /// already-connected Gateway WebSocket.
    fn subscribe_claw_session(&self, session_key: &str) {
        let Some(ref ws) = self.context.claw_ws else {
            return;
        };
        // OpenClaw/KimiClaw Gateways do not support the Gateway-native
        // subscribe/history WebSocket commands.
        let is_openclaw =
            self.active_claw_protocol() == Some(crate::claw::ClawProtocol::OpenClawJsonRpc);
        if is_openclaw {
            return;
        }
        ws.subscribe_session(session_key);
        ws.subscribe_messages(session_key);
        ws.get_history(session_key);
    }

    /// Returns true when an agent turn is actively loading/generating.
    pub(crate) fn is_loading(&self) -> bool {
        matches!(self.view_state.turn, clarity_core::ui::TurnState::Loading)
    }

    /// Render the left navigation tree.
    ///
    /// S6 Phase D: the left chrome is now a single fixed-width tree. The old
    /// 36px icon rail and the conditional expanded panel have been replaced by
    /// `panels::navigation_tree`.
    ///
    /// During collapse animation, the nav tree renders at the animated width
    /// so content doesn't skip while the layout adjusts.
    fn render_left_rail(&mut self, ui: &mut egui::Ui) {
        let effective_w = self.effective_left_rail_width(ui.ctx());
        // Only render when there is visible width (either expanded or animating).
        if effective_w > 0.0 {
            crate::panels::navigation_tree::render_left_navigation_tree(self, ui, effective_w);
        }
    }

    /// Return the effective left rail width, animated during expand/collapse.
    fn effective_left_rail_width(&self, ctx: &egui::Context) -> f32 {
        let theme = &self.context.ui_store.theme;
        let collapsed = theme.size_sidebar_collapsed;
        let expanded = theme.size_sidebar;
        let factor = theme.animate_bool_normal(
            ctx,
            egui::Id::new("left_rail_width"),
            self.view_state.left_rail_expanded,
        );
        collapsed + (expanded - collapsed) * factor
    }

    /// Return the effective right rail width, animated during open/close.
    fn effective_right_rail_width(&self, ctx: &egui::Context) -> f32 {
        let theme = &self.context.ui_store.theme;
        let is_visible = self.is_right_rail_visible();
        let factor = theme.animate_bool_normal(ctx, egui::Id::new("right_rail_width"), is_visible);
        let user_w = self
            .context
            .ui_store
            .right_rail_width
            .unwrap_or(theme.size_panel_right)
            .clamp(180.0, 400.0);
        user_w * factor
    }

    /// Render the unified outer border around the chat stage + right rail + input bar.
    ///
    /// The individual panels already fill themselves with `theme.bg`; this helper
    /// only draws the outer rounded stroke so there is no mismatch/black gap
    /// between a painter fill and the panel fills. The border is inset a few
    /// pixels from the window edges and from the left sidebar for a cleaner,
    /// Kimi-style floating-stage look.
    fn render_main_stage_border(&self, ctx: &egui::Context) {
        let theme = self.context.ui_store.theme.clone();
        let left_w = self.effective_left_rail_width(ctx);
        let right_w = self.effective_right_rail_width(ctx);

        let screen = ctx.input(|i| i.viewport_rect());
        let titlebar_h = theme.size_titlebar;
        let inset = theme.space_4;

        // Layer 1 — base: fills the entire area below the titlebar all the way
        // to the window edges, including the left-rail region. This makes
        // titlebar and left rail share one continuous background.
        let base_rect = egui::Rect::from_min_max(
            egui::pos2(0.0, titlebar_h),
            egui::pos2(screen.max.x, screen.max.y),
        );
        if base_rect.width() <= 0.0 || base_rect.height() <= 0.0 {
            return;
        }

        // Layer 2 — inner surface: inset from the base, with rounded corners and border.
        let surface_rect = egui::Rect::from_min_max(
            egui::pos2(left_w + inset, titlebar_h + inset),
            egui::pos2(screen.max.x - inset, screen.max.y - inset),
        );

        let bg_painter = ctx.layer_painter(egui::LayerId::background());
        let radius = theme.radius_lg as u8;
        let corner_radius = egui::CornerRadius::same(radius);

        bg_painter.rect_filled(base_rect, egui::CornerRadius::ZERO, theme.bg);

        // Layer 1.5 — soft shadow behind the floating main-stage surface.
        // Painted before the surface so it sits just below it. The shadow is
        // intentionally subtle to avoid the flat "egui panel" look without
        // requiring expensive per-frame blur.
        let shadow_offset = theme.space_8;
        let shadow_rect = surface_rect
            .expand(theme.space_4)
            .translate(egui::vec2(0.0, shadow_offset));
        let shadow_color = theme.shadow_panel.color.linear_multiply(0.4);
        bg_painter.rect_filled(shadow_rect, corner_radius, shadow_color);

        bg_painter.rect_filled(surface_rect, corner_radius, theme.bg);

        // Right-rail surface: paint the rail with the same rounded corners as
        // the main stage so its right edge seamlessly meets the outer border.
        // The divider line is drawn by the panel after its contents so the
        // native resize hover/drag line can be hidden.
        if right_w > 0.0 {
            let rail_w = right_w.min(surface_rect.width());
            let rail_rect = egui::Rect::from_min_max(
                egui::pos2(surface_rect.max.x - rail_w, surface_rect.min.y),
                surface_rect.max,
            );
            bg_painter.rect_filled(rail_rect, corner_radius, theme.bg);
        }

        // Paint the border stroke on the tooltip layer so it sits above the
        // right-rail cover that hides the native resize hover/drag line.
        let border_painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Tooltip,
            egui::Id::new("main_stage_border"),
        ));
        border_painter.rect_stroke(
            surface_rect,
            corner_radius,
            egui::Stroke::new(1.0_f32, theme.border),
            egui::StrokeKind::Inside,
        );
    }

    /// Render the IDE-style right rail panel.
    ///
    /// S6 Phase D: the right rail is now a single functional panel selected by
    /// the Bot bar. The old stacked-card content lives in `panels::right_rail`
    /// and will be migrated into the new IDE panels over the next iterations.
    fn render_right_rail(&mut self, ui: &mut egui::Ui) {
        // Allow rendering during close animation: the rail is still
        // visible (decreasing width) while the animation runs. Only
        // skip when fully collapsed.
        let is_visible = self.is_right_rail_visible();
        let factor = self.context.ui_store.theme.animate_bool_normal(
            ui.ctx(),
            egui::Id::new("right_rail_width"),
            is_visible,
        );
        if !is_visible && factor <= 0.0 {
            return;
        }
        crate::panels::right_ide_panel::render_right_ide_panel(self, ui);
    }

    /// Handle system tray events: show/hide window and menu actions.
    fn handle_tray_events(&mut self, ctx: &egui::Context) {
        let Some(tray) = self.context.tray_manager.as_ref() else {
            return;
        };

        // Tray icon clicks (double-click → show)
        for event in tray.poll_tray_events() {
            use tray_icon::TrayIconEvent;
            match event {
                TrayIconEvent::DoubleClick { .. } | TrayIconEvent::Click { .. } => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                }
                _ => {}
            }
        }

        // Context menu actions
        for action in tray.poll_menu_events() {
            use crate::services::tray::TrayAction;
            match action {
                TrayAction::Show => {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
                }
                TrayAction::CopySessionLink => {
                    if let Some(session) = self.context.session_store.active_session() {
                        let link = format!("clarity://session/{}", session.id);
                        ctx.copy_text(link);
                        self.context
                            .push_toast("Session link copied".to_string(), ToastLevel::Info);
                    }
                }
                TrayAction::Pause => {
                    self.stop();
                    self.context
                        .push_toast("Agent paused".to_string(), ToastLevel::Info);
                }
                TrayAction::Settings => {
                    self.navigate(clarity_core::ui::AppView::Settings.into());
                }
                TrayAction::Quit => {
                    self.tray_quit_requested = true;
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    fn handle_window_resize(&mut self, ctx: &egui::Context) {
        let screen_rect = ctx.input(|i| i.viewport_rect());
        let edge = self.context.ui_store.theme.window_edge_zone;

        // Skip resize when maximized; it may not work properly and conflicts with restore logic.
        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
        if is_maximized {
            return;
        }

        if let Some(pos) = ctx.input(|i| i.pointer.hover_pos()) {
            // Do not trigger edge resize inside the titlebar area; it conflicts with drag-to-move.
            if pos.y < screen_rect.min.y + self.context.ui_store.theme.size_titlebar + edge {
                return;
            }

            let on_left = pos.x < screen_rect.min.x + edge;
            let on_right = pos.x > screen_rect.max.x - edge;
            let on_top = pos.y < screen_rect.min.y + edge;
            let on_bottom = pos.y > screen_rect.max.y - edge;

            let (direction, cursor) = if on_top && on_left {
                (
                    Some(egui::ResizeDirection::NorthWest),
                    egui::CursorIcon::ResizeNorthWest,
                )
            } else if on_top && on_right {
                (
                    Some(egui::ResizeDirection::NorthEast),
                    egui::CursorIcon::ResizeNorthEast,
                )
            } else if on_bottom && on_left {
                (
                    Some(egui::ResizeDirection::SouthWest),
                    egui::CursorIcon::ResizeSouthWest,
                )
            } else if on_bottom && on_right {
                (
                    Some(egui::ResizeDirection::SouthEast),
                    egui::CursorIcon::ResizeSouthEast,
                )
            } else if on_left {
                (
                    Some(egui::ResizeDirection::West),
                    egui::CursorIcon::ResizeHorizontal,
                )
            } else if on_right {
                (
                    Some(egui::ResizeDirection::East),
                    egui::CursorIcon::ResizeHorizontal,
                )
            } else if on_top {
                (
                    Some(egui::ResizeDirection::North),
                    egui::CursorIcon::ResizeVertical,
                )
            } else if on_bottom {
                (
                    Some(egui::ResizeDirection::South),
                    egui::CursorIcon::ResizeVertical,
                )
            } else {
                (None, egui::CursorIcon::Default)
            };

            if let Some(dir) = direction {
                ctx.set_cursor_icon(cursor);
                if ctx.input(|i| i.pointer.button_pressed(egui::PointerButton::Primary)) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::BeginResize(dir));
                }
            }
        }
    }

    /// Unified command dispatcher (P0.5.C.1).
    ///
    /// Both the keyboard shortcut layer (`ShortcutAction::command_id()`) and
    /// the `CommandPalette` route through this method. Adding a new command
    /// means: (1) add a string constant in `clarity_core::ui::ids`,
    /// (2) match it here, (3) optionally bind a shortcut in `shortcuts/mod.rs`,
    /// (4) optionally surface in `built_in::all()`.
    ///
    /// Returns `true` when the command id is recognised.
    fn dispatch_command(&mut self, cmd_id: &str) -> bool {
        use clarity_core::ui::ids;
        match cmd_id {
            ids::CLOSE_MODAL => {
                // Global back: close modal first, then right rail, then pop main history.
                let _ = self.go_back();
                true
            }
            ids::NEW_SESSION => {
                if !self.is_loading() {
                    self.new_session();
                }
                true
            }
            ids::STOP_GENERATION => {
                self.stop();
                true
            }
            ids::SEND_MESSAGE => {
                if !self.chat_store().input.trim().is_empty() && !self.is_loading() {
                    self.chat_store_mut().stick_to_bottom = true;
                    self.send();
                }
                true
            }
            ids::TOGGLE_SKILL_PANEL => {
                if self.current_modal() == Some(&clarity_core::ui::ModalType::Skill) {
                    self.close_modal();
                } else {
                    self.open_modal(clarity_core::ui::ModalType::Skill);
                }
                true
            }
            ids::TOGGLE_TEAM_PANEL => {
                self.toggle_right_rail_tab(
                    clarity_core::ui::RightRailPanel::Team,
                    clarity_core::ui::RightRailContext::Session,
                );
                true
            }
            ids::FOCUS_INPUT => {
                self.context.ui_store.focus_target = Some(FocusTarget::ChatInput);
                true
            }
            ids::TOGGLE_COMMAND_PALETTE => {
                self.command_palette.open = true;
                self.command_palette.query.clear();
                self.command_palette.selected = 0;
                true
            }
            ids::TOGGLE_DASHBOARD => {
                let target = if self.current_main() == &clarity_core::ui::AppView::Dashboard {
                    clarity_core::ui::AppView::Chat
                } else {
                    clarity_core::ui::AppView::Dashboard
                };
                self.navigate(target.into());
                true
            }
            ids::TOGGLE_LAYOUT_DEBUG => {
                self.view_state.toggle_debug_layout_overlay();
                self.persist_layout_settings();
                true
            }
            ids::TOGGLE_SIDEBAR => {
                self.view_state.left_rail_expanded = !self.view_state.left_rail_expanded;
                true
            }
            ids::OPEN_SETTINGS => {
                self.navigate(clarity_core::ui::AppView::Settings.into());
                true
            }
            ids::NAVIGATE_DOWN => {
                self.navigate_line(1);
                true
            }
            ids::NAVIGATE_UP => {
                self.navigate_line(-1);
                true
            }
            ids::NAVIGATE_TOP => {
                self.context.ui_store.line_cursor_selected = Some(0);
                true
            }
            ids::NAVIGATE_BOTTOM => {
                let total = self.context.ui_store.line_cursor_total_lines;
                if total > 0 {
                    self.context.ui_store.line_cursor_selected = Some(total.saturating_sub(1));
                }
                true
            }
            ids::COPY_LINE => {
                // Actual copy is handled in App::update() where egui::Context is available.
                true
            }
            ids::INCREASE_FONT_SCALE => {
                self.increase_font_scale();
                true
            }
            ids::DECREASE_FONT_SCALE => {
                self.decrease_font_scale();
                true
            }
            ids::TOGGLE_CONSOLE => {
                self.toggle_right_rail_tab(
                    clarity_core::ui::RightRailPanel::Console,
                    clarity_core::ui::RightRailContext::Session,
                );
                true
            }
            ids::TOGGLE_FILES => {
                self.toggle_right_rail_tab(
                    clarity_core::ui::RightRailPanel::Files,
                    clarity_core::ui::RightRailContext::Session,
                );
                true
            }
            ids::SHOW_SHORTCUTS => {
                self.shortcuts_help_open = true;
                true
            }
            ids::TOGGLE_SHARE => {
                self.toggle_right_rail_tab(
                    clarity_core::ui::RightRailPanel::Share,
                    clarity_core::ui::RightRailContext::Session,
                );
                true
            }
            ids::SCROLL_TO_BOTTOM => {
                self.chat_store_mut().stick_to_bottom = true;
                true
            }
            ids::NAVIGATE_MESSAGE_UP => {
                self.navigate_message_selection(-1);
                true
            }
            ids::NAVIGATE_MESSAGE_DOWN => {
                self.navigate_message_selection(1);
                true
            }
            ids::COPY_SELECTED_MESSAGE => {
                // Actual copy is handled in App::update() where egui::Context is available.
                true
            }
            ids::EDIT_SELECTED_MESSAGE => {
                self.edit_selected_message();
                true
            }
            ids::REGENERATE_SELECTED_MESSAGE => {
                self.regenerate_selected_message();
                true
            }
            ids::CLEAR_MESSAGE_SELECTION => {
                self.context.ui_store.selected_message_idx = None;
                true
            }
            other => {
                tracing::warn!("dispatch_command: unknown command id '{}'", other);
                false
            }
        }
    }

    /// S7 Phase 2D: navigate line cursor by `delta` lines (-1 = up, +1 = down).
    fn navigate_line(&mut self, delta: isize) {
        let total = self.context.ui_store.line_cursor_total_lines;
        if total == 0 {
            return;
        }
        let current = self.context.ui_store.line_cursor_selected.unwrap_or(0);
        let new_idx = if delta > 0 {
            (current + delta as usize).min(total.saturating_sub(1))
        } else {
            current.saturating_sub((-delta) as usize)
        };
        self.context.ui_store.line_cursor_selected = Some(new_idx);
    }

    /// S7 Phase 2D: return the text of the currently selected line (if any).
    fn selected_line_text(&self) -> Option<String> {
        let global_idx = self.context.ui_store.line_cursor_selected?;
        let active_id = self.context.session_store.active_session_id.clone();
        let session = self
            .context
            .session_store
            .sessions
            .iter()
            .find(|s| s.id == active_id)?;
        let mut acc = 0;
        for msg in &session.messages {
            let msg_lines = msg.lines.len();
            if global_idx >= acc && global_idx < acc + msg_lines {
                let local_idx = global_idx - acc;
                return msg.lines.get(local_idx).map(render_line_text);
            }
            acc += msg_lines;
        }
        None
    }

    /// Search the active session's messages for `find_query` and populate
    /// `find_matches` with the indices of matching messages.
    pub(crate) fn update_find_matches(&mut self) {
        // Skip recomputation when the query hasn't changed — avoids an O(n)
        // scan over all messages every frame while the find bar is open.
        if self.chat_store().find_query == self.chat_store().find_last_query {
            return;
        }
        let query = self.chat_store().find_query.clone();
        let query_lower = query.to_lowercase();
        let matches: Vec<usize> = if query.is_empty() {
            Vec::new()
        } else {
            self.context
                .session_store
                .active_session()
                .map(|session| {
                    session
                        .messages
                        .iter()
                        .enumerate()
                        .filter(|(_, msg)| msg.content.to_lowercase().contains(&query_lower))
                        .map(|(i, _)| i)
                        .collect()
                })
                .unwrap_or_default()
        };
        let chat_store = self.chat_store_mut();
        chat_store.find_last_query = query;
        chat_store.find_matches = matches;
        let match_count = chat_store.find_matches.len();
        if chat_store.find_current >= match_count {
            chat_store.find_current = match_count.saturating_sub(1);
        }
    }

    /// Render the find-in-session bar above the chat message list.
    fn render_find_bar(&mut self, ui: &mut egui::Ui) {
        let theme = self.context.ui_store.theme.clone();
        let total = self.chat_store().find_matches.len();
        let current = if total > 0 {
            self.chat_store().find_current + 1
        } else {
            0
        };

        crate::design_system::surface_panel(ui, |ui| {
            ui.horizontal(|ui| {
                // Search input.
                let text_edit = ui.add(
                    TextInput::singleline(&mut self.chat_store_mut().find_query)
                        .transparent()
                        .hint_text("Find in session…")
                        .width(ui.available_width() - 120.0),
                );
                if text_edit.changed() {
                    self.chat_store_mut().find_current = 0;
                }
                if text_edit.lost_focus()
                    && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    && total > 0
                {
                    let next = (self.chat_store().find_current + 1) % total;
                    self.chat_store_mut().find_current = next;
                }

                // Match counter: "2 of 5"
                let count_text = if self.chat_store().find_query.is_empty() {
                    String::new()
                } else {
                    format!("{} of {}", current, total)
                };
                if !count_text.is_empty() {
                    crate::design_system::text(
                        ui,
                        count_text,
                        crate::design_system::TextStyle::Small,
                    );
                }

                // Prev / Next buttons.
                if ui
                    .add_enabled(total > 0, Button::new("▲").ghost().small())
                    .clicked()
                {
                    let current = self.chat_store().find_current;
                    self.chat_store_mut().find_current = if current > 0 {
                        current - 1
                    } else {
                        total.saturating_sub(1)
                    };
                }
                if ui
                    .add_enabled(total > 0, Button::new("▼").ghost().small())
                    .clicked()
                {
                    let current = self.chat_store().find_current;
                    self.chat_store_mut().find_current = (current + 1) % total.max(1);
                }

                // Close.
                if crate::widgets::icon_button_toolbar(
                    ui,
                    crate::theme::ICON_X,
                    theme.text_sm,
                    &theme,
                )
                .on_hover_text("Close find")
                .clicked()
                {
                    self.chat_store_mut().find_open = false;
                }
            });
        });
    }

    fn render_mcp_panel(&mut self, ctx: &egui::Context) {
        panels::mcp::render_mcp_panel(self, ctx);
    }

    fn render_task_create_modal(&mut self, ctx: &egui::Context) {
        panels::task_create::render_task_create_modal(self, ctx);
    }

    fn render_task_view_modal(&mut self, ctx: &egui::Context) {
        panels::task_view::render_task_view_modal(self, ctx);
    }

    fn render_subagent_view_modal(&mut self, ctx: &egui::Context) {
        panels::subagent_view::render_subagent_view_modal(self, ctx);
    }

    fn render_team_create_modal(&mut self, ctx: &egui::Context) {
        panels::team_create::render_team_create_modal(self, ctx);
    }

    fn render_cron_create_modal(&mut self, ctx: &egui::Context) {
        panels::cron_create::render_cron_create_modal(self, ctx);
    }

    fn render_skill_panel(&mut self, ctx: &egui::Context) {
        panels::skill::render_skill_panel(self, ctx);
    }

    fn render_toasts(&mut self, ctx: &egui::Context) {
        panels::toast::render_toasts(self, ctx);
    }

    fn render_approval_modal(&mut self, ctx: &egui::Context) {
        panels::approval::render_approval_modal(self, ctx);
    }

    fn render_snapshot_modal(&mut self, ctx: &egui::Context) {
        panels::snapshot::render_snapshot_modal(self, ctx);
    }

    /// Start an in-app OpenClaw pairing flow for the connection at `conn_idx`.
    pub(crate) fn start_openclaw_pairing(&mut self, conn_idx: usize) {
        let Some(conn) = self
            .settings_store()
            .settings_edit
            .openclaw_connections
            .get(conn_idx)
            .cloned()
        else {
            self.context.claw_pairing_state =
                PairingState::Error("Connection not found".to_string());
            return;
        };

        if conn.token.as_deref().unwrap_or("").is_empty() {
            self.context.claw_pairing_state =
                PairingState::Error("Gateway token is required to request pairing".to_string());
            return;
        }

        let identity = self
            .context
            .claw_device_identity
            .clone()
            .or_else(|| clarity_claw::DeviceIdentity::load_or_generate().ok());
        let Some(identity) = identity else {
            self.context.claw_pairing_state =
                PairingState::Error("Failed to load or generate device identity".to_string());
            return;
        };
        self.context.claw_device_identity = Some(identity.clone());

        let ws_url = crate::claw::to_ws_url(&conn.gateway_url);

        let token = crate::settings::GuiSettings::resolve_api_key(&conn.token).unwrap_or_default();
        self.context.claw_pairing_state = PairingState::Requesting;
        let client = clarity_claw::ClawClient::connect(&ws_url, &token);
        let scopes = vec![
            "operator.admin".into(),
            "operator.read".into(),
            "operator.write".into(),
            "operator.approvals".into(),
            "operator.pairing".into(),
            "operator.talk.secrets".into(),
        ];
        client.request_pairing(
            &identity.device_id(),
            &identity.public_key(),
            "openclaw-control-ui",
            "webchat",
            "windows",
            "operator",
            &scopes,
        );

        self.context.claw_pairing_client = Some(client);
        self.context.claw_pairing_state = PairingState::Waiting {
            gateway_url: ws_url,
            since: std::time::Instant::now(),
        };
        self.context.push_toast(
            "Pairing request sent. Approve it in the Gateway UI.".to_string(),
            ToastLevel::Info,
        );
    }

    /// Cancel an in-progress pairing flow.
    pub(crate) fn cancel_openclaw_pairing(&mut self) {
        self.context.claw_pairing_client = None;
        self.context.claw_pairing_state = PairingState::Idle;
    }

    /// Finish a successful pairing: save the device token to the matching
    /// settings connection and optionally persist a global paired token.
    fn finish_openclaw_pairing(&mut self, device_id: &str, token: &str, scopes: &[String]) {
        let gateway_url = match &self.context.claw_pairing_state {
            PairingState::Waiting { gateway_url, .. } => gateway_url.clone(),
            _ => String::new(),
        };

        let mut saved = false;
        for conn in &mut self.settings_store_mut().settings_edit.openclaw_connections {
            let conn_ws = if conn.gateway_url.starts_with("ws://")
                || conn.gateway_url.starts_with("wss://")
            {
                conn.gateway_url.clone()
            } else {
                conn.gateway_url
                    .replace("http://", "ws://")
                    .replace("https://", "wss://")
            };
            if normalize_gateway_url(&conn_ws) == normalize_gateway_url(&gateway_url) {
                conn.auth_mode = crate::settings::OpenClawAuthMode::DevicePaired;
                conn.device_token = Some(token.to_string());
                saved = true;
                break;
            }
        }

        // Also persist a global paired-token file as a fallback for discovery.
        let paired = clarity_claw::PairedToken {
            gateway_url: gateway_url.clone(),
            token: token.into(),
            device_token: Some(token.into()),
            role: "operator".into(),
            scopes: scopes.to_vec(),
            paired_at_ms: Utc::now().timestamp_millis(),
        };
        if let Err(e) = clarity_claw::save_paired_token(&paired) {
            tracing::warn!("Failed to save paired token: {}", e);
        } else {
            self.context.claw_device_token = Some(paired);
        }

        if saved {
            self.auto_save_settings();
            self.context.push_toast(
                format!(
                    "Device {} paired successfully (scopes: {})",
                    &device_id[..device_id.len().min(8)],
                    scopes.join(",")
                ),
                crate::ui::types::ToastLevel::Info,
            );
        } else {
            self.context.push_toast(
                "Pairing approved, but no matching settings connection was found.".to_string(),
                crate::ui::types::ToastLevel::Warn,
            );
        }

        self.context.claw_pairing_state = PairingState::Approved {
            gateway_url,
            token: token.into(),
        };
        self.context.claw_pairing_client = None;
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // ── Intercept system close (Alt+F4 / taskbar close) → hide to tray ──
        if ctx.input(|i| i.viewport().close_requested()) {
            if self.tray_quit_requested {
                // Allow the close to proceed (Quit from tray menu).
            } else {
                ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
                ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
            }
        }

        // ── Poll system tray events ──
        self.handle_tray_events(&ctx);

        self.tick_frame_counter(&ctx);

        // Sync live Claw device list and manage Claw WebSocket lifecycle
        // (~2 Hz — device snapshot, reconnect, response draining, pairing).
        if self.context.ui_store.frame_count % 30 == 0 {
            self.manage_claw_connection();
            self.drain_claw_ws_responses();
            self.drain_pairing_responses();
            self.timeout_claw_pairing();
        }

        // Persist window position every ~5 s so it survives crashes.
        if self.context.ui_store.frame_count % 300 == 0 {
            if let Some(rect) = ctx.input(|i| i.viewport().outer_rect) {
                let pos = rect.min;
                let settings_store = self.settings_store();
                let dirty = settings_store.settings_edit.window_x != Some(pos.x)
                    || settings_store.settings_edit.window_y != Some(pos.y);
                if dirty {
                    let settings_store = self.settings_store_mut();
                    settings_store.settings_edit.window_x = Some(pos.x);
                    settings_store.settings_edit.window_y = Some(pos.y);
                    let _ = self.commit_settings();
                }
            }
        }

        // Refresh shell prompt (~1 Hz) to track cwd / git branch changes.
        if self.context.ui_store.frame_count % 60 == 0 {
            self.context.refresh_shell_prompt();
        }

        self.process_events();

        // Detect session switches and reset chat-local transient state so the
        // new session's message list / input / scroll are rendered fresh.
        let active_id = self.context.session_store.active_session_id.clone();
        if self.context.ui_store.last_active_session_id != active_id {
            self.context.ui_store.last_active_session_id = active_id;
            self.context.ui_store.last_scroll_offset = 0.0;
            self.context.ui_store.selected_message_idx = None;
            let chat_store = self.chat_store_mut();
            // Start new sessions at the top of the message list so the wheel is
            // not locked to the bottom on launch; stick-to-bottom is enabled
            // only when the user sends a message or streaming begins.
            chat_store.stick_to_bottom = false;
            chat_store.editing_message_idx = None;
            chat_store.edit_buffer.clear();
            // Close find bar and clear query so stale matches from the
            // previous session don't persist into the new one.
            chat_store.find_open = false;
            chat_store.find_query.clear();
            chat_store.find_matches.clear();
            chat_store.find_current = 0;
            self.context.ui_store.focus_target = Some(FocusTarget::ChatInput);
            // Stateful providers (e.g. deepseek-device) must not carry
            // conversation context across clarity sessions.
            if let Some(ref llm) = self.context.state.agent.llm() {
                llm.reset_conversation_context();
            }
            ctx.request_repaint();
        }
        if self.context.ui_store.request_repaint {
            self.context.ui_store.request_repaint = false;
            ctx.request_repaint();
        }

        // Sync the layout debug overlay toggle from ViewState to egui memory.
        crate::ui::debug_overlay::sync_enabled(&ctx, self.view_state.debug_layout_overlay);

        // Poll MCP config for external changes (hot-reload).
        self.context.check_mcp_config_reload();

        // Drain batch-grant auto-approval notifications and show toasts.
        for msg in self
            .context
            .state
            .mode_aware_approval_runtime
            .drain_auto_approval_notifications()
        {
            self.context.push_toast(msg, ToastLevel::Info);
        }

        if self.is_loading() {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        self.handle_file_drops(&ctx);

        // ── Find-in-session (Ctrl+F) — lightweight toggle, not routed through
        //    the ShortcutAction system to keep the dispatch path simple. ──
        if ctx.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.ctrl)
            && !shortcuts::is_modal_open(self)
        {
            let find_open = !self.chat_store().find_open;
            let chat_store = self.chat_store_mut();
            chat_store.find_open = find_open;
            if find_open {
                chat_store.find_query.clear();
                chat_store.find_matches.clear();
                chat_store.find_current = 0;
                self.context.ui_store.focus_target = Some(FocusTarget::ChatInput);
            }
        }
        // Close find bar on Escape (before CloseModal for layering reasons).
        if self.chat_store().find_open && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.chat_store_mut().find_open = false;
        }

        // ── Global keyboard shortcuts (P0.5.C.1: unified dispatch) ──
        // All shortcut actions and CommandPalette entries route through
        // App::dispatch_command(&str) using ids from clarity_core::ui::ids.
        for action in shortcuts::collect_actions(&ctx, self) {
            if action == shortcuts::ShortcutAction::CopyLine {
                if let Some(text) = self.selected_line_text() {
                    ctx.copy_text(text);
                    self.context
                        .push_toast("Copied to clipboard", ToastLevel::Info);
                }
            } else if action == shortcuts::ShortcutAction::CopySelectedMessage {
                if let Some(text) = self.selected_message_text() {
                    ctx.copy_text(text);
                    self.context
                        .push_toast("Copied to clipboard", ToastLevel::Info);
                }
            } else {
                self.dispatch_command(action.command_id());
            }
        }

        self.poll_periodic_checks();

        // ── Auto-save: persist active session every 30 frames (~0.5 s) when
        //    modified. Guards against data loss on crash between explicit saves.
        if self.context.ui_store.frame_count % 30 == 0 {
            let needs_save = self
                .context
                .session_store
                .active_session()
                .map(|s| s.updated_at > s.last_saved_at)
                .unwrap_or(false);
            if needs_save {
                self.context.save_current_session();
            }
        }

        // ── Stuck-turn guard: if a turn has been in_flight for > 5 min
        //    without a Done or Error event, force-reset so the user isn't
        //    permanently blocked. ──
        if let Some(since) = self.chat_store().in_flight_since {
            if since.elapsed() > std::time::Duration::from_secs(300) {
                tracing::warn!("Turn stuck in_flight for > 5 min — force-resetting");
                if let Some(session) = self.context.session_store.active_session_mut() {
                    session.in_flight = false;
                }
                {
                    let chat_store = self.chat_store_mut();
                    chat_store.in_flight_since = None;
                    chat_store.agent_status = AgentStatus::Online;
                }
                self.view_state.turn = clarity_core::ui::TurnState::Idle;
                self.context.push_toast(
                    "Agent turn timed out — you can retry your last message.",
                    ToastLevel::Warn,
                );
            }
        }

        self.sync_agent_status();
        self.sync_tray_status();
        self.apply_frame_state(&ctx);

        // ── Layout shell: chrome + main view + overlays + modals ──
        if let Some(mut chrome) = self.chrome.take() {
            chrome.render(self, ui, &ctx);
            self.chrome = Some(chrome);
        }

        // Pretext PoC: measurement probe window
        if self.context.ui_store.pretext_probe_open {
            crate::widgets::pretext_probe::render_pretext_probe(self, &ctx);
        }

        // Keyboard shortcuts reference (Ctrl+/)
        self.render_shortcuts_help(&ctx);

        // Command Palette (top-most layer)
        if self.command_palette.open {
            let commands = clarity_core::ui::commands::built_in::all();
            let theme = self.context.ui_store.theme.clone();
            // P0.5.C.2: palette returns the activated command id (if any),
            // which we forward to the unified dispatcher.
            if let Some(cmd_id) = self.command_palette.show(&ctx, &theme, &commands) {
                self.dispatch_command(&cmd_id);
            }
        }

        // ── Theme-switch fade transition (250 ms) ──
        if let Some(start) = self.context.ui_store.theme_transition_start {
            let elapsed = start.elapsed().as_secs_f32();
            let duration = 0.25;
            if elapsed >= duration {
                self.context.ui_store.theme_transition_start = None;
            } else {
                // Ease-out: alpha goes from opaque to transparent.
                let t = elapsed / duration;
                let alpha = 1.0 - crate::animation::ease_out_cubic(t);
                let screen = ctx.input(|i| i.viewport_rect());
                let fg = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("theme_transition_overlay"),
                ));
                fg.rect_filled(
                    screen,
                    egui::CornerRadius::ZERO,
                    egui::Color32::from_black_alpha((alpha * 255.0) as u8),
                );
            }
            ctx.request_repaint();
        }
    }

    fn on_exit(&mut self) {
        self.context.save_current_session();
    }
}

fn main() -> eframe::Result {
    let startup_t0 = Instant::now();
    clarity_core::logging::init();
    std::panic::set_hook(Box::new(|info| {
        let msg = if let Some(s) = info.payload().downcast_ref::<&str>() {
            s.to_string()
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            s.clone()
        } else {
            "Unknown panic payload".to_string()
        };
        let location = info
            .location()
            .map(|l| format!("{}:{}", l.file(), l.line()))
            .unwrap_or_else(|| "unknown location".to_string());
        let report = format!("[{}] PANIC: {}\n", location, msg);
        eprintln!("{}", report);
        if let Some(data_dir) = dirs::data_dir() {
            let log_path = data_dir.join("clarity").join("panic.log");
            if let Err(e) = std::fs::create_dir_all(log_path.parent().unwrap_or(&data_dir)) {
                eprintln!("Failed to create panic log dir: {}", e);
            }
            if let Err(e) = std::fs::write(&log_path, report) {
                eprintln!("Failed to write panic log: {}", e);
            }
        }
    }));

    // Load settings early so we can restore the saved window position before
    // the window is created.
    let settings_early = crate::settings::GuiSettings::load();
    let options = eframe::NativeOptions {
        viewport: {
            let theme_defaults = crate::theme::Theme::default();
            let mut builder = egui::ViewportBuilder::default()
                .with_inner_size([
                    theme_defaults.window_default_w,
                    theme_defaults.window_default_h,
                ])
                .with_min_inner_size([theme_defaults.window_min_w, theme_defaults.window_min_h])
                .with_decorations(false);
            if let (Some(x), Some(y)) = (settings_early.window_x, settings_early.window_y) {
                builder = builder.with_position([x, y]);
            }
            builder
        },
        ..Default::default()
    };

    // ------------------------------------------------------------------
    // Auto-start Gateway if not already running
    // ------------------------------------------------------------------
    let gateway_manager = crate::services::gateway_manager::GatewayManager::new();
    let gateway_manager = match gateway_manager.start_if_needed() {
        Ok(true) => {
            tracing::info!("Auto-started Gateway");
            Some(gateway_manager)
        }
        Ok(false) => {
            tracing::info!("Gateway already running — no action needed");
            None
        }
        Err(e) => {
            tracing::warn!("Could not auto-start Gateway: {}", e);
            None
        }
    };

    let pre_window_elapsed = startup_t0.elapsed();
    tracing::info!("Startup: pre-window init took {:?}", pre_window_elapsed);

    eframe::run_native(
        "Clarity",
        options,
        Box::new(move |cc| {
            let app_creation_t0 = Instant::now();
            #[cfg(windows)]
            let _ = platform::windows::apply_rounded_corners(cc);
            let tray_manager = crate::services::tray::TrayManager::new();
            if tray_manager.is_none() {
                tracing::warn!("Failed to initialize system tray icon");
            }
            let app = Box::new(App::new(cc, gateway_manager, tray_manager)?);
            tracing::info!(
                "Startup: App creation took {:?} (total {:?})",
                app_creation_t0.elapsed(),
                startup_t0.elapsed()
            );
            Ok(app)
        }),
    )
}

/// S7 Phase 2D: extract plain text from a RenderLine for copy-to-clipboard.
fn render_line_text(line: &clarity_core::ui::RenderLine) -> String {
    use clarity_core::ui::RenderLine;
    match line {
        RenderLine::Text { spans, .. } => spans.iter().map(|s| s.text.as_str()).collect(),
        RenderLine::CodeLine { content, .. } => content.to_string(),
        RenderLine::ToolCallHeader { name, .. } => format!("🔧 {name}"),
        RenderLine::ToolCallArg { key, value } => format!("{key}: {value}"),
        RenderLine::Thinking { content, .. } => content.to_string(),
        RenderLine::ApprovalPrompt { options } => options
            .iter()
            .map(|o| format!("{:?}", o))
            .collect::<Vec<_>>()
            .join(" | "),
        RenderLine::StatusLine { content, .. } => content.to_string(),
        RenderLine::ArtifactRef {
            artifact_id,
            summary,
        } => {
            format!("{artifact_id} — {summary}")
        }
        RenderLine::CrossInstanceRef {
            target_instance,
            message,
            ..
        } => {
            format!("@{target_instance} — {message}")
        }
        RenderLine::SlashCompletion {
            command,
            description,
        } => {
            format!("/{command}  {description}")
        }
        RenderLine::StreamingCursor => "▌".to_string(),
        RenderLine::Divider => "───".to_string(),
        RenderLine::Empty => String::new(),
        RenderLine::BlockSlot {
            block_id,
            line_count,
        } => {
            format!("⤢ Block {block_id} ({line_count} lines)")
        }
    }
}

#[cfg(test)]
mod pretext_alignment;
#[cfg(test)]
mod test_util;
