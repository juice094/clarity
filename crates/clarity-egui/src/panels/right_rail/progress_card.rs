//! Right rail — Progress card.
//!
//! S6 Phase C: this is the top stacked card in the drawer-style right rail.
//! It surfaces running state, background tasks, and context-specific progress
//! (plain session, claw remote session, or project workspace).

use crate::App;
use clarity_core::ui::RightRailContext;

/// Render the progress card for the active drawer context.
pub fn render(app: &mut App, ui: &mut egui::Ui, ctx: RightRailContext) {
    match ctx {
        RightRailContext::Session => render_session_progress(app, ui),
        RightRailContext::Claw => render_claw_progress_placeholder(app, ui),
        RightRailContext::Project => render_project_progress_placeholder(app, ui),
    }
}

fn render_session_progress(app: &mut App, ui: &mut egui::Ui) {
    crate::panels::right_rail::status_card::render(app, ui);
    ui.add_space(app.ui_store.theme.space_16);
    crate::panels::right_rail::subagent_card::render(app, ui);
}

fn render_claw_progress_placeholder(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    ui.vertical_centered(|ui| {
        ui.add_space(theme.space_40);
        ui.label(
            egui::RichText::new("Claw 任务进度")
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        ui.add_space(theme.space_8);
        ui.label(
            egui::RichText::new("远程任务面板将在后续版本填充。")
                .size(theme.text_xs)
                .color(theme.text_muted),
        );
    });
}

fn render_project_progress_placeholder(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    ui.vertical_centered(|ui| {
        ui.add_space(theme.space_40);
        ui.label(
            egui::RichText::new("项目任务进度")
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        ui.add_space(theme.space_8);
        ui.label(
            egui::RichText::new("项目任务面板将在后续版本填充。")
                .size(theme.text_xs)
                .color(theme.text_muted),
        );
    });
}
