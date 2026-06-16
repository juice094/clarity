//! Right rail — Context card.
//!
//! S6 Phase C: this is the bottom stacked card in the drawer-style right rail.
//! It surfaces tools, memory, teams, and context-specific resources (plain
//! session, claw remote session, or project workspace).

use crate::App;
use crate::design_system::{self, Space, Text};
use clarity_core::ui::RightRailContext;

/// Render the context card for the active drawer context.
pub fn render(app: &mut App, ui: &mut egui::Ui, ctx: RightRailContext) {
    match ctx {
        RightRailContext::Session => render_session_context(app, ui),
        RightRailContext::Claw => render_claw_context_placeholder(ui),
        RightRailContext::Project => render_project_context_placeholder(ui),
    }
}

fn render_session_context(app: &mut App, ui: &mut egui::Ui) {
    crate::panels::right_rail::tools_card::render(app, ui);
    design_system::gap(ui, Space::S3);
    crate::panels::right_rail::memory_card::render(app, ui);
}

fn render_claw_context_placeholder(ui: &mut egui::Ui) {
    design_system::center(ui, |ui| {
        design_system::gap(ui, Space::S6);
        design_system::text(ui, "Claw 远程上下文", Text::BodyMuted);
        design_system::gap(ui, Space::S1);
        design_system::text(ui, "远程文件与环境面板将在后续版本填充。", Text::Caption);
    });
}

fn render_project_context_placeholder(ui: &mut egui::Ui) {
    design_system::center(ui, |ui| {
        design_system::gap(ui, Space::S6);
        design_system::text(ui, "项目资源上下文", Text::BodyMuted);
        design_system::gap(ui, Space::S1);
        design_system::text(ui, "工作区文件与计划面板将在后续版本填充。", Text::Caption);
    });
}
