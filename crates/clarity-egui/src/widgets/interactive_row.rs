use crate::theme::Theme;

/// Interactive row — full-width clickable container with hover / selected states.
///
/// Renders a rounded rectangle background across the entire available width.
/// Selected rows use a stronger surface fill plus an accent stroke; hovered rows
/// get the same stronger fill. This matches the visual treatment of the settings
/// provider list so the sidebar and settings panels feel consistent.
///
/// # Architecture note
/// Uses `ui.allocate_exact_size(..., Sense::click())` to reserve a precise row
/// rectangle, then paints the frame and lets the caller lay out content inside.
/// Hover is read from the returned `Response`, which respects egui's widget
/// layering and does not bleed into the remaining sidebar area.
///
/// # Keyboard navigation
/// The returned response participates in egui's focus system because the sense
/// is declared at allocation time.
///
/// # Usage
/// ```ignore
/// let resp = interactive_row(ui, true, &theme, |ui| {
///     ui.label("Title");
/// });
/// if resp.response.clicked() { /* handle click */ }
/// ```
pub fn interactive_row<R>(
    ui: &mut egui::Ui,
    is_selected: bool,
    theme: &Theme,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let desired_size = egui::vec2(ui.available_width(), theme.size_nav_row_h);
    let (rect, response) = ui.allocate_exact_size(desired_size, egui::Sense::click());

    // Prevent text/icons inside the row from being selectable or stealing hover.
    // This keeps the row behaving like a single button rather than a floating
    // text layer above the background.
    let mut style: egui::Style = (**ui.style()).clone();
    style.interaction.selectable_labels = false;
    let style = std::sync::Arc::new(style);

    // Screenshot-style selection/hover: accent-tinted rounded background.
    // Normal state is transparent so the sidebar surface shows through.
    let fill = if is_selected {
        theme.accent.gamma_multiply(0.18)
    } else if response.hovered() {
        theme.accent.gamma_multiply(0.10)
    } else {
        egui::Color32::TRANSPARENT
    };
    let stroke = egui::Stroke::NONE;

    // VS Code-style accent indicator bar (2px left edge when selected).
    if is_selected {
        let bar_w = 2.0;
        let bar_h = (rect.height() - 8.0).max(4.0);
        let bar_rect = egui::Rect::from_min_size(
            egui::pos2(rect.left(), rect.center().y - bar_h / 2.0),
            egui::vec2(bar_w, bar_h),
        );
        ui.painter()
            .rect_filled(bar_rect, egui::CornerRadius::same(1), theme.accent);
    }

    let inner = ui
        .allocate_new_ui(egui::UiBuilder::new().max_rect(rect), move |ui| {
            ui.set_style(style);
            let frame_resp = egui::Frame::new()
                .fill(fill)
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                .stroke(stroke)
                .inner_margin(egui::Margin::symmetric(theme.space_8 as i8, 0))
                .show(ui, |ui| {
                    ui.set_min_size(ui.available_size());
                    add_contents(ui)
                });
            frame_resp.inner
        })
        .inner;

    // Focus ring (shared helper).
    if response.has_focus() {
        crate::design_system::paint_focus_ring(
            ui,
            rect,
            egui::CornerRadius::same(theme.radius_sm as u8),
        );
    }

    egui::InnerResponse { inner, response }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;

    fn run_in_frame<R>(f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ctx = egui::Context::default();
        let mut f_opt = Some(f);
        let mut output = None;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(400.0, 800.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                if let Some(f) = f_opt.take() {
                    output = Some(f(ui));
                }
            });
        });
        output.expect("CentralPanel should always run its closure")
    }

    #[test]
    fn interactive_row_allocates_space_and_click_response() {
        let theme = Theme::default();
        let resp = run_in_frame(|ui| {
            interactive_row(ui, false, &theme, |ui| {
                ui.label("row");
            })
        });
        assert!(resp.response.rect.width() > 0.0);
        assert!(resp.response.rect.height() > 0.0);
    }

    #[test]
    fn interactive_row_selected_returns_same_response_shape() {
        let theme = Theme::default();
        let resp = run_in_frame(|ui| {
            interactive_row(ui, true, &theme, |ui| {
                ui.label("row");
            })
        });
        assert!(resp.response.rect.width() > 0.0);
        assert!(resp.response.rect.height() > 0.0);
    }
}
