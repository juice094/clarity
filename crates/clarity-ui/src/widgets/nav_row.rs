//! Flat navigation row helpers for the left sidebar.
//!
//! These helpers wrap [`interactive_row`] and [`nav_icon_rail`] so that simple
//! icon + label rows (and rows with a trailing shortcut/badge) can be rendered
//! in one call and share the same hover / selected visual treatment.

use crate::theme::Theme;
use crate::widgets::{interactive_row, nav_icon_rail};

/// Render a simple navigation row: icon rail + label.
///
/// Selected rows use the strong text color; unselected rows use the default
/// text color for the label and a dimmer color for the icon.
pub fn nav_row(
    ui: &mut egui::Ui,
    theme: &Theme,
    icon: &str,
    label: &str,
    is_selected: bool,
) -> egui::Response {
    nav_row_with_trailing(ui, theme, icon, label, is_selected, |_| {})
}

/// Render a navigation row with an optional trailing widget (shortcut key,
/// badge count, etc.).
///
/// The trailing widget is right-aligned within the row and does not affect the
/// icon/label grid.
pub fn nav_row_with_trailing(
    ui: &mut egui::Ui,
    theme: &Theme,
    icon: &str,
    label: &str,
    is_selected: bool,
    trailing: impl FnOnce(&mut egui::Ui),
) -> egui::Response {
    let icon_color = if is_selected {
        theme.accent
    } else {
        theme.text_dim
    };
    let label_color = if is_selected {
        theme.accent
    } else {
        theme.text
    };

    interactive_row(ui, is_selected, theme, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.space_8;
            nav_icon_rail(ui, theme, icon, icon_color);
            ui.label(
                egui::RichText::new(label)
                    .size(theme.text_sm)
                    .color(label_color),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                trailing(ui);
            });
        });
    })
    .response
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    fn run_in_frame<R>(f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ctx = egui::Context::default();
        crate::theme::setup_fonts(&ctx);
        let mut f_opt = Some(f);
        let mut output = None;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(400.0, 800.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                if let Some(f) = f_opt.take() {
                    output = Some(f(ui));
                }
            });
        });
        // SAFE: egui::CentralPanel::show always invokes its closure at least
        // once during the `ctx.run()` call. The `f_opt.take()` ensures the
        // function is called at most once, so output is always populated.
        output.expect("CentralPanel should always run its closure")
    }

    #[test]
    fn nav_row_allocates_space() {
        let theme = Theme::default();
        let resp = run_in_frame(|ui| nav_row(ui, &theme, "icon", "Label", false));
        assert!(resp.rect.width() > 0.0);
        assert!(resp.rect.height() > 0.0);
    }

    #[test]
    fn nav_row_with_trailing_allocates_space() {
        let theme = Theme::default();
        let resp = run_in_frame(|ui| {
            nav_row_with_trailing(ui, &theme, "icon", "Label", false, |ui| {
                ui.label("K");
            })
        });
        assert!(resp.rect.width() > 0.0);
        assert!(resp.rect.height() > 0.0);
    }
}
