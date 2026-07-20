//! Custom titlebar widget — window drag region, sidebar toggle, and OS chrome buttons.
//!
//! Extracted from `main.rs` per the egui panel render limit (300 lines).

use crate::App;

impl App {
    /// Render a custom titlebar with window drag and control buttons.
    ///
    /// LAYOUT (two independent sub-layouts at the same vertical origin):
    ///   ┌─ left_to_right ──────────────────────────┐  ┌─ right_to_left ─┐
    ///   │ [☰] Clarity  [drag region ─── elastic]  │  │ [─] [□] [✕]    │
    ///   └──────────────────────────────────────────┘  └─────────────────┘
    ///
    /// ARCHITECTURE NOTE:
    ///   The drag region uses `allocate_exact_size` ONLY inside a horizontal
    ///   sub-layout, so `avail` is REMAINING WIDTH — not the full panel height.
    ///   This avoids the layout feedback loop where the drag region consumed
    ///   the entire panel, forcing content below and causing panel growth
    ///   every frame.
    ///
    ///   Button sub-layout (right_to_left) is rendered second, so its buttons
    ///   have higher z-order than the drag region — clicks on buttons are
    ///   NOT swallowed by the drag.
    pub(crate) fn render_titlebar(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let theme = self.context.ui_store.theme.clone();

        egui::Panel::top("titlebar")
            .exact_size(theme.size_titlebar)
            .resizable(false)
            .show_separator_line(false)
            .frame(
                clarity_ui::design_system::Elevation::Base
                    .frame(&theme)
                    .fill(theme.bg)
                    .stroke(egui::Stroke::NONE)
                    .inner_margin(egui::Margin::symmetric(8, 0)),
            )
            .show(ui, |ui| {
                let titlebar_rect = ui.max_rect();

                // Register the entire titlebar as a drag region first; buttons
                // rendered afterwards automatically override this hitbox.
                let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                let drag_resp = ui.interact(
                    titlebar_rect,
                    ui.id().with("titlebar_drag"),
                    egui::Sense::click_and_drag(),
                );
                if drag_resp.drag_started_by(egui::PointerButton::Primary) {
                    ctx.send_viewport_cmd(egui::ViewportCommand::StartDrag);
                }
                if drag_resp.double_clicked() {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                }

                ui.horizontal(|ui| {
                    // Sidebar toggle.
                    let sidebar_tooltip = if self.view_state.left_rail_expanded {
                        "Collapse sidebar"
                    } else {
                        "Expand sidebar"
                    };
                    if crate::widgets::icon_button_toolbar(
                        ui,
                        crate::theme::ICON_LIST,
                        theme.text_base,
                        &theme,
                    )
                    .on_hover_text(sidebar_tooltip)
                    .clicked()
                    {
                        self.view_state.left_rail_expanded = !self.view_state.left_rail_expanded;
                    }

                    // Right-aligned window controls.
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = 0.0;

                        // Close.
                        let close = crate::widgets::window_control_button(
                            ui,
                            crate::theme::ICON_X,
                            &theme,
                            theme.danger.linear_multiply(0.25),
                            egui::Color32::WHITE,
                            theme.text,
                        )
                        .on_hover_text("Close window");
                        if close.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(false));
                        }

                        // Maximize / restore.
                        let max_icon = if is_maximized {
                            crate::theme::ICON_COPY
                        } else {
                            crate::theme::ICON_SQUARE
                        };
                        let max = crate::widgets::window_control_button(
                            ui,
                            max_icon,
                            &theme,
                            theme.overlay_medium,
                            theme.text,
                            theme.text,
                        )
                        .on_hover_text(if is_maximized {
                            "Restore window"
                        } else {
                            "Maximize window"
                        });
                        if max.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                        }

                        // Minimize.
                        let min = crate::widgets::window_control_button(
                            ui,
                            crate::theme::ICON_MINUS,
                            &theme,
                            theme.overlay_medium,
                            theme.text,
                            theme.text,
                        )
                        .on_hover_text("Minimize to taskbar");
                        if min.clicked() {
                            ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                        }
                    });
                });
            });
    }
}
