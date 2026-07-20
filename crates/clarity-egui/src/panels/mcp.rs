use crate::App;
use clarity_ui::design_system::{Space, gap};
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::overlay::{Overlay, overlay_scrim};

/// Renders the MCP (Model Context Protocol) overlay panel — lists configured
/// MCP servers, their transport methods, available tools, and connection status.
///
/// The config is borrowed mutably from `app.context.mcp_store` via `if let` so there is
/// no `take()`/restore window where a panic could permanently lose the config.
pub fn render_mcp_panel(app: &mut App, ctx: &egui::Context) {
    if app.current_modal() != Some(&clarity_core::ui::ModalType::Mcp) {
        return;
    }

    // Ensure config is loaded at least once (first open of the panel).
    if app.context.mcp_store.mcp_config.is_none() {
        app.context.mcp_store.mcp_config = crate::ui::mcp_panel::load_mcp_config();
    }

    let mut save_clicked = false;
    let mut cancel_clicked = false;
    let mut create_clicked = false;
    let mut close_requested = false;
    let theme = app.context.ui_store.theme.clone();

    // ── Dimmer + outside-click-to-close ──
    let scrim_resp = overlay_scrim(ctx);
    if scrim_resp.clicked() || ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        close_requested = true;
    }

    // ── Config editor overlay ──
    Overlay::new("mcp_panel").width(400.0).show(ctx, |ui| {
        ui.set_min_width(360.0);

        // Borrow config mutably in-place — no take()/restore.
        if let Some(ref mut config) = app.context.mcp_store.mcp_config {
            let mut changed = false;
            crate::ui::mcp_panel::render_mcp_panel(ui, config, &theme, &mut changed);
            if changed {
                app.context.mcp_store.mcp_changed = true;
            }
            if app.context.mcp_store.mcp_changed {
                gap(ui, Space::S2);
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.add(Button::new("Save").primary().width(80.0)).clicked() {
                            save_clicked = true;
                        }
                        if ui.add(Button::new("Cancel").ghost().width(80.0)).clicked() {
                            cancel_clicked = true;
                        }
                    });
                });
            }
        } else {
            ui.vertical_centered(|ui| {
                gap(ui, Space::S6);
                // ponytail: raw Label for text_base + text_dim empty-state copy.
                ui.label(
                    egui::RichText::new("No MCP config found")
                        .size(theme.text_base)
                        .color(theme.text_dim),
                );
                gap(ui, Space::S1);
                if ui
                    .add(Button::new("Create Config").primary().large().width(140.0))
                    .clicked()
                {
                    create_clicked = true;
                }
            });
        }
    });

    // ── Action handlers (after borrow is released) ──
    if close_requested {
        app.close_modal();
    }

    if save_clicked {
        // Clone before mutable access so the immutable borrow is released.
        let save_result = app.context.mcp_store.mcp_config.as_ref().map(|config| {
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
                    app.context.mcp_store.mcp_changed = false;
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
        app.context.mcp_store.mcp_config = crate::ui::mcp_panel::load_mcp_config();
        app.context.mcp_store.mcp_changed = false;
        app.close_modal();
    }
    if create_clicked {
        app.context.mcp_store.mcp_config = Some(clarity_core::mcp::config::McpConfig::default());
        app.context.mcp_store.mcp_changed = true;
    }
}
