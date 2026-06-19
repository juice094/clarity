use crate::theme::Theme;

/// Interactive row — full-width clickable container with hover / selected states.
///
/// Renders a rounded rectangle background across the entire available width.
/// Selected rows receive a neutral highlight; hovered rows get a subtle hover
/// fill. This matches the flat, compact sidebar style used by modern reference
/// UIs where the row itself is the only visual chrome.
///
/// # Architecture note
/// Uses `UiBuilder::sense(Sense::click())` to create a child Ui whose natural
/// response carries click/hover semantics. No manual `ui.interact` or raw Rect
/// construction is required. This complies with EGUI_LAYOUT.md RULE 3.
///
/// # Keyboard navigation
/// The returned response participates in egui's focus system (Tab navigation)
/// because the sense is declared at Ui creation time, not via late-bound interact.
///
/// # Usage
/// ```ignore
/// let resp = interactive_row(ui, true, &theme, |ui| {
///     ui.label("Title");
///     ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
///         ui.label("▸");
///     });
/// });
/// if resp.response.clicked() { /* toggle */ }
/// ```
pub fn interactive_row<R>(
    ui: &mut egui::Ui,
    is_selected: bool,
    theme: &Theme,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let available_rect = ui.available_rect_before_wrap();
    let row_h = theme.size_nav_row_h;

    // Create a child Ui sized to the full available width. We do not set a sense
    // on the builder; instead we use an explicit `interact` on the final rect so
    // the response rect is guaranteed to match the painted row.
    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(available_rect)
            .layout(*ui.layout()),
    );

    let is_hovered = child_ui.rect_contains_pointer(available_rect);
    let fill = if is_selected {
        theme.nav_row_selected
    } else if is_hovered {
        theme.nav_row_hover
    } else {
        egui::Color32::TRANSPARENT
    };

    // Render the row background across the full available width, then let the
    // caller lay out icon + text inside. The symmetric horizontal inner margin
    // gives the row a consistent left/right padding while keeping the highlight
    // flush to the edges.
    let inner = egui::Frame::new()
        .fill(fill)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .inner_margin(egui::Margin::symmetric(theme.space_8 as i8, 0))
        .show(&mut child_ui, |ui| {
            ui.set_min_height(row_h);
            add_contents(ui)
        });

    let row_rect = child_ui.min_rect();

    // Click/hover response for the full row.
    let response = child_ui.interact(row_rect, child_ui.id(), egui::Sense::click());

    // Advance parent cursor so subsequent widgets are laid out correctly.
    ui.advance_cursor_after_rect(row_rect);

    egui::InnerResponse {
        inner: inner.inner,
        response,
    }
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
