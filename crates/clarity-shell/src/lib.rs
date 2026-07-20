//! Clarity egui application host.
//!
//! The shell provides the `ClarityApp` trait, the `ClarityAppContext` dependency
//! injection bundle, and the `ClarityHost` that implements `eframe::App` and
//! schedules sub-applications. It is analogous to the `notedeck` crate in the
//! notedeck architecture.

#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, unsafe_code)
)]

pub use clarity_ui;

// ============================================================================
// Host abstractions for sub-applications
// ============================================================================

/// Toast severity level.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ToastLevel {
    /// Informational toast.
    Info,
    /// Warning toast.
    Warn,
    /// Error toast.
    Error,
}

/// Lifecycle status of an OpenClaw bot instance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BotStatus {
    /// Bot is online and reachable.
    Online,
    /// Bot is offline or unreachable.
    Offline,
    /// Bot is synchronising.
    Syncing,
}

/// Summary of the active OpenClaw bot instance for UI panels.
#[derive(Clone, Debug)]
pub struct BotInfo {
    /// Bot instance id.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Gateway-assigned device id.
    pub device_id: String,
    /// Software version reported by the bot.
    pub version: String,
    /// Human-readable last-backup timestamp/label.
    pub last_backup: String,
    /// Current status.
    pub status: BotStatus,
}

/// State of an in-app OpenClaw device-pairing flow.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum PairingState {
    /// No pairing in progress.
    #[default]
    Idle,
    /// Sending the pairing request.
    Requesting,
    /// Waiting for the user to approve the request in the Gateway UI.
    Waiting {
        /// Gateway URL being paired with.
        gateway_url: String,
        /// When the wait started.
        since: std::time::Instant,
    },
    /// Pairing approved and token saved.
    Approved {
        /// Gateway URL that was paired.
        gateway_url: String,
        /// Device token returned by the Gateway.
        token: String,
    },
    /// Pairing failed or timed out.
    Error(String),
}

/// Optional render extension point for apps whose concrete rendering is still
/// co-located with the host crate.
///
/// `clarity-apps` owns the `ChatApp` shell, but the egui render body remains
/// in `clarity-egui` during the P1c migration. The host implements this trait
/// for its concrete `AppState` and returns it from [`AppState::chat_renderer`].
///
/// ponytail: this is a temporary seam; once panel helpers move to
/// `clarity-apps`, `ChatApp` can render directly and this trait can be removed.
pub trait ChatRenderer {
    /// Render the chat application. `chat` is the `clarity_apps::ChatApp`
    /// shell passed as `dyn Any` to avoid a circular dependency between
    /// `clarity-shell` and `clarity-apps`.
    ///
    /// `ClarityAppContext` is intentionally not passed here: the host already
    /// has `&mut self` as its concrete `AppState`, so it can read theme,
    /// provider, model, etc. directly. This avoids a double-borrow of the
    /// context when `ChatApp::render` retrieves the renderer.
    fn render_chat(
        &mut self,
        chat: &mut dyn std::any::Any,
        ui: &mut egui::Ui,
        egui_ctx: &egui::Context,
    ) -> ClarityAppResponse;
}

