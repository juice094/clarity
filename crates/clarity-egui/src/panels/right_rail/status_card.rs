//! Right rail — Status card.

use crate::App;
use crate::design_system::{self, Space, Surface, Text};

/// Render system + Agent status summary into the right rail.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    design_system::text(ui, "System Status", Text::BodyStrong);
    design_system::gap(ui, Space::S2);

    // Agent status row
    let (status_label, status_color) = match app.chat_store.agent_status {
        crate::ui::types::AgentStatus::Online => ("Online", app.ui_store.theme.status_online),
        crate::ui::types::AgentStatus::Busy => ("Busy", app.ui_store.theme.status_busy),
        crate::ui::types::AgentStatus::Offline => ("Offline", app.ui_store.theme.danger),
        crate::ui::types::AgentStatus::Unconfigured => {
            ("Unconfigured", app.ui_store.theme.text_dim)
        }
    };

    design_system::row(ui, |ui| {
        design_system::text(ui, "Agent:", Text::Caption);
        ui.label(
            egui::RichText::new(status_label)
                .size(app.ui_store.theme.text_sm)
                .strong()
                .color(status_color),
        );
    });

    // Gateway status
    let (gw_label, gw_color) = match app.chat_store.gateway_status {
        crate::ui::types::GatewayStatus::Online => ("Online", app.ui_store.theme.status_online),
        crate::ui::types::GatewayStatus::Offline => ("Offline", app.ui_store.theme.danger),
        crate::ui::types::GatewayStatus::Checking => ("Checking", app.ui_store.theme.status_busy),
    };

    design_system::row(ui, |ui| {
        design_system::text(ui, "Gateway:", Text::Caption);
        ui.label(
            egui::RichText::new(gw_label)
                .size(app.ui_store.theme.text_sm)
                .strong()
                .color(gw_color),
        );
    });

    // Network banner
    if let Some(banner) = app.ui_store.network_banner.as_ref() {
        design_system::gap(ui, Space::S1);
        design_system::surface(ui, Surface::Warning, |ui| {
            ui.set_min_width(ui.available_width());
            design_system::text(ui, banner, Text::Caption);
        });
    }

    // Token usage
    if let Some(usage) = app.chat_store.last_usage.as_ref() {
        design_system::gap(ui, Space::S2);
        design_system::text(ui, "Session Usage", Text::CaptionStrong);
        design_system::gap(ui, Space::S0);
        design_system::row(ui, |ui| {
            design_system::text(
                ui,
                format!("{:.2}K tokens", usage.2 as f64 / 1000.0),
                Text::Small,
            );
        });
    }
}
