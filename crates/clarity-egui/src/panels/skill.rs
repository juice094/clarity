use crate::App;

pub fn render_skill_panel(app: &mut App, ctx: &egui::Context) {
    if !app.ui_store.skill_panel_open {
        return;
    }

    let skills = app.state.agent.list_skills();
    let active_ids = app.state.agent.skill_active_ids();

    let mut open = app.ui_store.skill_panel_open;
    let mut close_requested = false;
    let screen = ctx.screen_rect();

    // Dimmer + outside-click-to-close
    let scrim = egui::Color32::from_rgba_premultiplied(0, 0, 0, 180);
    ctx.layer_painter(egui::LayerId::background()).rect_filled(
        screen,
        egui::CornerRadius::same(0),
        scrim,
    );
    egui::Area::new("skill_scrim".into())
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

    egui::Window::new(app.t("Skills"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::vec2(400.0, 480.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(
            egui::Frame::window(&ctx.style())
                .fill(app.ui_store.theme.surface)
                .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_lg as u8))
                .inner_margin(egui::Margin::same(16)),
        )
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(
                    egui::RichText::new("Skills")
                        .size(app.ui_store.theme.text_lg)
                        .strong()
                        .color(app.ui_store.theme.text),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .button(
                            egui::RichText::new(crate::theme::ICON_X)
                                .font(app.ui_store.theme.font_icon(app.ui_store.theme.text_sm))
                                .color(app.ui_store.theme.text_dim),
                        )
                        .clicked()
                    {
                        app.ui_store.skill_panel_open = false;
                    }
                    if ui
                        .button(
                            egui::RichText::new("🔄")
                                .size(app.ui_store.theme.text_sm)
                                .color(app.ui_store.theme.text_dim),
                        )
                        .clicked()
                    {
                        let _ = app.state.agent.discover_skills();
                    }
                });
            });
            ui.add_space(app.ui_store.theme.space_8);

            if skills.is_empty() {
                ui.label(
                    egui::RichText::new(
                        "No skills found.\nPlace .md files in .clarity/skills/ to add skills.",
                    )
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text_dim),
                );
            } else {
                ui.label(
                    egui::RichText::new(format!(
                        "{} skill(s) loaded, {} active",
                        skills.len(),
                        active_ids.len()
                    ))
                    .size(app.ui_store.theme.text_sm)
                    .color(app.ui_store.theme.text_dim),
                );
                ui.add_space(app.ui_store.theme.space_8);

                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for skill in &skills {
                            let is_active = active_ids.contains(&skill.meta.id);
                            let id = skill.meta.id.clone();

                            egui::Frame::new()
                                .fill(app.ui_store.theme.bg_accent)
                                .corner_radius(egui::CornerRadius::same(
                                    app.ui_store.theme.radius_sm as u8,
                                ))
                                .inner_margin(egui::Margin::same(10))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        ui.label(
                                            egui::RichText::new(&skill.meta.name)
                                                .size(app.ui_store.theme.text_base)
                                                .strong()
                                                .color(app.ui_store.theme.text),
                                        );
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                let toggle_text =
                                                    if is_active { "ON" } else { "OFF" };
                                                let toggle_color = if is_active {
                                                    app.ui_store.theme.ok
                                                } else {
                                                    app.ui_store.theme.text_dim
                                                };
                                                if ui
                                                    .button(
                                                        egui::RichText::new(toggle_text)
                                                            .size(app.ui_store.theme.text_sm)
                                                            .color(toggle_color),
                                                    )
                                                    .clicked()
                                                {
                                                    app.state
                                                        .agent
                                                        .set_skill_active(&id, !is_active);
                                                }
                                            },
                                        );
                                    });
                                    if !skill.meta.description.is_empty() {
                                        ui.label(
                                            egui::RichText::new(&skill.meta.description)
                                                .size(app.ui_store.theme.text_sm)
                                                .color(app.ui_store.theme.text_dim),
                                        );
                                    }
                                    if !skill.meta.tools.is_empty() {
                                        ui.horizontal_wrapped(|ui| {
                                            ui.label(
                                                egui::RichText::new("Tools:")
                                                    .size(app.ui_store.theme.text_xs)
                                                    .color(app.ui_store.theme.text_dim),
                                            );
                                            for tool in &skill.meta.tools {
                                                ui.label(
                                                    egui::RichText::new(format!("• {}", tool))
                                                        .size(app.ui_store.theme.text_xs)
                                                        .color(app.ui_store.theme.accent),
                                                );
                                            }
                                        });
                                    }
                                    if !skill.meta.tags.is_empty() {
                                        ui.horizontal_wrapped(|ui| {
                                            for tag in &skill.meta.tags {
                                                ui.label(
                                                    egui::RichText::new(format!("#{}", tag))
                                                        .size(app.ui_store.theme.text_xs)
                                                        .color(app.ui_store.theme.text_dim),
                                                );
                                            }
                                        });
                                    }
                                });
                            ui.add_space(app.ui_store.theme.space_8);
                        }
                    });
            }
        });

    app.ui_store.skill_panel_open = open && !close_requested;
}