/// Shared application state exposed to every `ClarityApp`.
///
/// Implementors live in the concrete host crate (e.g. `clarity-egui`). The
/// trait carries `Any` so apps that are still co-located with the host can
/// downcast to the concrete type during the migration window; over time those
/// downcasts are replaced by additional trait methods.
pub trait AppState: std::any::Any {
    /// Borrow as `dyn Any` for downcasting.
    fn as_any(&self) -> &dyn std::any::Any;
    /// Borrow mutably as `dyn Any` for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
    /// Current theme.
    fn theme(&self) -> &clarity_ui::theme::Theme;
    /// Mutable theme.
    fn theme_mut(&mut self) -> &mut clarity_ui::theme::Theme;
    /// Localized string lookup. Default returns the key unchanged.
    fn t(&self, key: &'static str) -> &'static str {
        key
    }
    /// Number of messages in the active session.
    fn session_message_count(&self) -> usize {
        0
    }
    /// Number of tool calls in the active session.
    fn session_tool_call_count(&self) -> usize {
        0
    }
    /// Token count for the active session, if available.
    fn session_token_count(&self) -> Option<u32> {
        None
    }
    /// Label for the current agent status.
    fn agent_status_label(&self) -> &'static str {
        "Offline"
    }
    /// Color for the current agent status indicator.
    fn agent_status_color(&self) -> egui::Color32 {
        egui::Color32::from_gray(128)
    }
    /// Label for the current gateway status.
    fn gateway_status_label(&self) -> &'static str {
        "Offline"
    }
    /// Color for the current gateway status indicator.
    fn gateway_status_color(&self) -> egui::Color32 {
        egui::Color32::from_gray(128)
    }
    /// Active LLM provider name.
    fn active_provider(&self) -> &str {
        ""
    }
    /// Active LLM model name.
    fn active_model(&self) -> &str {
        ""
    }
    /// Current instantaneous frames per second.
    fn fps(&self) -> f64 {
        0.0
    }
    /// Maximum content width for the central stage.
    fn content_max_width(&self) -> f32 {
        0.0
    }
    /// Optional chat renderer provided by the host. Returns `None` when the
    /// host has not implemented chat rendering (e.g. headless or test states).
    fn chat_renderer(&mut self) -> Option<&mut dyn ChatRenderer> {
        None
    }

    // ── Settings surface host hooks (P1c) ──

    /// Navigate to the given route.
    fn navigate(&mut self, _route: clarity_core::ui::Route) {}

    /// Show a transient toast.
    fn push_toast(&mut self, _message: String, _level: ToastLevel) {}

    /// Open the given modal.
    fn open_modal(&mut self, _modal: clarity_core::ui::ModalType) {}

    /// Apply a new theme (including any transition animation).
    fn set_theme(&mut self, _theme: clarity_ui::theme::Theme) {}

    /// Set the global font scale.
    fn set_font_scale(&mut self, _scale: f32) {}

    /// Increase the global font scale.
    fn increase_font_scale(&mut self) {}

    /// Decrease the global font scale.
    fn decrease_font_scale(&mut self) {}

    /// Persist layout-related settings (right rail, debug overlay, etc.).
    fn persist_layout_settings(&mut self) {}

    /// Trigger an asynchronous save of the settings file.
    fn auto_save_settings(&mut self) {}

    /// Set the maximum content width for the central stage.
    fn set_content_max_width(&mut self, _width: f32) {}

    /// Whether the layout debug overlay is enabled.
    fn debug_layout_overlay(&self) -> bool {
        false
    }

    /// Enable or disable the layout debug overlay.
    fn set_debug_layout_overlay(&mut self, _value: bool) {}

    /// Current UI locale.
    fn locale(&self) -> clarity_ui::i18n::Locale {
        clarity_ui::i18n::Locale::default()
    }

    /// Set the UI locale.
    fn set_locale(&mut self, _locale: clarity_ui::i18n::Locale) {}

    /// Open the Pretext measurement probe window.
    fn set_pretext_probe_open(&mut self, _open: bool) {}

    /// Whether Pretext height estimation is enabled.
    fn pretext_estimate_enabled(&self) -> bool {
        false
    }

    /// Enable or disable Pretext height estimation.
    fn set_pretext_estimate_enabled(&mut self, _enabled: bool) {}

    /// Current OpenClaw pairing state.
    fn claw_pairing_state(&self) -> PairingState {
        PairingState::Idle
    }

    /// Start pairing the OpenClaw connection at the given index.
    fn start_openclaw_pairing(&mut self, _index: usize) {}

    /// Cancel the in-progress OpenClaw pairing flow.
    fn cancel_openclaw_pairing(&mut self) {}

    /// The currently active OpenClaw bot, if any.
    fn active_bot(&self) -> Option<BotInfo> {
        None
    }

    /// Spawn an asynchronous provider connection test.
    ///
    /// The host is responsible for constructing the runtime config, running
    /// `clarity_llm::runtime::test_connection`, and sending the result back as
    /// a UI event.
    fn spawn_provider_test(
        &self,
        _provider_id: String,
        _base_url: String,
        _api_format: String,
        _api_key: String,
        _model: String,
    ) {
    }

    /// Spawn an asynchronous provider model-list refresh.
    ///
    /// The host is responsible for constructing the runtime config, running
    /// `clarity_llm::runtime::list_models`, and sending the result back as a
    /// UI event.
    fn spawn_provider_refresh(
        &self,
        _provider_id: String,
        _base_url: String,
        _api_format: String,
        _api_key: String,
        _model: String,
    ) {
    }
}

/// Mutable dependency bundle passed to every `ClarityApp` each frame.
///
/// This is intentionally a flat struct of references so apps do not need to
/// know about the concrete host type. The `state` field is a trait object;
/// sub-apps downcast or use the `AppState` methods to access shared state.
pub struct ClarityAppContext<'a> {
    /// Active theme/token set.
    pub theme: &'a mut clarity_ui::theme::Theme,
    /// Application name (e.g. "Clarity").
    pub app_name: &'a str,
    /// Application version (e.g. "0.4.0").
    pub app_version: &'a str,
    /// Application description.
    pub app_description: &'a str,
    /// Application license.
    pub app_license: &'a str,
    /// Application-specific shared state.
    pub state: &'a mut dyn AppState,
}

/// Actions an app can request from the chrome by returning them from `render`.
#[derive(Clone, Debug, PartialEq)]
pub enum ClarityAppResponse {
    /// No action.
    None,
    /// Show a transient toast to the user.
    Toast(String),
    /// Ask the chrome to navigate to another route.
    Navigate(clarity_core::ui::Route),
    /// Request keyboard focus on the chat input field.
    FocusChatInput,
}

