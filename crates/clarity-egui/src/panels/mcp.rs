use crate::App;

pub fn render_mcp_panel(app: &mut App, ctx: &egui::Context) {
    if !app.mcp_store.mcp_panel_open {
        return;
    }
    // Safety: if a previous panic left mcp_config as None, reload from disk.
    if app.mcp_store.mcp_config.is_none() {
        app.mcp_store.mcp_config = crate::ui::mcp_panel::load_mcp_config();
    }
    let mut config_opt = app.mcp_store.mcp_config.take();
    let mut save_clicked = false;
    let mut cancel_clicked = false;
    let mut create_clicked = false;
    let mut open = app.mcp_store.mcp_panel_open;

    egui::Window::new("MCP Servers")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_size([400.0, 500.0])
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(app.ui_store.theme.surface)
                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_lg as u8)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(360.0);
            if let Some(ref mut config) = config_opt {
                let mut changed = false;
                crate::ui::mcp_panel::render_mcp_panel(ui, config, &app.ui_store.theme, &mut changed);
                if changed {
                    app.mcp_store.mcp_changed = true;
                }
                if app.mcp_store.mcp_changed {
                    ui.add_space(app.ui_store.theme.space_12);
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("Save")
                                            .size(app.ui_store.theme.text_base)
                                            .color(app.ui_store.theme.text),
                                    )
                                    .fill(app.ui_store.theme.accent)
                                    .min_size(egui::vec2(80.0, 32.0)),
                                )
                                .clicked()
                            {
                                save_clicked = true;
                            }
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("Cancel")
                                            .size(app.ui_store.theme.text_base)
                                            .color(app.ui_store.theme.text),
                                    )
                                    .fill(app.ui_store.theme.border)
                                    .min_size(egui::vec2(80.0, 32.0)),
                                )
                                .clicked()
                            {
                                cancel_clicked = true;
                            }
                        });
                    });
                }
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(app.ui_store.theme.space_40);
                    ui.label(
                        egui::RichText::new("No MCP config found")
                            .size(app.ui_store.theme.text_base)
                            .color(app.ui_store.theme.text_dim),
                    );
                    ui.add_space(app.ui_store.theme.space_8);
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Create Config")
                                    .size(app.ui_store.theme.text_base)
                                    .color(app.ui_store.theme.text),
                            )
                            .fill(app.ui_store.theme.accent)
                            .min_size(egui::vec2(140.0, 36.0)),
                        )
                        .clicked()
                    {
                        create_clicked = true;
                    }
                });
            }
        });

    app.mcp_store.mcp_panel_open = open;

    if save_clicked {
        if let Some(ref mut config) = config_opt {
            match crate::ui::mcp_panel::save_mcp_config(config) {
                Ok(()) => {
                    app.push_toast("MCP config saved", crate::ui::types::ToastLevel::Info);
                    app.mcp_store.mcp_changed = false;
                }
                Err(e) => app.push_toast(
                    format!("Save failed: {}", e),
                    crate::ui::types::ToastLevel::Error,
                ),
            }
        }
    }
    if cancel_clicked {
        config_opt = crate::ui::mcp_panel::load_mcp_config();
        app.mcp_store.mcp_changed = false;
        app.mcp_store.mcp_panel_open = false;
    }
    if create_clicked {
        config_opt = Some(clarity_core::mcp::config::McpConfig::default());
        app.mcp_store.mcp_changed = true;
    }

    app.mcp_store.mcp_config = config_opt;
}
