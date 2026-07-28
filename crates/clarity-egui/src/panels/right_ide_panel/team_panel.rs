//! Team panel — configured agent teams for the right IDE rail.
//!
//! ponytail: teams are configuration records owned by `TeamStore`; there is no
//! live "executing team" concept in the store yet, so this panel is a
//! configuration list (view + create). Live execution visualisation waits for
//! wire-level team events — see the Subagents wiring TODO in `mod.rs`.

use crate::App;
use crate::design_system::{self, BadgeVariant, Space, TextStyle};
use crate::stores::Team;
use clarity_ui::widgets::button::Button;

/// Render the team panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.context.ui_store.theme.clone();

    // ── Toolbar ──
    ui.horizontal(|ui| {
        design_system::text(ui, app.t("Teams"), TextStyle::CaptionStrong);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(Button::new(app.t("New Team")).primary().small())
                .clicked()
            {
                app.open_modal(clarity_core::ui::ModalType::TeamCreate);
            }
        });
    });
    design_system::gap(ui, Space::S1);

    let teams = app.team_store().teams.clone();
    if teams.is_empty() {
        render_empty_state(app, ui, &theme);
        return;
    }

    egui::ScrollArea::vertical()
        .id_salt("team_panel")
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            for team in &teams {
                render_team_card(&*app, ui, team, &theme);
                design_system::gap(ui, Space::S1);
            }
        });
}

fn render_empty_state(app: &App, ui: &mut egui::Ui, theme: &crate::theme::Theme) {
    ui.vertical_centered(|ui| {
        ui.add_space(theme.space_24);
        clarity_ui::design_system::icon_with_color(
            ui,
            crate::theme::ICON_BOT,
            theme.text_2xl,
            theme.text_dim,
        );
        design_system::gap(ui, Space::S1);
        design_system::text(ui, app.t("No teams yet"), TextStyle::Subheading);
        design_system::gap(ui, Space::S1);
        design_system::text(
            ui,
            app.t("Create a team to coordinate multiple agents"),
            TextStyle::Small,
        );
    });
}

fn render_team_card(app: &App, ui: &mut egui::Ui, team: &Team, theme: &crate::theme::Theme) {
    design_system::card(ui, |ui| {
        ui.set_min_width(ui.available_width());

        // Row 1: team icon + name + member count badge
        ui.horizontal(|ui| {
            clarity_ui::design_system::icon_with_color(
                ui,
                crate::theme::ICON_BOT,
                theme.text_sm,
                theme.accent,
            );
            design_system::gap(ui, Space::S0);
            design_system::text(ui, &team.name, TextStyle::Body);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                design_system::badge(
                    ui,
                    format!("{}: {}", app.t("Members"), team.members.len()),
                    BadgeVariant::Neutral,
                );
            });
        });

        // Goal
        if !team.goal.is_empty() {
            design_system::gap(ui, Space::S0);
            design_system::text(ui, &team.goal, TextStyle::Small);
        }

        // Execution limits
        design_system::gap(ui, Space::S0);
        design_system::text(
            ui,
            format!(
                "{}: {} · {}: {}s",
                app.t("Max Concurrency"),
                team.max_concurrency,
                app.t("Timeout"),
                team.timeout_secs,
            ),
            TextStyle::Small,
        );

        // Members
        if !team.members.is_empty() {
            design_system::gap(ui, Space::S1);
            for member in &team.members {
                let line = if member.description.is_empty() {
                    format!("• {} ({})", member.name, member.agent_type)
                } else {
                    format!(
                        "• {} ({}) — {}",
                        member.name, member.agent_type, member.description
                    )
                };
                design_system::text(ui, line, TextStyle::Small);
            }
        }
    });
}

// ── Panel trait implementation ──

/// Team panel renderer.
pub struct TeamPanel;

impl crate::design_system::Panel for TeamPanel {
    fn title(&self, app: &crate::App) -> &str {
        app.t("Team")
    }
    fn render(&mut self, app: &mut crate::App, ui: &mut egui::Ui) {
        render(app, ui);
    }
}
