//! IDE-style right rail panel.
//!
//! S6 Phase D: the right rail is now an `egui_dock::DockArea` hosting multiple
//! dockable tabs (Share, Console, Files, Claw settings, Knowledge base,
//! Templates). The left navigation tree and central chat area stay unchanged.

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

// ponytail: `subagents_panel.rs` is implemented but not wired. It needs a
// `RightRailPanel::Subagents` variant plus a UI trigger (Bot bar button or
// shortcut) before users can open it. Until then the module is intentionally
// excluded from `pub mod` to avoid dead-code warnings.
// TODO(P2 follow-up): wire Subagents into `RightRailPanel`, `RightRailTab`,
// `ActivePanel`, and the Bot bar/shortcut surface.

/// Dockable tab identifier for the right IDE rail.
///
/// Mirrors [`RightRailPanel`] so that every functional panel has a 1:1 tab
/// counterpart. The legacy/migrated variants (`Team`, `Task`, `Dashboard`,
/// `None`) render placeholder content until their full panels are implemented.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RightRailTab {
    /// Share / export panel.
    Share,
    /// Console / task log panel.
    Console,
    /// File explorer / workspace files panel.
    Files,
    /// Claw remote device settings panel.
    ClawSettings,
    /// Claw workspace file tree panel.
    ClawWorkspace,
    /// Claw SSH terminal panel.
    ClawTerminal,
    /// Claw embedded web / code viewer panel.
    ClawWebBridge,
    /// Project knowledge base panel.
    KnowledgeBase,
    /// Template / preset injection panel.
    Templates,
    /// Team collaboration panel (placeholder; migrated from legacy views).
    Team,
    /// Task details panel (placeholder; migrated from legacy views).
    Task,
    /// Dashboard aggregate view (placeholder; migrated from legacy views).
    Dashboard,
}

impl RightRailTab {
    /// Map a core right-rail panel kind to a dockable tab.
    ///
    /// Returns `None` only for [`RightRailPanel::None`], because opening a tab
    /// for "no panel" is meaningless.
    pub fn from_panel(panel: RightRailPanel) -> Option<Self> {
        match panel {
            RightRailPanel::Share => Some(Self::Share),
            RightRailPanel::Console => Some(Self::Console),
            RightRailPanel::Files => Some(Self::Files),
            RightRailPanel::ClawSettings => Some(Self::ClawSettings),
            RightRailPanel::ClawWorkspace => Some(Self::ClawWorkspace),
            RightRailPanel::ClawTerminal => Some(Self::ClawTerminal),
            RightRailPanel::ClawWebBridge => Some(Self::ClawWebBridge),
            RightRailPanel::KnowledgeBase => Some(Self::KnowledgeBase),
            RightRailPanel::Templates => Some(Self::Templates),
            RightRailPanel::Team => Some(Self::Team),
            RightRailPanel::Task => Some(Self::Task),
            RightRailPanel::Dashboard => Some(Self::Dashboard),
            RightRailPanel::None => None,
        }
    }

    /// Map the dockable tab back to the core right-rail panel kind.
    pub const fn to_panel(self) -> RightRailPanel {
        match self {
            Self::Share => RightRailPanel::Share,
            Self::Console => RightRailPanel::Console,
            Self::Files => RightRailPanel::Files,
            Self::ClawSettings => RightRailPanel::ClawSettings,
            Self::ClawWorkspace => RightRailPanel::ClawWorkspace,
            Self::ClawTerminal => RightRailPanel::ClawTerminal,
            Self::ClawWebBridge => RightRailPanel::ClawWebBridge,
            Self::KnowledgeBase => RightRailPanel::KnowledgeBase,
            Self::Templates => RightRailPanel::Templates,
            Self::Team => RightRailPanel::Team,
            Self::Task => RightRailPanel::Task,
            Self::Dashboard => RightRailPanel::Dashboard,
        }
    }
}

/// Dispatch enum that materialises the currently active right-rail panel as a
/// `design_system::Panel` implementation. This keeps header concerns in one
/// place while letting each panel module own its rendering.
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
                render_empty_state(ui, app.t(hint_key), &app.context.ui_store.theme);
            }
        }
    }
}

/// Renders the tab bar and delegates each tab to its functional panel.
struct RightRailTabViewer<'a> {
    app: &'a mut App,
}

impl<'a> egui_dock::TabViewer for RightRailTabViewer<'a> {
    type Tab = RightRailTab;

