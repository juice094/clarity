use crate::App;
use clarity_ui::design_system::{Space, TextStyle, card, gap, text, toggle};
use clarity_ui::widgets::overlay::{Overlay, overlay_scrim};

/// Renders the skill panel UI.
pub fn render_skill_panel(app: &mut App, ctx: &egui::Context) {
    if app.current_modal() != Some(&clarity_core::ui::ModalType::Skill) {
        return;
    }

    let skills = app.context.state.agent.list_skills();
    let active_ids = app.context.state.agent.skill_active_ids();

    let mut close_requested = false;
    let theme = app.context.ui_store.theme.clone();

    // Dimmer + outside-click-to-close.
    let scrim_response = overlay_scrim(ctx);

    Overlay::new("skill")
        .width(420.0)
        .max_height(600.0)
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                text(ui, "Skills", TextStyle::Title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // ponytail: icon-button component does not yet expose
                    // per-icon colour; keep raw Button with icon font.
                    if ui
                        .button(
                            egui::RichText::new(crate::theme::ICON_X)
                                .font(theme.font_icon(theme.text_sm))
                                .color(theme.text_dim),
                        )
                        .clicked()
                    {
                        close_requested = true;
                    }
                    // ponytail: icon-button component does not yet expose
                    // per-icon colour; keep raw Button with icon font.
                    if ui
                        .button(
                            egui::RichText::new(crate::theme::ICON_REFRESH)
                                .font(theme.font_icon(theme.text_sm))
                                .color(theme.text_dim),
                        )
                        .clicked()
                    {
                        let _ = app.context.state.agent.discover_skills();
                    }
                });
            });
            gap(ui, Space::S1);

            if skills.is_empty() {
                text(
                    ui,
                    "No skills found.\nPlace .md files in .clarity/skills/ to add skills.",
                    TextStyle::Small,
                );
            } else {
                text(
                    ui,
                    format!(
                        "{} skill(s) loaded, {} active",
                        skills.len(),
                        active_ids.len()
                    ),
                    TextStyle::Small,
                );
                gap(ui, Space::S1);

                // ponytail: ScrollArea is not yet wrapped in clarity-ui.
                egui::ScrollArea::vertical()
                    .max_height(320.0)
                    .show(ui, |ui| {
                        for skill in &skills {
                            let is_active = active_ids.contains(&skill.meta.id);
                            let id = skill.meta.id.clone();

                            card(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    text(ui, &skill.meta.name, TextStyle::Body);
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            let mut active = is_active;
                                            if toggle(ui, &mut active) {
                                                app.context
                                                    .state
                                                    .agent
                                                    .set_skill_active(&id, active);
                                            }
                                        },
                                    );
                                });
                                if !skill.meta.description.is_empty() {
                                    text(ui, &skill.meta.description, TextStyle::Small);
                                }
                                if !skill.meta.tools.is_empty() {
                                    // ponytail: wrapped tool chips with custom
                                    // accent colour are not yet a protocol
                                    // component.
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(
                                            egui::RichText::new("Tools:")
                                                .size(theme.text_xs)
                                                .color(theme.text_dim),
                                        );
                                        for tool in &skill.meta.tools {
                                            ui.label(
                                                egui::RichText::new(format!("• {}", tool))
                                                    .size(theme.text_xs)
                                                    .color(theme.accent),
                                            );
                                        }
                                    });
                                }
                                if !skill.meta.tags.is_empty() {
                                    // ponytail: wrapped tag chips are not yet
                                    // a protocol component.
                                    ui.horizontal_wrapped(|ui| {
                                        for tag in &skill.meta.tags {
                                            text(ui, format!("#{}", tag), TextStyle::Small);
                                        }
                                    });
                                }
                            });
                            gap(ui, Space::S1);
                        }
                    });
            }
        });

    if close_requested
        || scrim_response.clicked()
        || ctx.input(|i| i.key_pressed(egui::Key::Escape))
    {
        app.close_modal();
    }
}
