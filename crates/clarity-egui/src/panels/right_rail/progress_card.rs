//! Right rail — Progress card.
//!
//! S6 Phase C: this is the top stacked card in the drawer-style right rail.
//! It surfaces running state, background tasks, and context-specific progress
//! (plain session, claw remote session, or project workspace).

use crate::App;
use crate::design_system::{self, Space, Text};
use clarity_core::ui::RightRailContext;

/// Render the progress card for the active drawer context.
pub fn render(app: &mut App, ui: &mut egui::Ui, ctx: RightRailContext) {
    match ctx {
        RightRailContext::Session => render_session_progress(app, ui),
        RightRailContext::Claw => render_claw_progress_placeholder(ui),
        RightRailContext::Project => render_project_progress_placeholder(ui),
    }
}

fn render_session_progress(app: &mut App, ui: &mut egui::Ui) {
    crate::panels::right_rail::status_card::render(app, ui);
    design_system::gap(ui, Space::S3);
    crate::panels::right_rail::subagent_card::render(app, ui);
}

fn render_claw_progress_placeholder(ui: &mut egui::Ui) {
    design_system::center(ui, |ui| {
        design_system::gap(ui, Space::S6);
        design_system::text(ui, "Claw 任务进度", Text::BodyMuted);
        design_system::gap(ui, Space::S1);
        design_system::text(ui, "远程任务面板将在后续版本填充。", Text::Caption);
    });
}

fn render_project_progress_placeholder(ui: &mut egui::Ui) {
    design_system::center(ui, |ui| {
        design_system::gap(ui, Space::S6);
        design_system::text(ui, "项目任务进度", Text::BodyMuted);
        design_system::gap(ui, Space::S1);
        design_system::text(ui, "项目任务面板将在后续版本填充。", Text::Caption);
    });
}
