//! Right rail — Tools card.

use crate::App;

/// Render tools summary (skills + MCP) into the right rail.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    ui.label(
        egui::RichText::new("Tools")
            .size(theme.text_base)
            .strong()
            .color(theme.text),
    );
    ui.add_space(theme.space_12);

    // ── Skills ──
    let skills = app.state.agent.list_skills();
    let active_ids = app.state.agent.skill_active_ids();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "Skills: {} / {} active",
                active_ids.len(),
                skills.len()
            ))
            .size(theme.text_sm)
            .color(theme.text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(
                    egui::RichText::new("Manage")
                        .size(theme.text_xs)
                        .color(theme.text),
                )
                .clicked()
            {
                app.view_state
                    .open_modal(clarity_core::ui::ModalType::Skill);
            }
        });
    });

    if !skills.is_empty() {
        ui.add_space(theme.space_4);
        egui::Frame::new()
            .fill(theme.surface)
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                egui::ScrollArea::vertical()
                    .max_height(120.0)
                    .show(ui, |ui| {
                        for skill in &skills {
                            let active = active_ids.contains(&skill.meta.id);
                            let marker = if active { "●" } else { "○" };
                            let color = if active { theme.ok } else { theme.text_dim };
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(marker).size(theme.text_xs).color(color),
                                );
                                ui.label(
                                    egui::RichText::new(&skill.meta.name)
                                        .size(theme.text_xs)
                                        .color(theme.text),
                                );
                            });
                        }
                    });
            });
    }

    ui.add_space(theme.space_16);

    // ── MCP ──
    let server_count = app
        .mcp_store
        .mcp_config
        .as_ref()
        .map(|c| c.servers.len())
        .unwrap_or(0);
    let tool_count = app.mcp_store.connected_tools.len();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "MCP: {} servers, {} tools",
                server_count, tool_count
            ))
            .size(theme.text_sm)
            .color(theme.text_dim),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .button(
                    egui::RichText::new("Configure")
                        .size(theme.text_xs)
                        .color(theme.text),
                )
                .clicked()
            {
                app.view_state.open_modal(clarity_core::ui::ModalType::Mcp);
            }
        });
    });

    if !app.mcp_store.connected_tools.is_empty() {
        ui.add_space(theme.space_4);
        egui::Frame::new()
            .fill(theme.surface)
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                egui::ScrollArea::vertical()
                    .max_height(120.0)
                    .show(ui, |ui| {
                        for tool in &app.mcp_store.connected_tools {
                            ui.label(
                                egui::RichText::new(format!("• {}", tool))
                                    .size(theme.text_xs)
                                    .color(theme.text_dim),
                            );
                        }
                    });
            });
    }
}
