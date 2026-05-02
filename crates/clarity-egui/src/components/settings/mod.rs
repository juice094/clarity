use crate::App;

pub mod about_tab;
pub mod interface_tab;
pub mod provider_tab;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingsTab {
    Provider,
    Interface,
    About,
}

pub fn render_settings_panel(app: &mut App, ctx: &egui::Context) {
    if !app.settings_store.settings_open {
        return;
    }

    let screen = ctx.screen_rect();

    // ── Dimmer + outside-click-to-close ──
    ctx.layer_painter(egui::LayerId::background()).rect_filled(
        screen,
        egui::CornerRadius::same(0),
        app.ui_store.theme.overlay,
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

    let tabs = [
        (SettingsTab::Provider, app.t("Provider")),
        (SettingsTab::Interface, app.t("Interface")),
        (SettingsTab::About, app.t("About")),
    ];
    let mut at = app.settings_store.settings_active_tab;

    egui::Window::new(app.t("Settings"))
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::vec2(560.0, 460.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(app.ui_store.theme.surface)
                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_lg as u8))
                .inner_margin(egui::Margin::same(0)),
        )
        .show(ctx, |ui| {
            // ── Tab bar ──
            egui::Frame::new()
                .fill(app.ui_store.theme.bg_accent)
                .inner_margin(egui::Margin::symmetric(8, 0))
                .corner_radius(egui::CornerRadius {
                    nw: app.ui_store.theme.radius_lg as u8,
                    ne: app.ui_store.theme.radius_lg as u8,
                    sw: 0,
                    se: 0,
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.set_min_height(34.0);
                        for (i, (_t, name)) in tabs.iter().enumerate() {
                            let is = i as u8 == at;
                            let bg = if is {
                                app.ui_store.theme.surface
                            } else {
                                egui::Color32::TRANSPARENT
                            };
                            let tc = if is {
                                app.ui_store.theme.text
                            } else {
                                app.ui_store.theme.text_muted
                            };
                            if ui
                                .add(
                                    egui::Button::new(
                                        egui::RichText::new(*name)
                                            .size(app.ui_store.theme.text_base)
                                            .color(tc),
                                    )
                                    .fill(bg)
                                    .corner_radius(app.ui_store.theme.radius_sm as u8)
                                    .min_size(egui::vec2(90.0, 28.0)),
                                )
                                .clicked()
                            {
                                at = i as u8;
                            }
                        }
                    });
                });

            // ── Content ──
            egui::Frame::new()
                .fill(egui::Color32::TRANSPARENT)
                .inner_margin(egui::Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    ui.set_min_height(350.0);
                    match tabs[at as usize].0 {
                        SettingsTab::Provider => provider_tab::render_provider(app, ui),
                        SettingsTab::Interface => interface_tab::render_interface(app, ui),
                        SettingsTab::About => about_tab::render_about(app, ui),
                    }
                });
        });

    app.settings_store.settings_active_tab = at;
    if close_requested {
        app.settings_store.settings_open = false;
    }
}