    fn title(&mut self, tab: &mut Self::Tab) -> egui::WidgetText {
        let app = &*self.app;
        let panel = ActivePanel::from_kind(tab.to_panel());
        egui::WidgetText::from(panel.title(app))
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab: &mut Self::Tab) {
        let panel_kind = tab.to_panel();
        let show_hints = panel_kind == RightRailPanel::Console
            && crate::panels::chat::is_empty_state(&*self.app);
        if show_hints {
            render_quick_start_hints(self.app, ui);
            crate::design_system::gap(ui, crate::design_system::Space::S3);
        }

        let mut panel = ActivePanel::from_kind(panel_kind);
        panel.render(self.app, ui);
    }

    fn on_close(&mut self, tab: &mut Self::Tab) -> egui_dock::tab_viewer::OnCloseResponse {
        // Hide the rail when the currently active tab is closed. This also
        // covers the "last tab" case because the last remaining tab is active.
        if self
            .app
            .current_right_rail()
            .map(|p| tab.to_panel() == *p)
            .unwrap_or(false)
        {
            self.app
                .context
                .ui_store
                .right_rail_tab_close_hide_requested = true;
        }
        egui_dock::tab_viewer::OnCloseResponse::Close
    }

    fn scroll_bars(&self, _tab: &Self::Tab) -> [bool; 2] {
        // Each panel manages its own scrolling; disable the dock body's
        // scrollbars to avoid nested scroll areas.
        [false, false]
    }

    fn allowed_in_windows(&self, _tab: &mut Self::Tab) -> bool {
        // Keep tabs anchored inside the right rail; floating windows would
        // escape the intended IDE layout.
        false
    }
}

