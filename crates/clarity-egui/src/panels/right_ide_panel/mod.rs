//! IDE-style right rail panel.
//!
//! S6 Phase D: the right rail is now a single compressed IDE panel that shows
//! one functional panel at a time (Share, Console, Files, Claw settings,
//! Knowledge base, Templates). The old stacked-card drawer has been moved to
//! `panels::right_rail` as a content source during migration.

use crate::App;
use clarity_core::ui::RightRailPanel;

pub mod claw_settings_panel;
pub mod claw_terminal_panel;
pub mod claw_webbridge_panel;
pub mod claw_workspace_panel;
pub mod console_panel;
pub mod files_panel;
pub mod knowledge_panel;
pub mod share_panel;
pub mod template_panel;

/// Render the IDE-style right rail panel.
pub fn render_right_ide_panel(app: &mut App, ctx: &egui::Context) {
    let theme = app.ui_store.theme.clone();
    let inset = theme.space_4 as i8;
    let inner_margin = egui::Margin::symmetric(12, 16);
    let outer_margin = egui::Margin {
        left: 0,
        right: inset,
        top: inset,
        bottom: inset,
    };

    let response = egui::SidePanel::right("right_ide_panel")
        .default_width(theme.size_panel_right.ceil())
        .min_width(180.0)
        .max_width(400.0)
        .resizable(true)
        .show_separator_line(false)
        .frame(
            egui::Frame::side_top_panel(&ctx.style())
                // The background is painted by the unified main-stage painter so
                // the rail shares the same surface as the chat stage. Keep the
                // panel frame transparent here.
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE)
                .shadow(egui::Shadow::NONE)
                .inner_margin(inner_margin)
                .outer_margin(outer_margin),
        )
        .show(ctx, |ui| {
            if crate::ui::debug_overlay::is_enabled(ctx) {
                crate::ui::debug_overlay::show_layout_state(ui, "right-ide-panel");
            }
            ui.set_min_width(ui.available_width());

            let panel = app.view_state.right_rail_panel;

            // Header: title + close button.
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(panel_title(panel, app))
                        .size(theme.text_base)
                        .strong()
                        .color(theme.text_strong),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if crate::widgets::icon_button_toolbar(
                        ui,
                        crate::theme::ICON_X,
                        theme.text_base,
                        &theme,
                    )
                    .on_hover_text(app.t("Collapse right rail"))
                    .clicked()
                    {
                        app.view_state.collapse_right_rail();
                    }
                });
            });
            ui.add_space(theme.space_12);

            // Panel content.
            match panel {
                RightRailPanel::Share => share_panel::render(app, ui),
                RightRailPanel::Console => console_panel::render(app, ui),
                RightRailPanel::Files => files_panel::render(app, ui),
                RightRailPanel::ClawSettings => claw_settings_panel::render(app, ui),
                RightRailPanel::ClawWorkspace => claw_workspace_panel::render(app, ui),
                RightRailPanel::ClawTerminal => claw_terminal_panel::render(app, ui),
                RightRailPanel::ClawWebBridge => claw_webbridge_panel::render(app, ui),
                RightRailPanel::KnowledgeBase => knowledge_panel::render(app, ui),
                RightRailPanel::Templates => template_panel::render(app, ui),
                RightRailPanel::None => {
                    ui.label(
                        egui::RichText::new(app.t("Select a panel from the Bot bar"))
                            .size(theme.text_sm)
                            .color(theme.text_dim),
                    );
                }
            }
        });

    app.ui_store.right_rail_width = Some(response.response.rect.width());

    // The native resize hover/drag line is drawn by egui on top of the panel
    // contents and cannot be disabled with `show_separator_line(false)`. Cover
    // the line with the background color and redraw the divider ourselves so it
    // aligns cleanly with the rounded main-stage surface.
    let panel_rect = response.response.rect;
    let screen = ctx.screen_rect();
    let surface_top = theme.size_titlebar + theme.space_4;
    let surface_bottom = screen.max.y - theme.space_4;
    // The native resize hover/drag line can extend all the way from the
    // titlebar to the window bottom, even though the panel content is inset by
    // `space_4`. Cover the full vertical range so nothing leaks outside the
    // rounded main-stage surface.
    let cover = egui::Rect::from_min_max(
        egui::pos2(panel_rect.left() - 2.0, theme.size_titlebar),
        egui::pos2(panel_rect.left() + 2.0, screen.max.y),
    );
    // Paint on the foreground layer so the cover and divider sit on top of the
    // native resize hover line that egui draws even with
    // `show_separator_line(false)`.
    let painter = ctx.layer_painter(egui::LayerId::new(
        egui::Order::Foreground,
        egui::Id::new("right_rail_divider"),
    ));
    painter.rect_filled(cover, egui::CornerRadius::ZERO, theme.bg);
    // Draw the divider all the way from the top of the rounded main-stage
    // surface to the bottom so it visually connects with the outer border.
    painter.line_segment(
        [
            egui::pos2(panel_rect.left(), surface_top),
            egui::pos2(panel_rect.left(), surface_bottom),
        ],
        egui::Stroke::new(1.0, theme.border),
    );
}

fn panel_title(panel: RightRailPanel, app: &crate::App) -> &'static str {
    match panel {
        RightRailPanel::None => app.t("Panel"),
        RightRailPanel::Share => app.t("Share"),
        RightRailPanel::Console => app.t("Console"),
        RightRailPanel::Files => app.t("Files"),
        RightRailPanel::ClawSettings => app.t("Claw"),
        RightRailPanel::ClawWorkspace => app.t("Workspace"),
        RightRailPanel::ClawTerminal => app.t("Terminal"),
        RightRailPanel::ClawWebBridge => app.t("WebBridge"),
        RightRailPanel::KnowledgeBase => app.t("Knowledge"),
        RightRailPanel::Templates => app.t("Templates"),
    }
}
