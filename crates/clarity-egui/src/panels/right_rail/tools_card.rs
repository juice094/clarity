//! Right rail — Tools card.

use crate::App;
use crate::design_system::{self, ButtonStyle, Space, Surface, Text};

/// Render tools summary (skills + MCP) into the right rail.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    design_system::text(ui, "Tools", Text::BodyStrong);
    design_system::gap(ui, Space::S2);

    // ── Skills ──
    let skills = app.state.agent.list_skills();
    let active_ids = app.state.agent.skill_active_ids();
    let mut manage_clicked = false;
    ui.horizontal(|ui| {
        design_system::text(
            ui,
            format!("Skills: {} / {} active", active_ids.len(), skills.len()),
            Text::Caption,
        );
        design_system::push_right(ui);
        manage_clicked = design_system::btn(ui, "Manage", ButtonStyle::Ghost).clicked();
    });
    if manage_clicked {
        app.view_state
            .open_modal(clarity_core::ui::ModalType::Skill);
    }

    if !skills.is_empty() {
        design_system::gap(ui, Space::S0);
        design_system::surface(ui, Surface::Well, |ui| {
            ui.set_min_width(ui.available_width());
            design_system::scroll(ui, design_system::Scroll::VerticalMax(120.0), |ui| {
                for skill in &skills {
                    let active = active_ids.contains(&skill.meta.id);
                    let marker = if active { "●" } else { "○" };
                    let color = if active { theme.ok } else { theme.text_dim };
                    design_system::row(ui, |ui| {
                        ui.label(egui::RichText::new(marker).size(theme.text_xs).color(color));
                        design_system::gap(ui, Space::S0);
                        design_system::text(ui, &skill.meta.name, Text::Small);
                    });
                }
            });
        });
    }

    design_system::gap(ui, Space::S3);

    // ── MCP ──
    let server_count = app
        .mcp_store
        .mcp_config
        .as_ref()
        .map(|c| c.servers.len())
        .unwrap_or(0);
    let tool_count = app.mcp_store.connected_tools.len();
    let mut config_clicked = false;
    ui.horizontal(|ui| {
        design_system::text(
            ui,
            format!("MCP: {} servers, {} tools", server_count, tool_count),
            Text::Caption,
        );
        design_system::push_right(ui);
        config_clicked = design_system::btn(ui, "Configure", ButtonStyle::Ghost).clicked();
    });
    if config_clicked {
        app.view_state.open_modal(clarity_core::ui::ModalType::Mcp);
    }

    if !app.mcp_store.connected_tools.is_empty() {
        design_system::gap(ui, Space::S0);
        design_system::surface(ui, Surface::Well, |ui| {
            ui.set_min_width(ui.available_width());
            design_system::scroll(ui, design_system::Scroll::VerticalMax(120.0), |ui| {
                for tool in &app.mcp_store.connected_tools {
                    design_system::text(ui, format!("• {}", tool), Text::Small);
                }
            });
        });
    }
}
