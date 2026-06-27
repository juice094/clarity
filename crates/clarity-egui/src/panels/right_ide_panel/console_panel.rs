//! Console panel — task execution log and terminal output viewer.
//!
//! Displays a filterable, scrollable log of tool executions, status
//! messages, and errors emitted by the agent loop.  Rendered in the
//! right IDE rail.

use crate::App;
use crate::stores::console::{ConsoleFilter, ConsoleLevel};

/// Maximum visible entry width in characters before truncation hint.
const CONSOLE_MAX_VISIBLE_CHARS: usize = 2000;

/// Render the console panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    // --- filter bar ---
    ui.horizontal(|ui| {
        let filters = [
            ConsoleFilter::All,
            ConsoleFilter::Errors,
            ConsoleFilter::Warnings,
            ConsoleFilter::ToolOutput,
            ConsoleFilter::Status,
        ];
        // Pre-compute per-level counts.
        let total = app.console_store.entries.len();
        let errs = app
            .console_store
            .entries
            .iter()
            .filter(|e| e.level == ConsoleLevel::Error)
            .count();
        let warns = app
            .console_store
            .entries
            .iter()
            .filter(|e| e.level == ConsoleLevel::Warn)
            .count();
        let tools = app
            .console_store
            .entries
            .iter()
            .filter(|e| e.level == ConsoleLevel::ToolOutput)
            .count();
        let stats = app
            .console_store
            .entries
            .iter()
            .filter(|e| e.level == ConsoleLevel::Status)
            .count();

        for f in filters {
            let count = match f {
                ConsoleFilter::All => total,
                ConsoleFilter::Errors => errs,
                ConsoleFilter::Warnings => warns,
                ConsoleFilter::ToolOutput => tools,
                ConsoleFilter::Status => stats,
            };
            let label = format!("{} ({})", app.t(f.label()), count);
            let is_active = app.console_store.filter == f;
            let chip =
                egui::Frame::new()
                    .fill(if is_active {
                        theme.accent_subtle
                    } else {
                        theme.surface
                    })
                    .stroke(egui::Stroke::new(
                        1.0,
                        if is_active {
                            theme.accent
                        } else {
                            theme.border
                        },
                    ))
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                    .inner_margin(egui::Margin::symmetric(
                        theme.space_8 as i8,
                        theme.space_4 as i8,
                    ))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(label).size(theme.text_xs).color(
                            if is_active {
                                theme.accent
                            } else {
                                theme.text_dim
                            },
                        ));
                    });
            if chip.response.clicked() {
                app.console_store.filter = f;
            }
            if chip.response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let auto_label = app.t("Auto");
            ui.toggle_value(&mut app.console_store.auto_scroll, auto_label);
            ui.add_space(theme.space_4);
            let clear_label = app.t("Clear");
            if ui
                .add_sized(
                    [40.0, 20.0],
                    egui::Button::new(
                        egui::RichText::new(clear_label)
                            .size(theme.text_xs)
                            .color(theme.text_dim),
                    )
                    .fill(theme.surface)
                    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8)),
                )
                .clicked()
            {
                app.console_store.entries.clear();
            }
        });
    });
    ui.add_space(theme.space_8);

    // --- log area ---
    let scroll = egui::ScrollArea::vertical()
        .id_salt("console_log")
        .auto_shrink([false; 2])
        .stick_to_bottom(app.console_store.auto_scroll);

    let (row_height, _row_spacing) = (theme.text_sm + theme.space_4, theme.space_4);

    scroll.show_rows(
        ui,
        row_height,
        app.console_store.filtered().count(),
        |ui, row_range| {
            let entries: Vec<_> = app
                .console_store
                .filtered()
                .skip(row_range.start)
                .take(row_range.end - row_range.start)
                .cloned()
                .collect();

            for entry in entries {
                render_console_row(app, ui, &entry, &theme);
            }

            if app.console_store.filtered().count() == 0 {
                let msg = if app.console_store.entries.is_empty() {
                    app.t("Waiting for agent output…").to_string()
                } else {
                    app.t("No entries matching filter").to_string()
                };
                ui.label(
                    egui::RichText::new(msg)
                        .size(theme.text_sm)
                        .color(theme.text_dim),
                );
            }
        },
    );
}

