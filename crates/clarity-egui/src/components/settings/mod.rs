use crate::App;

pub mod about_tab;
pub mod interface_tab;
pub mod provider_tab;

pub fn render_settings_panel(app: &mut App, ctx: &egui::Context) {
    if !app.settings_store.settings_open {
        return;
    }

    let screen = ctx.screen_rect();

    // ── Dimmer + outside-click-to-close ──
    let scrim = egui::Color32::from_rgba_premultiplied(0, 0, 0, 180);
    ctx.layer_painter(egui::LayerId::background()).rect_filled(
        screen,
        egui::CornerRadius::same(0),
        scrim,
    );

    // Click outside the settings window → close
    let mut close_requested = false;
    egui::Area::new("settings_scrim".into())
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

    egui::Window::new("")
        .collapsible(false)
        .resizable(false)
        .default_size(egui::vec2(640.0, 560.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(app.ui_store.theme.bg)
                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_lg as u8))
                .inner_margin(egui::Margin::same(0)),
        )
        .show(ctx, |ui| {
            egui::ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    egui::Frame::new()
                        .fill(egui::Color32::TRANSPARENT)
                        .inner_margin(egui::Margin::symmetric(16, 12))
                        .show(ui, |ui| {
                            // ── Provider ──
                            provider_tab::render_provider(app, ui);

                            // Divider
                            ui.add_space(app.ui_store.theme.space_16);
                            ui.separator();
                            ui.add_space(app.ui_store.theme.space_16);

                            // ── Interface ──
                            interface_tab::render_interface(app, ui);

                            // Divider
                            ui.add_space(app.ui_store.theme.space_16);
                            ui.separator();
                            ui.add_space(app.ui_store.theme.space_16);

                            // ── About ──
                            about_tab::render_about(app, ui);
                        });
                });
        });

    if close_requested {
        app.settings_store.settings_open = false;
    }
}