/// Render the IDE-style right rail panel.
pub fn render_right_ide_panel(app: &mut App, ui: &mut egui::Ui) {
    let ctx = ui.ctx().clone();
    let theme = app.context.ui_store.theme.clone();
    let panel_at_start = app
        .current_right_rail()
        .copied()
        .unwrap_or(RightRailPanel::None);
    let inset = theme.space_4 as i8;
    let inner_margin = egui::Margin::symmetric(theme.space_12 as i8, theme.space_16 as i8);
    let outer_margin = egui::Margin {
        left: 0,
        right: inset,
        top: inset,
        bottom: inset,
    };

    // ── Synchronise router requests into the dock ──
    // When the authoritative panel kind changes (e.g. Bot bar click), make
    // sure the matching tab exists and is focused.
    let prev = app
        .panel_animation
        .prev_panel
        .unwrap_or(RightRailPanel::None);
    if panel_at_start != prev && panel_at_start != RightRailPanel::None {
        app.open_or_focus_right_rail_tab(panel_at_start);
    }

    // ── Animated width ──
    // Visibility is driven by the router; egui's built-in animation helper
    // interpolates the width so the chat area doesn't jump while the rail
    // opens or closes.
    let is_visible = panel_at_start != RightRailPanel::None;
    let factor = theme.animate_bool_normal(ui.ctx(), egui::Id::new("right_rail_width"), is_visible);
    if factor <= 0.0 && !is_visible {
        app.panel_animation.prev_panel = Some(RightRailPanel::None);
        return;
    }
    let user_w = app
        .context
        .ui_store
        .right_rail_width
        .unwrap_or(theme.size_panel_right)
        .clamp(180.0, 400.0);
    let effective_w = (user_w * factor).max(0.0);
    let fully_open = is_visible && factor >= 1.0;

    let panel_response = egui::Panel::right("right_ide_panel")
        // LAYOUT-EXEMPT: panel width bounds chosen for IDE-style utility rail.
        .default_size(effective_w.ceil().max(0.0))
        .min_size(if fully_open { 180.0 } else { 0.0 })
        .max_size(if fully_open { 400.0 } else { effective_w })
        .resizable(fully_open)
        .show_separator_line(false)
        .frame(
            egui::Frame::side_top_panel(ui.style())
                .fill(egui::Color32::TRANSPARENT)
                .stroke(egui::Stroke::NONE)
                .shadow(egui::Shadow::NONE)
                .inner_margin(inner_margin)
                .outer_margin(outer_margin),
        )
        .show(ui, |ui| {
            if crate::ui::debug_overlay::is_enabled(&ctx) {
                crate::ui::debug_overlay::show_layout_state(ui, "right-ide-panel");
            }
            ui.set_min_width(ui.available_width());

            // Take the dock out of `app` for the duration of `show_inside` so
            // that the viewer can hold a mutable borrow of `app` without
            // conflicting with the dock's own mutable borrow.
            //
            // ponytail: `std::mem::replace` followed by a normal-path restore
            // loses the dock if `show_inside` panics. The caller wraps this
            // panel in `render_safe`, so the app survives but the right rail
            // becomes empty until the user reopens a panel. A panic-safe guard
            // (drop impl or `catch_unwind` around `show_inside`) can be added
            // if this becomes observable in practice.
            // TODO(P2 follow-up): make dock take/restore panic-safe.
            let mut dock = std::mem::replace(
                &mut app.context.ui_store.right_rail_dock,
                egui_dock::DockState::new(vec![]),
            );
            egui_dock::DockArea::new(&mut dock)
                .style(egui_dock::Style::from_egui(ui.style()))
                .show_add_buttons(false)
                .show_inside(ui, &mut RightRailTabViewer { app });
            app.context.ui_store.right_rail_dock = dock;
        });

    if fully_open {
        app.context.ui_store.right_rail_width = Some(panel_response.response.rect.width());
    }

    // ── Post-render synchronisation back to the router ──
    // The dock is the visual source of truth once it is on screen: the active
    // tab updates the router, and closing the active/last tab hides the rail.
    let close_happened = app.context.ui_store.right_rail_tab_close_hide_requested;
    if close_happened {
        app.context.ui_store.right_rail_tab_close_hide_requested = false;
        app.collapse_right_rail();
    } else if let Some((_, active_tab)) = app.context.ui_store.right_rail_dock.find_active_focused()
    {
        // Keep the router as the single source of truth: reflect the active
        // dock tab without discarding the existing stack (replace is
        // idempotent when the panel is already current).
        let panel = active_tab.to_panel();
        app.right_rail_router.replace(panel);
    } else if app
        .context
        .ui_store
        .right_rail_dock
        .iter_all_tabs()
        .next()
        .is_none()
    {
        app.collapse_right_rail();
    }

    // Store the previous panel for the animation state machine. If a close
    // gesture happened this frame, keep the old panel as `prev` so the width
    // collapse animation plays next frame.
    app.panel_animation.prev_panel = Some(if close_happened {
        panel_at_start
    } else {
        app.current_right_rail()
            .copied()
            .unwrap_or(RightRailPanel::None)
    });

    // The native resize hover/drag line is drawn by egui on top of the panel
    // contents and cannot be disabled with `show_separator_line(false)`. Cover
    // the line with the background color and redraw the divider ourselves so it
    // aligns cleanly with the rounded main-stage surface.
    let panel_rect = panel_response.response.rect;
    let screen = ctx.input(|i| i.viewport_rect());
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
    let theme = app.context.ui_store.theme.clone();
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
            let chip = clarity_ui::design_system::Elevation::Elevated
                .frame(&theme)
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
                        clarity_ui::design_system::text_with_color(
                            ui,
                            cmd,
                            clarity_ui::design_system::TextStyle::Small.mono(),
                            theme.accent,
                        );
                        crate::design_system::gap(ui, crate::design_system::Space::S1);
                        clarity_ui::design_system::text_with_color(
                            ui,
                            desc,
                            clarity_ui::design_system::TextStyle::Small,
                            theme.text_muted,
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            clarity_ui::design_system::icon_with_color(
                                ui,
                                "→",
                                theme.text_xs,
                                theme.text_dim,
                            );
                        });
                    });
                });
            let chip_response = chip
                .response
                .on_hover_cursor(egui::CursorIcon::PointingHand)
                .on_hover_text(format!("{} {}", cmd, desc));
            if chip_response.clicked() {
                app.chat_store_mut().input = format!("{} ", cmd);
                app.context.ui_store.focus_target = Some(FocusTarget::ChatInput);
            }
        }
    });
}

/// Shared empty-state widget used by placeholder right-rail panels.
fn render_empty_state(ui: &mut egui::Ui, message: &str, theme: &crate::theme::Theme) {
    clarity_ui::design_system::Elevation::Surface
        .frame(theme)
        .fill(theme.surface)
        .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
        .inner_margin(egui::Margin::same(theme.space_16 as i8))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.vertical_centered(|ui| {
                clarity_ui::design_system::icon_with_color(
                    ui,
                    crate::theme::ICON_LAYERS,
                    theme.text_2xl,
                    theme.text_dim,
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
