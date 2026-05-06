use crate::theme::Theme;
use clarity_core::mcp::config::{McpConfig, McpServerEntry};

// ============================================================================
// MCP Configuration Panel
// ============================================================================

pub fn render_mcp_panel(
    ui: &mut egui::Ui,
    config: &mut McpConfig,
    theme: &Theme,
    changed: &mut bool,
) {
    ui.add_space(theme.space_8);

    if config.servers.is_empty() {
        ui.vertical_centered(|ui| {
            ui.add_space(theme.space_40);
            ui.label(
                egui::RichText::new("No MCP servers configured")
                    .size(theme.text_base)
                    .color(theme.text_dim),
            );
            ui.add_space(theme.space_8);
            ui.label(
                egui::RichText::new("Add servers to ~/.config/clarity/mcp.json")
                    .size(theme.text_sm)
                    .color(theme.text_dim),
            );
        });
        return;
    }

    // Sort servers by name for stable display
    let mut servers: Vec<(String, McpServerEntry)> = config
        .servers
        .iter()
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    servers.sort_by(|a, b| a.0.cmp(&b.0));

    for (name, entry) in servers.iter_mut() {
        egui::Frame::group(ui.style())
            .fill(theme.surface)
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
            .stroke(egui::Stroke::new(1.0_f32, theme.border))
            .inner_margin(egui::Margin::same(10))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());

                // Header row: name + toggle
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(name.as_str())
                            .size(theme.text_base)
                            .strong()
                            .color(theme.text),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let mut enabled = !entry.disabled;
                        if ui.checkbox(&mut enabled, "Enabled").changed() {
                            entry.disabled = !enabled;
                            *changed = true;
                        }
                    });
                });

                ui.add_space(theme.space_4);

                // Command
                if !entry.command.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Command:")
                                .size(theme.text_sm)
                                .color(theme.text_dim),
                        );
                        ui.label(
                            egui::RichText::new(&entry.command)
                                .size(theme.text_sm)
                                .monospace()
                                .color(theme.text_muted),
                        );
                    });
                }

                // Args
                if !entry.args.is_empty() {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Args:")
                                .size(theme.text_sm)
                                .color(theme.text_dim),
                        );
                        ui.label(
                            egui::RichText::new(entry.args.join(" "))
                                .size(theme.text_sm)
                                .monospace()
                                .color(theme.text_muted),
                        );
                    });
                }

                // URL (for HTTP/SSE transport)
                if let Some(ref url) = entry.url {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("URL:")
                                .size(theme.text_sm)
                                .color(theme.text_dim),
                        );
                        ui.label(
                            egui::RichText::new(url)
                                .size(theme.text_sm)
                                .monospace()
                                .color(theme.text_muted),
                        );
                    });
                }

                // Transport
                if let Some(ref transport) = entry.transport {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("Transport:")
                                .size(theme.text_sm)
                                .color(theme.text_dim),
                        );
                        ui.label(
                            egui::RichText::new(transport)
                                .size(theme.text_sm)
                                .color(theme.text_muted),
                        );
                    });
                }

                // Env vars
                if !entry.env.is_empty() {
                    ui.add_space(2.0);
                    ui.label(
                        egui::RichText::new("Environment:")
                            .size(theme.text_sm)
                            .color(theme.text_dim),
                    );
                    for (k, v) in &entry.env {
                        ui.horizontal(|ui| {
                            ui.add_space(theme.space_8);
                            ui.label(
                                egui::RichText::new(format!("{}={}", k, v))
                                    .size(theme.text_xs)
                                    .monospace()
                                    .color(theme.text_dim),
                            );
                        });
                    }
                }
            });
        ui.add_space(theme.space_8);
    }

    // Write back modified entries
    for (name, entry) in servers {
        config.servers.insert(name, entry);
    }
}

/// Load MCP config from default path.
pub fn load_mcp_config() -> Option<McpConfig> {
    match McpConfig::load_default() {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            tracing::warn!("MCP config not found or invalid: {}", e);
            None
        }
    }
}

/// Save MCP config to default path.
pub fn save_mcp_config(config: &McpConfig) -> Result<(), String> {
    let path = clarity_core::mcp::config::default_config_path()
        .map_err(|e| format!("Failed to get MCP config path: {}", e))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())?;
    tracing::info!("MCP config saved to {}", path.display());
    Ok(())
}
