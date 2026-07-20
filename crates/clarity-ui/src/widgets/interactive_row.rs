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

    // Animate hover and press states for a polished micro-interaction.
    // Normal state is transparent so the sidebar surface shows through.
    let hover_target = if response.hovered() { 1.0 } else { 0.0 };
    let press_target = if response.is_pointer_button_down_on() {
        1.0
    } else {
        0.0
    };
    let hover = ui.ctx().animate_value_with_time(
        response.id.with("hover"),
        hover_target,
        theme.duration_normal,
    );
    let press = ui.ctx().animate_value_with_time(
        response.id.with("press"),
        press_target,
        theme.duration_fast,
    );

    let selected_fill = theme.accent.gamma_multiply(0.18);
    let hover_fill = theme.accent.gamma_multiply(0.10);
    let press_fill = theme.accent.gamma_multiply(0.24);
    let base_fill = if is_selected {
        selected_fill
    } else {
        egui::Color32::TRANSPARENT
    };
    let fill = lerp_color(lerp_color(base_fill, hover_fill, hover), press_fill, press);
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
        .scope_builder(egui::UiBuilder::new().max_rect(rect), move |ui| {
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

/// Linearly interpolate between two premultiplied RGBA colours.
fn lerp_color(a: egui::Color32, b: egui::Color32, t: f32) -> egui::Color32 {
    let t = t.clamp(0.0, 1.0);
    egui::Color32::from_rgba_premultiplied(
        (a.r() as f32 + (b.r() as f32 - a.r() as f32) * t) as u8,
        (a.g() as f32 + (b.g() as f32 - a.g() as f32) * t) as u8,
        (a.b() as f32 + (b.b() as f32 - a.b() as f32) * t) as u8,
        (a.a() as f32 + (b.a() as f32 - a.a() as f32) * t) as u8,
    )
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
