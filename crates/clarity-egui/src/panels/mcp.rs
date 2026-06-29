use crate::App;

/// Renders the MCP (Model Context Protocol) overlay panel — lists configured
/// MCP servers, their transport methods, available tools, and connection status.
///
/// The config is borrowed mutably from `app.mcp_store` via `if let` so there is
/// no `take()`/restore window where a panic could permanently lose the config.
pub fn render_mcp_panel(app: &mut App, ctx: &egui::Context) {
    if !matches!(app.view_state.modal, Some(clarity_core::ui::ModalType::Mcp)) {
        return;
    }

    // Ensure config is loaded at least once (first open of the panel).
    if app.mcp_store.mcp_config.is_none() {
        app.mcp_store.mcp_config = crate::ui::mcp_panel::load_mcp_config();
    }

    let mut save_clicked = false;
    let mut cancel_clicked = false;
    let mut create_clicked = false;
    let mut open = true;
    let mut close_requested = false;
    let screen = ctx.screen_rect();
    let theme = app.ui_store.theme.clone();

    // ── Dimmer + outside-click-to-close ──
    let scrim = theme.overlay;
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
            if ui
                .allocate_response(screen.size(), egui::Sense::click())
                .clicked()
                || ctx.input(|i| i.key_pressed(egui::Key::Escape))
            {
                close_requested = true;
            }
        });

    // ── Config editor window ──
    egui::Window::new(app.t("MCP Servers"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .default_size([400.0, 500.0])
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(theme.surface)
                .corner_radius(egui::CornerRadius::same(theme.radius_lg as u8)),
        )
        .show(ctx, |ui| {
            ui.set_min_width(360.0);

            // Borrow config mutably in-place — no take()/restore.
            if let Some(ref mut config) = app.mcp_store.mcp_config {
                let mut changed = false;
                crate::ui::mcp_panel::render_mcp_panel(ui, config, &theme, &mut changed);
                if changed {
                    app.mcp_store.mcp_changed = true;
                }
                if app.mcp_store.mcp_changed {
                    ui.add_space(theme.space_12);
                    ui.horizontal(|ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new("Save")
                                            .size(theme.text_base)
                                            .color(theme.text),
                                    )
                                    .fill(theme.accent)
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
                                            .size(theme.text_base)
                                            .color(theme.text),
                                    )
                                    .fill(theme.border)
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
                    ui.add_space(theme.space_40);
                    ui.label(
                        egui::RichText::new("No MCP config found")
                            .size(theme.text_base)
                            .color(theme.text_dim),
                    );
                    ui.add_space(theme.space_8);
                    if ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Create Config")
                                    .size(theme.text_base)
                                    .color(theme.text),
                            )
                            .fill(theme.accent)
                            .min_size(egui::vec2(140.0, 36.0)),
                        )
                        .clicked()
                    {
                        create_clicked = true;
                    }
                });
            }
        });

    // ── Action handlers (after borrow is released) ──
    if close_requested || !open {
        app.view_state.close_modal();
    }

    if save_clicked {
        // Clone before mutable access so the immutable borrow is released.
        let save_result = app.mcp_store.mcp_config.as_ref().map(|config| {
            let saved = crate::ui::mcp_panel::save_mcp_config(config);
            (saved, config.clone())
        });

        if let Some((result, config_clone)) = save_result {
            match result {
                Ok(()) => {
                    app.push_toast(
                        app.t("MCP config saved").to_string(),
                        crate::ui::types::ToastLevel::Info,
                    );
                    app.mcp_store.mcp_changed = false;
                    app.hot_reload_mcp(config_clone);
                }
                Err(e) => app.push_toast(
                    format!("Save failed: {}", e),
                    crate::ui::types::ToastLevel::Error,
                ),
            }
        }
    }
    if cancel_clicked {
        app.mcp_store.mcp_config = crate::ui::mcp_panel::load_mcp_config();
        app.mcp_store.mcp_changed = false;
        app.view_state.close_modal();
    }
    if create_clicked {
        app.mcp_store.mcp_config = Some(clarity_core::mcp::config::McpConfig::default());
        app.mcp_store.mcp_changed = true;
    }
}
