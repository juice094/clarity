//! Right rail — Status card.

use crate::App;

/// Render system + Agent status summary into the right rail.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    ui.label(
        egui::RichText::new("System Status")
            .size(theme.text_base)
            .strong()
            .color(theme.text),
    );
    ui.add_space(theme.space_12);

    // Agent status row
    let (status_label, status_color) = match app.chat_store.agent_status {
        crate::ui::types::AgentStatus::Online => ("Online", theme.status_online),
        crate::ui::types::AgentStatus::Busy => ("Busy", theme.status_busy),
        crate::ui::types::AgentStatus::Offline => ("Offline", theme.danger),
        crate::ui::types::AgentStatus::Unconfigured => ("Unconfigured", theme.text_dim),
    };

    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Agent:")
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        ui.label(
            egui::RichText::new(status_label)
                .size(theme.text_sm)
                .strong()
                .color(status_color),
        );
    });

    // Gateway status
    let (gw_label, gw_color) = match app.chat_store.gateway_status {
        crate::ui::types::GatewayStatus::Online => ("Online", theme.status_online),
        crate::ui::types::GatewayStatus::Offline => ("Offline", theme.danger),
        crate::ui::types::GatewayStatus::Checking => ("Checking", theme.status_busy),
    };
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Gateway:")
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        ui.label(
            egui::RichText::new(gw_label)
                .size(theme.text_sm)
                .strong()
                .color(gw_color),
        );
    });

    // Network banner
    if let Some(banner) = app.ui_store.network_banner.as_ref() {
        ui.add_space(theme.space_8);
        egui::Frame::new()
            .fill(theme.status_busy.linear_multiply(0.15))
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.label(
                    egui::RichText::new(banner)
                        .size(theme.text_sm)
                        .color(theme.status_busy),
                );
            });
    }

    // Token usage
    if let Some(usage) = app.chat_store.last_usage.as_ref() {
        ui.add_space(theme.space_12);
        ui.label(
            egui::RichText::new("Session Usage")
                .size(theme.text_sm)
                .strong()
                .color(theme.text),
        );
        ui.add_space(theme.space_4);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!("{:.2}K tokens", usage.2 as f64 / 1000.0))
                    .size(theme.text_xs)
                    .color(theme.text_dim),
            );
        });
    }
}
