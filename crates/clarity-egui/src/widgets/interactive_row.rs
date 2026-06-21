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

    let fill = if is_selected || response.hovered() {
        theme.surface_strong
    } else {
        theme.surface
    };
    let stroke = if is_selected {
        egui::Stroke::new(1.5, theme.accent)
    } else {
        egui::Stroke::NONE
    };

    let inner = ui
        .allocate_new_ui(egui::UiBuilder::new().max_rect(rect), |ui| {
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

    // Focus ring (P0.5.E.1).
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            egui::CornerRadius::same(theme.radius_sm as u8),
            egui::Stroke::new(2.0, theme.focus_ring),
            egui::StrokeKind::Inside,
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
