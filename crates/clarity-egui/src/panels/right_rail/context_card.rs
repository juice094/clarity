//! Right rail — Context card.
//!
//! S6 Phase C: this is the bottom stacked card in the drawer-style right rail.
//! It surfaces tools, memory, teams, and context-specific resources (plain
//! session, claw remote session, or project workspace).

use crate::App;
use clarity_core::ui::RightRailContext;

/// Render the context card for the active drawer context.
pub fn render(app: &mut App, ui: &mut egui::Ui, ctx: RightRailContext) {
    match ctx {
        RightRailContext::Session => render_session_context(app, ui),
        RightRailContext::Claw => render_claw_context_placeholder(app, ui),
        RightRailContext::Project => render_project_context_placeholder(app, ui),
    }
}

fn render_session_context(app: &mut App, ui: &mut egui::Ui) {
    crate::panels::right_rail::tools_card::render(app, ui);
    ui.add_space(app.ui_store.theme.space_16);
    crate::panels::right_rail::memory_card::render(app, ui);
}

fn render_claw_context_placeholder(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    ui.vertical_centered(|ui| {
        ui.add_space(theme.space_40);
        ui.label(
            egui::RichText::new("Claw 远程上下文")
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        ui.add_space(theme.space_8);
        ui.label(
            egui::RichText::new("远程文件与环境面板将在后续版本填充。")
                .size(theme.text_xs)
                .color(theme.text_muted),
        );
    });
}

fn render_project_context_placeholder(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    ui.vertical_centered(|ui| {
        ui.add_space(theme.space_40);
        ui.label(
            egui::RichText::new("项目资源上下文")
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        ui.add_space(theme.space_8);
        ui.label(
            egui::RichText::new("工作区文件与计划面板将在后续版本填充。")
                .size(theme.text_xs)
                .color(theme.text_muted),
        );
    });
}
