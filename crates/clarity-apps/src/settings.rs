#![allow(missing_docs)]
//! Settings app — main rendering entry for the settings surface.
//!
//! The high-level shell (scrim, overlay, tab bar) lives here. Per-tab content
//! is delegated to `crate::settings_panels`.
//!
//! ponytail: `SettingsStore` and `KimiCodeLoginState` live here and are owned by
//! `SettingsApp::store`. The rendering code receives the store directly so the
//! egui `App` no longer holds this sub-application state.

use crate::settings_data::{GuiSettings, OpenClawConnection};
use crate::settings_panels;
use clarity_core::view_models::settings::SettingsViewModel;
use clarity_shell::{AppState, ClarityApp, ClarityAppContext, ClarityAppResponse};
use clarity_ui::design_system::{Space, gap};
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::overlay::{Overlay, overlay_scrim};

/// State variants for kimi code login.
#[derive(Clone, Debug, Default)]
pub enum KimiCodeLoginState {
    #[default]
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

/// Holds settings UI state.
#[derive(Clone, Debug, Default)]
pub struct SettingsStore {
    pub settings_edit: GuiSettings,
    pub settings_vm: SettingsViewModel,
    pub settings_active_tab: u8,
    pub show_add_provider: bool,
    pub add_provider_name: String,
    pub add_provider_url: String,
    pub add_provider_key: String,
    pub add_provider_format: String,
    pub provider_registry: crate::provider::ProviderRegistry,
    pub testing_provider: Option<String>,
    /// Current state of the Kimi Code OAuth login flow.
    pub kimi_code_login_state: KimiCodeLoginState,
    /// Index of the OpenClaw connection currently being edited in settings.
    pub claw_editing_index: Option<usize>,
    /// Form state for adding or editing an OpenClaw connection.
    pub claw_form: OpenClawConnection,
}

/// Settings / configuration sub-application.
#[derive(Clone, Debug, Default)]
pub struct SettingsApp {
    /// Settings UI state owned by this sub-application.
    pub store: SettingsStore,
}

impl SettingsApp {
    /// Create a new settings app instance.
    pub fn new() -> Self {
        Self::default()
    }
}

impl ClarityApp for SettingsApp {
    fn id(&self) -> &'static str {
        "settings"
    }

    fn title(&self, ctx: &ClarityAppContext<'_>) -> String {
        ctx.state.t("Settings").to_string()
    }

    fn render(
        &mut self,
        ctx: &mut ClarityAppContext<'_>,
        _ui: &mut egui::Ui,
        egui_ctx: &egui::Context,
    ) -> ClarityAppResponse {
        render_settings(&mut self.store, ctx.state, egui_ctx);
        ClarityAppResponse::None
    }
}

/// Settings tab variants.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingsTab {
    Provider,
    Interface,
    Ops,
    Claw,
    About,
}

/// Fixed content height so all tabs feel equally sized.
const CONTENT_HEIGHT: f32 = 420.0;

/// Renders the settings panel UI.
fn render_settings(store: &mut SettingsStore, state: &mut dyn AppState, ctx: &egui::Context) {
    let scrim_response = overlay_scrim(ctx);

    let tabs = [
        (SettingsTab::Provider, state.t("Provider")),
        (SettingsTab::Interface, state.t("Interface")),
        (SettingsTab::Ops, state.t("Ops")),
        (SettingsTab::Claw, state.t("Claw")),
        (SettingsTab::About, state.t("About")),
    ];
    let mut at = store.settings_active_tab;

    Overlay::new("settings")
        .width(640.0)
        .top_center(80.0)
        .show(ctx, |ui| {
            // ── Tab bar ──
            ui.horizontal(|ui| {
                ui.set_min_height(34.0);
                for (i, (_t, name)) in tabs.iter().enumerate() {
                    let is = i as u8 == at;
                    let btn = if is {
                        Button::new(name).primary().width(90.0)
                    } else {
                        Button::new(name).ghost().width(90.0)
                    };
                    if ui.add(btn).clicked() {
                        at = i as u8;
                    }
                }
            });

            gap(ui, Space::S2);

            // ── Content — fixed height for consistent feel across tabs ──
            ui.set_min_height(CONTENT_HEIGHT);
            match tabs[at as usize].0 {
                SettingsTab::Provider => {
                    settings_panels::provider_tab::render_provider(store, state, ui)
                }
                SettingsTab::Interface => {
                    settings_panels::interface_tab::render_interface(store, state, ui)
                }
                SettingsTab::Ops => settings_panels::ops_tab::render_ops(store, state, ui),
                SettingsTab::Claw => settings_panels::claw_tab::render_claw(store, state, ui),
                SettingsTab::About => settings_panels::about_tab::render_about(state, ui),
            }
        });

    store.settings_active_tab = at;
    if scrim_response.clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.navigate(clarity_core::ui::AppView::Chat.into());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_shell::{AppState, ClarityApp};
    use clarity_ui::theme::Theme;

    struct TestState {
        theme: Theme,
    }

    impl AppState for TestState {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
        fn theme(&self) -> &Theme {
            &self.theme
        }
        fn theme_mut(&mut self) -> &mut Theme {
            &mut self.theme
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
    fn settings_app_id_and_title() {
        let mut theme = Theme::dark();
        let mut state = TestState {
            theme: Theme::dark(),
        };
        let ctx = &mut test_context(&mut theme, &mut state);
        let settings = SettingsApp::new();
        assert_eq!(settings.id(), "settings");
        assert_eq!(settings.title(ctx), "Settings");
    }

    #[test]
    fn settings_app_renders_without_panic() {
        let egui_ctx = egui::Context::default();
        let mut theme = Theme::dark();
        let mut state = TestState {
            theme: Theme::dark(),
        };
        let mut settings = SettingsApp::new();

        let _output = egui_ctx.run_ui(egui::RawInput::default(), |egui_ctx| {
            egui::Area::new("settings_test".into()).show(egui_ctx, |ui| {
                let mut ctx = test_context(&mut theme, &mut state);
                let response = settings.render(&mut ctx, ui, egui_ctx);
                assert_eq!(response, ClarityAppResponse::None);
            });
        });
    }
}