/// Lifecycle contract for every Clarity sub-application.
///
/// Mirrors `notedeck::App`: apps receive a mutable context and an egui Ui,
/// and return a response that the chrome can act on.
pub trait ClarityApp {
    /// Stable app identifier used for routing and persistence.
    fn id(&self) -> &'static str;

    /// Human-readable title for tabs, sidebars, and window chrome.
    fn title(&self, _ctx: &ClarityAppContext<'_>) -> String {
        self.id().to_string()
    }

    /// Background update hook called every frame before `render`.
    fn update(&mut self, _ctx: &mut ClarityAppContext<'_>, _egui_ctx: &egui::Context) {}

    /// Render the app.
    ///
    /// Sub-apps may draw into the supplied `ui` (central panels, side panels)
    /// or create floating windows via `egui_ctx`. The chrome decides which app
    /// receives input focus and screen real estate.
    fn render(
        &mut self,
        ctx: &mut ClarityAppContext<'_>,
        ui: &mut egui::Ui,
        egui_ctx: &egui::Context,
    ) -> ClarityAppResponse;

    /// Optional badge count shown on the app tab or sidebar entry.
    fn tab_notifications(&self) -> usize {
        0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_ui::theme::Theme;

    /// Dummy state implementing `AppState` for unit tests.
    struct DummyState;

    impl AppState for DummyState {
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

    /// A dummy app used to exercise the default `ClarityApp` method implementations.
    struct DummyApp;

    impl ClarityApp for DummyApp {
        fn id(&self) -> &'static str {
            "dummy"
        }

        fn render(
            &mut self,
            _ctx: &mut ClarityAppContext<'_>,
            _ui: &mut egui::Ui,
            _egui_ctx: &egui::Context,
        ) -> ClarityAppResponse {
            ClarityAppResponse::None
        }
    }

    #[test]
    fn context_construction_holds_references() {
        let mut theme = Theme::dark();
        let mut state = DummyState;
        let ctx = ClarityAppContext {
            theme: &mut theme,
            app_name: "Clarity",
            app_version: "0.4.0",
            app_description: "Test app",
            app_license: "AGPL-3.0-or-later",
            state: &mut state,
        };
        assert_eq!(ctx.app_name, "Clarity");
        assert_eq!(ctx.app_version, "0.4.0");
        assert_eq!(ctx.app_license, "AGPL-3.0-or-later");
    }

    #[test]
    fn default_title_uses_id() {
        let mut theme = Theme::dark();
        let mut state = DummyState;
        let ctx = ClarityAppContext {
            theme: &mut theme,
            app_name: "Clarity",
            app_version: "0.4.0",
            app_description: "Test app",
            app_license: "AGPL-3.0-or-later",
            state: &mut state,
        };
        let app = DummyApp;
        assert_eq!(app.title(&ctx), "dummy");
    }

    #[test]
    fn default_update_is_noop() {
        let mut theme = Theme::dark();
        let mut state = DummyState;
        let mut ctx = ClarityAppContext {
            theme: &mut theme,
            app_name: "Clarity",
            app_version: "0.4.0",
            app_description: "Test app",
            app_license: "AGPL-3.0-or-later",
            state: &mut state,
        };
        let mut app = DummyApp;
        let egui_ctx = egui::Context::default();
        app.update(&mut ctx, &egui_ctx);
        // The default update does nothing; reaching this line is the assertion.
        assert_eq!(app.tab_notifications(), 0);
    }

    #[test]
    fn id_is_exposed() {
        let app = DummyApp;
        assert_eq!(app.id(), "dummy");
    }

    /// Variant of `DummyApp` that returns a non-default response so the render
    /// path can be asserted end-to-end.
    struct ResponsiveDummyApp;

    impl ClarityApp for ResponsiveDummyApp {
        fn id(&self) -> &'static str {
            "responsive_dummy"
        }

        fn render(
            &mut self,
            _ctx: &mut ClarityAppContext<'_>,
            _ui: &mut egui::Ui,
            _egui_ctx: &egui::Context,
        ) -> ClarityAppResponse {
            ClarityAppResponse::Toast("hello".into())
        }
    }

    #[test]
    fn render_returns_response() {
        let mut app = ResponsiveDummyApp;
        let mut theme = Theme::dark();
        let mut state = DummyState;
        let mut ctx = ClarityAppContext {
            theme: &mut theme,
            app_name: "Test",
            app_version: "0.0.0",
            app_description: "",
            app_license: "",
            state: &mut state,
        };
        let egui_ctx = egui::Context::default();

        let _output = egui_ctx.run_ui(egui::RawInput::default(), |egui_ctx| {
            egui::Area::new("render_response_test".into()).show(egui_ctx, |ui| {
                let response = app.render(&mut ctx, ui, egui_ctx);
                assert_eq!(response, ClarityAppResponse::Toast("hello".into()));
            });
        });
    }
}
