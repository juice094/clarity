//! IDE-style right rail panel.
//!
//! S6 Phase D: the right rail is now a single compressed IDE panel that shows
//! one functional panel at a time (Share, Console, Files, Claw settings,
//! Knowledge base, Templates). The old stacked-card drawer has been moved to
//! `panels::right_rail` as a content source during migration.

use crate::App;
use crate::design_system::Panel;
use crate::stores::FocusTarget;
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

/// Dispatch enum that materialises the currently active right-rail panel as a
/// `design_system::Panel` implementation. This keeps header, close, and
/// animation concerns in one place while letting each panel own its rendering.
enum ActivePanel {
    Share(share_panel::SharePanel),
    Console(console_panel::ConsolePanel),
    Files(files_panel::FilesPanel),
    ClawSettings(claw_settings_panel::ClawSettingsPanel),
    ClawWorkspace(claw_workspace_panel::ClawWorkspacePanel),
    ClawTerminal(claw_terminal_panel::ClawTerminalPanel),
    ClawWebBridge(claw_webbridge_panel::ClawWebBridgePanel),
    KnowledgeBase(knowledge_panel::KnowledgePanel),
    Templates(template_panel::TemplatesPanel),
    /// Panels that have not been migrated from legacy views yet. They render a
    /// friendly placeholder instead of a raw migration message.
    Placeholder {
        title_key: &'static str,
        hint_key: &'static str,
    },
}

impl ActivePanel {
    fn from_kind(kind: RightRailPanel) -> Self {
        match kind {
            RightRailPanel::Share => Self::Share(share_panel::SharePanel),
            RightRailPanel::Console => Self::Console(console_panel::ConsolePanel),
            RightRailPanel::Files => Self::Files(files_panel::FilesPanel),
            RightRailPanel::ClawSettings => {
                Self::ClawSettings(claw_settings_panel::ClawSettingsPanel)
            }
            RightRailPanel::ClawWorkspace => {
                Self::ClawWorkspace(claw_workspace_panel::ClawWorkspacePanel)
            }
            RightRailPanel::ClawTerminal => {
                Self::ClawTerminal(claw_terminal_panel::ClawTerminalPanel)
            }
            RightRailPanel::ClawWebBridge => {
                Self::ClawWebBridge(claw_webbridge_panel::ClawWebBridgePanel)
            }
            RightRailPanel::KnowledgeBase => Self::KnowledgeBase(knowledge_panel::KnowledgePanel),
            RightRailPanel::Templates => Self::Templates(template_panel::TemplatesPanel),
            RightRailPanel::Team => Self::Placeholder {
                title_key: "Team",
                hint_key: "Team collaboration coming soon",
            },
            RightRailPanel::Task => Self::Placeholder {
                title_key: "Task",
                hint_key: "Task details coming soon",
            },
            RightRailPanel::Dashboard => Self::Placeholder {
                title_key: "Dashboard",
                hint_key: "Dashboard coming soon",
            },
            RightRailPanel::None => Self::Placeholder {
                title_key: "Panel",
                hint_key: "Select a panel from the Bot bar",
            },
        }
    }
}

impl Panel for ActivePanel {
    fn title(&self, app: &crate::App) -> &str {
        match self {
            Self::Share(_) => app.t("Share"),
            Self::Console(_) => app.t("Console"),
            Self::Files(_) => app.t("Files"),
            Self::ClawSettings(_) => app.t("Claw"),
            Self::ClawWorkspace(_) => app.t("Workspace"),
            Self::ClawTerminal(_) => app.t("Terminal"),
            Self::ClawWebBridge(_) => app.t("WebBridge"),
            Self::KnowledgeBase(_) => app.t("Knowledge"),
            Self::Templates(_) => app.t("Templates"),
            Self::Placeholder { title_key, .. } => app.t(title_key),
        }
    }

    fn render(&mut self, app: &mut crate::App, ui: &mut egui::Ui) {
        match self {
            Self::Share(p) => p.render(app, ui),
            Self::Console(p) => p.render(app, ui),
            Self::Files(p) => p.render(app, ui),
            Self::ClawSettings(p) => p.render(app, ui),
            Self::ClawWorkspace(p) => p.render(app, ui),
            Self::ClawTerminal(p) => p.render(app, ui),
            Self::ClawWebBridge(p) => p.render(app, ui),
            Self::KnowledgeBase(p) => p.render(app, ui),
            Self::Templates(p) => p.render(app, ui),
            Self::Placeholder { hint_key, .. } => {
                render_empty_state(ui, app.t(hint_key), &app.ui_store.theme);
            }
        }
    }
}

