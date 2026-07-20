//! Global status banner that surfaces system health to the user.
//!
//! The banner is rendered at the top of the window, below the custom titlebar.
//! It shows the highest-priority issue from [`SystemHealth`] and offers a
//! dismiss button plus an optional retry action.

use crate::stores::{HealthState, SystemHealth};

/// Priority level used to pick banner colors.
#[derive(Clone, Copy, Debug, PartialEq)]
enum BannerLevel {
    Error,
    Warning,
}

/// Render the system-health status banner if there is anything to show.
pub fn render_status_banner(app: &mut crate::App, ui: &mut egui::Ui) {
    let health = app.system_health_store.get();
    let theme = app.ui_store.theme.clone();

    let (level, title, message, retry) = summarize_health(&health);

    if title.is_empty() {
        return;
    }

    let (bg, fg, border) = match level {
        BannerLevel::Error => (
            theme.danger.linear_multiply(0.15),
            theme.danger,
            theme.danger,
        ),
        BannerLevel::Warning => (theme.warn.linear_multiply(0.15), theme.warn, theme.warn),
    };

    clarity_ui::design_system::Elevation::Base
        .frame(&theme)
        .fill(bg)
        .stroke(egui::Stroke::new(1.0, border))
        .inner_margin(egui::Margin::symmetric(
            theme.space_12 as i8,
            theme.space_8 as i8,
        ))
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                clarity_ui::design_system::text_with_color(
                    ui,
                    title,
                    clarity_ui::design_system::TextStyle::Body.strong(),
                    fg,
                );
                clarity_ui::design_system::text_with_color(
                    ui,
                    message,
                    clarity_ui::design_system::TextStyle::Body,
                    theme.text,
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(egui::RichText::new(app.t("Dismiss")).color(theme.text_dim))
                        .clicked()
                    {
                        app.system_health_store.clear_error();
                    }
                    if retry
                        && ui
                            .button(egui::RichText::new(app.t("Retry")).color(fg))
                            .clicked()
                    {
                        app.request_health_recheck();
                    }
                });
            });
        });
}

/// Pick the highest-priority message from the aggregated health state.
///
/// Returns `(level, title, message, retry_allowed)`.
fn summarize_health(health: &SystemHealth) -> (BannerLevel, String, String, bool) {
    if let Some(err) = &health.last_error {
        return (
            BannerLevel::Error,
            err.title.clone(),
            err.message.clone(),
            true,
        );
    }

    if let HealthState::Unhealthy { message } = &health.network {
        return (
            BannerLevel::Error,
            "Offline".to_string(),
            message.clone(),
            true,
        );
    }

    for provider in health.providers.values() {
        match &provider.state {
            HealthState::Unhealthy { message } => {
                return (
                    BannerLevel::Error,
                    format!("{} unavailable", provider.name),
                    message.clone(),
                    true,
                );
            }
            HealthState::Degraded { message } => {
                return (
                    BannerLevel::Warning,
                    format!("{} degraded", provider.name),
                    message.clone(),
                    true,
                );
            }
            _ => {}
        }
    }

    if let HealthState::Unhealthy { message } = &health.memory {
        return (
            BannerLevel::Warning,
            "Memory store unavailable".to_string(),
            message.clone(),
            false,
        );
    }

    if let HealthState::Unhealthy { message } = &health.gateway {
        return (
            BannerLevel::Warning,
            "Gateway unavailable".to_string(),
            message.clone(),
            false,
        );
    }

    if let HealthState::Unhealthy { message } = &health.mcp {
        return (
            BannerLevel::Warning,
            "MCP unavailable".to_string(),
            message.clone(),
            true,
        );
    }

    (BannerLevel::Error, String::new(), String::new(), false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::{HealthState, SystemHealthStore};

    #[test]
    fn empty_health_no_banner() {
        let health = SystemHealth::default();
        let (level, title, _, _) = summarize_health(&health);
        assert!(title.is_empty());
        // Default enum is Unknown, not Error, but the function only returns
        // Error when there is a real issue. With an empty title the caller
        // skips rendering, so the level is irrelevant.
        assert_eq!(level, BannerLevel::Error);
    }

    #[test]
    fn unhealthy_network_shows_error() {
        let store = SystemHealthStore::new();
        store.set_network(HealthState::Unhealthy {
            message: "No internet".into(),
        });
        let (level, title, _, retry) = summarize_health(&store.get());
        assert_eq!(level, BannerLevel::Error);
        assert_eq!(title, "Offline");
        assert!(retry);
    }

    #[test]
    fn degraded_provider_shows_warning() {
        let store = SystemHealthStore::new();
        store.set_provider(
            "kimi",
            HealthState::Degraded {
                message: "slow".into(),
            },
        );
        let (level, title, _, retry) = summarize_health(&store.get());
        assert_eq!(level, BannerLevel::Warning);
        assert_eq!(title, "kimi degraded");
        assert!(retry);
    }
}