fn render_console_row(
    app: &mut App,
    ui: &mut egui::Ui,
    entry: &crate::stores::console::ConsoleEntry,
    theme: &crate::theme::Theme,
) {
    let elapsed = entry.timestamp.elapsed().as_secs_f32();
    let ts = if elapsed < 1.0 {
        app.t("now").to_string()
    } else if elapsed < 60.0 {
        format!("{}s", elapsed as u32)
    } else if elapsed < 3600.0 {
        format!("{}m{}s", elapsed as u32 / 60, elapsed as u32 % 60)
    } else {
        format!("{}h", elapsed as u32 / 3600)
    };

    let (icon, color) = match entry.level {
        ConsoleLevel::Info => (crate::theme::ICON_INFO, theme.text_dim),
        ConsoleLevel::Warn => (crate::theme::ICON_WARNING, theme.warn),
        ConsoleLevel::Error => (crate::theme::ICON_X, theme.danger),
        ConsoleLevel::ToolOutput => (crate::theme::ICON_TERMINAL, theme.text_muted),
        ConsoleLevel::Status => (crate::theme::ICON_CHECK, theme.ok),
    };

    let clickable = entry.level == ConsoleLevel::Error || entry.level == ConsoleLevel::Warn;

    let row_resp = if clickable {
        egui::Frame::new()
            .fill(
                if ui.rect_contains_pointer(ui.available_rect_before_wrap()) {
                    theme.bg_hover
                } else {
                    theme.surface
                },
            )
            .inner_margin(egui::Margin::symmetric(theme.space_4 as i8, 2))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(ts)
                            .size(theme.text_xs)
                            .color(theme.text_dim)
                            .monospace(),
                    );
                    ui.label(
                        egui::RichText::new(icon)
                            .font(theme.font_icon(theme.text_xs))
                            .color(color),
                    );
                    ui.label(
                        egui::RichText::new(&entry.source)
                            .size(theme.text_xs)
                            .color(theme.text_muted),
                    );
                    let display =
                        crate::ui::truncate::truncate(&entry.message, CONSOLE_MAX_VISIBLE_CHARS);
                    ui.label(
                        egui::RichText::new(&display)
                            .size(theme.text_xs)
                            .color(color)
                            .monospace(),
                    );
                });
            })
            .response
    } else {
        egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(theme.space_4 as i8, 2))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(ts)
                            .size(theme.text_xs)
                            .color(theme.text_dim)
                            .monospace(),
                    );
                    ui.label(
                        egui::RichText::new(icon)
                            .font(theme.font_icon(theme.text_xs))
                            .color(color),
                    );
                    ui.label(
                        egui::RichText::new(&entry.source)
                            .size(theme.text_xs)
                            .color(theme.text_muted),
                    );
                    let display =
                        crate::ui::truncate::truncate(&entry.message, CONSOLE_MAX_VISIBLE_CHARS);
                    ui.label(
                        egui::RichText::new(&display)
                            .size(theme.text_xs)
                            .color(theme.text)
                            .monospace(),
                    );
                });
            })
            .response
    };

    if clickable && row_resp.clicked() {
        // Inject error message into chat input for debugging.
        let snippet = format!(
            "[console] Error from {}: {}",
            entry.source,
            entry.message.lines().next().unwrap_or(&entry.message)
        );
        app.chat_store.input = snippet;
        app.ui_store.focus_input_requested = true;
    }

    if entry.truncated {
        ui.label(
            egui::RichText::new(app.t("…output truncated"))
                .size(theme.text_xs)
                .color(theme.text_dim)
                .italics(),
        );
    }
}