/// Render the IDE-style right rail panel.
pub fn render_right_ide_panel(app: &mut App, ctx: &egui::Context) {
    let theme = app.ui_store.theme.clone();
    let inset = theme.space_4 as i8;
    let inner_margin = egui::Margin::symmetric(theme.space_12 as i8, theme.space_16 as i8);
    let outer_margin = egui::Margin {
        left: 0,
        right: inset,
        top: inset,
        bottom: inset,
    };

    // ── Animation state machine ──
    // `right_rail_visible` is now driven by the animation, not set
    // imperatively by `collapse_right_rail()`. This prevents the chat
    // area from jumping to full width while the close animation runs.
    let current_panel = Some(app.view_state.right_rail_panel);
    let prev = app.panel_animation.prev_panel;

    // Detect open: panel went from None to a real panel.
    if current_panel != prev
        && current_panel != Some(RightRailPanel::None)
        && prev == Some(RightRailPanel::None)
    {
        app.view_state.right_rail_visible = true;
        let user_w = app
            .ui_store
            .right_rail_width
            .unwrap_or(theme.size_panel_right);
        app.panel_animation.right_panel_width =
            crate::animation::FloatAnimation::start(0.0, user_w, theme.duration_normal);
    }
    // Detect close: panel went from a real panel to None.
    else if current_panel == Some(RightRailPanel::None)
        && prev.is_some()
        && prev != Some(RightRailPanel::None)
    {
        let current_w = app.panel_animation.right_panel_width.current();
        app.panel_animation.right_panel_width =
            crate::animation::FloatAnimation::start(current_w, 0.0, theme.duration_normal);
    }
    app.panel_animation.prev_panel = current_panel;

    // Compute effective width based on animation progress.
    let anim_done = app.panel_animation.right_panel_width.done;
    let is_closing = current_panel == Some(RightRailPanel::None);
    let anim_w = app.panel_animation.right_panel_width.current();

    // When closing animation completes, hide the panel and stop rendering.
    if anim_done && is_closing {
        app.view_state.right_rail_visible = false;
        return;
    }
    // When open animation completes, use the user's preferred width.
    let user_w = app
        .ui_store
        .right_rail_width
        .unwrap_or(theme.size_panel_right);
    let effective_w = if anim_done { user_w } else { anim_w.max(0.0) };

    let panel_response = egui::SidePanel::right("right_ide_panel")
        // LAYOUT-EXEMPT: panel width bounds chosen for IDE-style utility rail.
        .default_width(effective_w.ceil().max(180.0))
        .min_width(180.0)
        .max_width(400.0)
        .resizable(true)
        .show_separator_line(false)
        .frame(
            egui::Frame::side_top_panel(&ctx.style())
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

            let panel_kind = app.view_state.right_rail_panel;
            let mut panel = ActivePanel::from_kind(panel_kind);

            // Header: title + close button.
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(panel.title(app))
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
            crate::design_system::gap(ui, crate::design_system::Space::S2);

            // Surface the empty-session quick-start hints only inside the Console
            // panel; the central empty stage keeps just the Clarity title/subtitle.
            if panel_kind == clarity_core::ui::RightRailPanel::Console
                && crate::panels::chat::is_empty_state(app)
            {
                render_quick_start_hints(app, ui);
                crate::design_system::gap(ui, crate::design_system::Space::S3);
            }

            // Panel content.
            panel.render(app, ui);
        });

    app.ui_store.right_rail_width = Some(panel_response.response.rect.width());

    // The native resize hover/drag line is drawn by egui on top of the panel
    // contents and cannot be disabled with `show_separator_line(false)`. Cover
    // the line with the background color and redraw the divider ourselves so it
    // aligns cleanly with the rounded main-stage surface.
    let panel_rect = panel_response.response.rect;
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

fn render_quick_start_hints(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    ui.label(
        egui::RichText::new(app.t("Quick start"))
            .size(theme.text_sm)
            .strong()
            .color(theme.text),
    );
    crate::design_system::gap(ui, crate::design_system::Space::S1);

    let hints = [
        ("/coder", app.t("Code assistant")),
        ("/plan", app.t("Task planning")),
        ("/review", app.t("Code review")),
    ];
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = theme.space_8;
        for (cmd, desc) in hints {
            let chip = egui::Frame::new()
                .fill(theme.bg_hover)
                .stroke(egui::Stroke::new(1.0, theme.border))
                .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
                .inner_margin(egui::Margin::symmetric(
                    theme.space_12 as i8,
                    theme.space_8 as i8,
                ))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(cmd)
                                .size(theme.text_xs)
                                .monospace()
                                .color(theme.accent),
                        );
                        crate::design_system::gap(ui, crate::design_system::Space::S1);
                        ui.label(
                            egui::RichText::new(desc)
                                .size(theme.text_xs)
                                .color(theme.text_muted),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(
                                egui::RichText::new("→")
                                    .font(theme.font_icon(theme.text_xs))
                                    .color(theme.text_dim),
                            );
                        });
                    });
                });
            let chip_response = chip
                .response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(format!("{} {}", cmd, desc));
            if chip_response.clicked() {
                app.chat_store.input = format!("{} ", cmd);
                app.ui_store.focus_target = Some(FocusTarget::ChatInput);
            }
        }
    });
}

/// Shared empty-state widget used by placeholder right-rail panels.
fn render_empty_state(ui: &mut egui::Ui, message: &str, theme: &crate::theme::Theme) {
    egui::Frame::new()
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
        .inner_margin(egui::Margin::same(theme.space_16 as i8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(crate::theme::ICON_LAYERS)
                        .font(theme.font_icon(theme.text_2xl))
                        .color(theme.text_dim),
                );
                crate::design_system::gap(ui, crate::design_system::Space::S2);
                ui.label(
                    egui::RichText::new(message)
                        .size(theme.text_sm)
                        .color(theme.text_dim)
                        .italics(),
                );
            });
        });
}
