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
    let mut close_requested = false;
    let screen = ctx.screen_rect();

    // Dimmer + outside-click-to-close
    let scrim = egui::Color32::from_rgba_premultiplied(0, 0, 0, 180);
    ctx.layer_painter(egui::LayerId::background()).rect_filled(
        screen,
        egui::CornerRadius::same(0),
        scrim,
    );
    egui::Area::new("mcp_scrim".into())
        .interactable(true)
        .order(egui::Order::Background)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());
            if ui.allocate_response(screen.size(), egui::Sense::click()).clicked()
                || ctx.input(|i| i.key_pressed(egui::Key::Escape))
            {
                close_requested = true;
            }
        });

    egui::Window::new(app.t("MCP Servers"))
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

    app.mcp_store.mcp_panel_open = open && !close_requested;

    if save_clicked {
        if let Some(ref mut config) = config_opt {
            match crate::ui::mcp_panel::save_mcp_config(config) {
                Ok(()) => {
                    app.push_toast("MCP config saved", crate::ui::types::ToastLevel::Info);
                    app.mcp_store.mcp_changed = false;

                    // Hot-reload MCP tools after config save
                    let old_tools = app.mcp_store.connected_tools.clone();
                    let agent = app.state.agent.clone();
                    let tx = app.ui_tx.clone();
                    let config_clone = config.clone();
                    app.runtime.spawn(async move {
                        // 1. Unregister old MCP tools
                        for name in &old_tools {
                            let _ = agent.registry().unregister(name);
                        }

                        // 2. Re-connect and register new tools
                        let manager = clarity_core::mcp::McpManager::from_config(&config_clone).await;
                        let tool_names: Vec<String> = manager
                            .tools()
                            .iter()
                            .map(|t: &clarity_core::mcp::McpToolAdapter| t.name().to_string())
                            .collect();
                        manager.register_all(agent.registry());
                        let _ = tx.send(crate::ui::types::UiEvent::McpReloaded {
                            success: true,
                            tools: tool_names,
                            message: format!(
                                "MCP reloaded: {} server(s), {} tool(s)",
                                manager.list_servers().len(),
                                manager.tools().len()
                            ),
                        });
                    });
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
