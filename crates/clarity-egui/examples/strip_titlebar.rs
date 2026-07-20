#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    missing_docs,
    unsafe_code
)]
//! S2.P1.0 PoC — StripBuilder titlebar layout validation.
//!
//! Validates that `egui_extras::StripBuilder` can replace the current
//! imperative `ui.horizontal + estimated_right_w` titlebar pattern in
//! `crates/clarity-egui/src/main.rs:193-268` with a declarative
//! three-zone (LEFT exact / CENTER remainder / RIGHT exact) layout.
//!
//! Run with:
//! ```bash
//! cargo run --example strip_titlebar -p clarity-egui
//! ```
//!
//! Acceptance criteria:
//! 1. LEFT zone: brand label, fixed width based on content
//! 2. CENTER zone: simulated tabs + drag filler, takes remaining space
//! 3. RIGHT zone: window controls, fixed width
//! 4. Resize window: LEFT/RIGHT stay exact, CENTER grows/shrinks
//! 5. No `estimated_right_w` heuristic; widths are declared, not computed
//!
//! This PoC informs RULE 6 (chrome must use StripBuilder) in
//! `EGUI_LAYOUT.md` and the subsequent P1.2 refactor of `render_titlebar`.

use eframe::egui;
use egui_extras::{Size, StripBuilder};

const TITLEBAR_HEIGHT: f32 = 36.0;
const LEFT_ZONE_WIDTH: f32 = 140.0; // brand + sidebar toggle
const RIGHT_ZONE_WIDTH: f32 = 280.0; // 4 window controls + 2 status capsules

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([900.0, 600.0])
            .with_decorations(false)
            .with_transparent(false),
        ..Default::default()
    };

    eframe::run_ui_native("StripTitlebar PoC", options, |ui, _frame| {
        egui::CentralPanel::default()
            .frame(egui::Frame::new().fill(egui::Color32::from_rgb(28, 28, 32)))
            .show(ui, |ui| {
                render_titlebar(ui);
                ui.add_space(8.0);
                ui.heading("Body — resize the window to verify CENTER zone behavior");
                ui.label(
                    "LEFT and RIGHT zones should stay at their declared widths; \
                     CENTER expands and contracts.",
                );
            });
    })
}

fn render_titlebar(ui: &mut egui::Ui) {
    ui.add_space(2.0);
    StripBuilder::new(ui)
        .size(Size::exact(LEFT_ZONE_WIDTH))
        .size(Size::remainder().at_least(40.0))
        .size(Size::exact(RIGHT_ZONE_WIDTH))
        .horizontal(|mut strip| {
            // ── LEFT zone ──
            strip.cell(|ui| {
                ui.set_min_height(TITLEBAR_HEIGHT);
                ui.horizontal_centered(|ui| {
                    ui.add_space(8.0);
                    if ui.button("\u{2630}").on_hover_text("Sidebar").clicked() {
                        // toggle sidebar (no-op in PoC)
                    }
                    ui.add_space(8.0);
                    ui.label(
                        egui::RichText::new("Clarity")
                            .size(14.0)
                            .strong()
                            .color(egui::Color32::from_rgb(220, 220, 220)),
                    );
                });
            });

            // ── CENTER zone (tabs + elastic drag filler) ──
            strip.cell(|ui| {
                ui.set_min_height(TITLEBAR_HEIGHT);
                ui.horizontal_centered(|ui| {
                    // Simulated session tabs
                    ui.label("[chat-001]");
                    ui.add_space(4.0);
                    ui.label("[chat-002]");
                    ui.add_space(4.0);
                    ui.label("[chat-003]");
                    ui.add_space(12.0);

                    // Drag filler — occupies all remaining space inside CENTER
                    let avail = ui.available_width();
                    let (_rect, response) = ui.allocate_exact_size(
                        egui::vec2(avail, TITLEBAR_HEIGHT),
                        egui::Sense::click_and_drag(),
                    );
                    if response.drag_started_by(egui::PointerButton::Primary) {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
                    }
                });
            });

            // ── RIGHT zone (window controls + status) ──
            strip.cell(|ui| {
                ui.set_min_height(TITLEBAR_HEIGHT);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(4.0);
                    if ui.button("\u{2715}").on_hover_text("Close").clicked() {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    if ui.button("\u{2610}").on_hover_text("Maximize").clicked() {
                        // toggle maximize (no-op in PoC)
                    }
                    if ui.button("\u{2014}").on_hover_text("Minimize").clicked() {
                        ui.ctx()
                            .send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                    ui.add_space(8.0);
                    ui.label("\u{25CF}  Gateway");
                    ui.add_space(8.0);
                    ui.label("\u{25CF}  3 sessions");
                });
            });
        });
}
