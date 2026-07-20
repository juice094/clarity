//! Clarity sub-applications.
//!
//! Each module here implements the `ClarityApp` trait from `clarity-shell`
//! for one top-level surface: Chat, Settings, Dashboard, and future surfaces.

#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, unsafe_code)
)]

pub mod about;
pub mod chat;
pub mod dashboard;
pub mod provider;
pub mod settings;
pub mod settings_data;
pub mod settings_panels;
pub mod test_util;

pub use about::AboutApp;
pub use chat::{ChatApp, ChatStore};
pub use dashboard::{CronStore, DashboardApp, SubAgentStore, TaskStore, TeamStore};
pub use settings::{KimiCodeLoginState, SettingsApp, SettingsStore};
pub use settings_data::{GuiSettings, OpenClawConnection, WebTab};

/// Unified enum for all Clarity sub-applications hosted inside the egui shell.
///
/// P1d: replaces the three independent fields on `clarity_egui::App` with a
/// single indexed array so `render_main_stage` can dispatch by `AppView`
/// without repeating the same `ClarityAppContext` construction three times.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum ClarityAppEnum {
    /// Chat / conversation surface.
    Chat(ChatApp),
    /// Settings / configuration surface.
    Settings(SettingsApp),
    /// System dashboard surface.
    Dashboard(DashboardApp),
}

impl Default for ClarityAppEnum {
    fn default() -> Self {
        Self::Chat(ChatApp::default())
    }
}

impl ClarityAppEnum {
    /// Borrow the chat variant, if this is one.
    pub fn as_chat(&self) -> Option<&ChatApp> {
        match self {
            Self::Chat(app) => Some(app),
            _ => None,
        }
    }

    /// Mutably borrow the chat variant, if this is one.
    pub fn as_chat_mut(&mut self) -> Option<&mut ChatApp> {
        match self {
            Self::Chat(app) => Some(app),
            _ => None,
        }
    }

    /// Take the chat app out, leaving a default `ChatApp` in its place.
    ///
    /// ponytail: this is a temporary seam used by `clarity-egui` while the
    /// `ChatRenderer` callback still needs exclusive access to both the host
    /// `App` and the `ChatApp`. Once the chat render body moves fully into
    /// `clarity-apps` or receives a trait-based host state instead of `&mut App`,
    /// this helper can be removed.
    pub fn take_chat(&mut self) -> ChatApp {
        let mut taken = Self::Chat(ChatApp::default());
        std::mem::swap(&mut taken, self);
        match taken {
            Self::Chat(app) => app,
            _ => unreachable!("take_chat called on non-chat variant"),
        }
    }

    /// Borrow the settings variant, if this is one.
    pub fn as_settings(&self) -> Option<&SettingsApp> {
        match self {
            Self::Settings(app) => Some(app),
            _ => None,
        }
    }

    /// Mutably borrow the settings variant, if this is one.
    pub fn as_settings_mut(&mut self) -> Option<&mut SettingsApp> {
        match self {
            Self::Settings(app) => Some(app),
            _ => None,
        }
    }

    /// Take the settings app out, leaving a default `SettingsApp` in its place.
    ///
    /// ponytail: kept for symmetry with `take_chat` while the chrome render
    /// path uses temporary move-then-restore to avoid `&mut App` aliasing.
    pub fn take_settings(&mut self) -> SettingsApp {
        let mut taken = Self::Settings(SettingsApp::default());
        std::mem::swap(&mut taken, self);
        match taken {
            Self::Settings(app) => app,
            _ => unreachable!("take_settings called on non-settings variant"),
        }
    }

    /// Borrow the dashboard variant, if this is one.
    pub fn as_dashboard(&self) -> Option<&DashboardApp> {
        match self {
            Self::Dashboard(app) => Some(app),
            _ => None,
        }
    }

    /// Mutably borrow the dashboard variant, if this is one.
    pub fn as_dashboard_mut(&mut self) -> Option<&mut DashboardApp> {
        match self {
            Self::Dashboard(app) => Some(app),
            _ => None,
        }
    }

    /// Take the dashboard app out, leaving a default `DashboardApp` in its place.
    ///
    /// ponytail: kept for symmetry with `take_chat` while the chrome render
    /// path uses temporary move-then-restore to avoid `&mut App` aliasing.
    pub fn take_dashboard(&mut self) -> DashboardApp {
        let mut taken = Self::Dashboard(DashboardApp::default());
        std::mem::swap(&mut taken, self);
        match taken {
            Self::Dashboard(app) => app,
            _ => unreachable!("take_dashboard called on non-dashboard variant"),
        }
    }
}

impl clarity_shell::ClarityApp for ClarityAppEnum {
    fn id(&self) -> &'static str {
        match self {
            Self::Chat(_) => "chat",
            Self::Settings(_) => "settings",
            Self::Dashboard(_) => "dashboard",
        }
    }

    fn title(&self, ctx: &clarity_shell::ClarityAppContext<'_>) -> String {
        match self {
            Self::Chat(app) => app.title(ctx),
            Self::Settings(app) => app.title(ctx),
            Self::Dashboard(app) => app.title(ctx),
        }
    }

    fn update(&mut self, ctx: &mut clarity_shell::ClarityAppContext<'_>, egui_ctx: &egui::Context) {
        match self {
            Self::Chat(app) => app.update(ctx, egui_ctx),
            Self::Settings(app) => app.update(ctx, egui_ctx),
            Self::Dashboard(app) => app.update(ctx, egui_ctx),
        }
    }

    fn render(
        &mut self,
        ctx: &mut clarity_shell::ClarityAppContext<'_>,
        ui: &mut egui::Ui,
        egui_ctx: &egui::Context,
    ) -> clarity_shell::ClarityAppResponse {
        match self {
            Self::Chat(app) => app.render(ctx, ui, egui_ctx),
            Self::Settings(app) => app.render(ctx, ui, egui_ctx),
            Self::Dashboard(app) => app.render(ctx, ui, egui_ctx),
        }
    }

    fn tab_notifications(&self) -> usize {
        match self {
            Self::Chat(app) => app.tab_notifications(),
            Self::Settings(app) => app.tab_notifications(),
            Self::Dashboard(app) => app.tab_notifications(),
        }
    }
}
