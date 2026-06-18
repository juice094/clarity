use crate::theme::Theme;

/// Interactive row — full-width clickable container with free child layout.
///
/// Replaces the anti-pattern of `ui.interact(Rect::from_min_size(...))` + manual
/// painter overlay. Provides theme-consistent hover/selected backgrounds while
/// allowing arbitrary child widgets inside the row.
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

    // Create a child Ui with built-in click sense.
    // This is the canonical egui 0.31+ way to make an arbitrary region interactive
    // without resorting to ui.interact(raw_rect).
    let mut child_ui = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(available_rect)
            .layout(*ui.layout())
            .sense(egui::Sense::click()),
    );

    // Render background + contents inside the click-sensed child Ui.
    let inner = egui::Frame::new()
        .fill(egui::Color32::TRANSPARENT)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .show(&mut child_ui, |ui| {
            ui.set_min_width(available_rect.width());

            let is_hovered = ui.rect_contains_pointer(ui.max_rect());
            // Selected rows get a subtle accent tint; hovered rows get a
            // neutral background. This distinguishes "you are here" from
            // "you might click here" without adding extra chrome.
            let fill = if is_selected {
                theme.nav_row_selected
            } else if is_hovered {
                theme.nav_row_hover
            } else {
                egui::Color32::TRANSPARENT
            };

            // Return the content directly so we get InnerResponse<R>, not nested.
            egui::Frame::new()
                .fill(fill)
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                .show(ui, add_contents)
                .inner
        });

    // The child Ui accumulates a response from all widgets inside it.
    // Because we set Sense::click() at construction, this response natively
    // supports clicked(), hovered(), and focus ring — no ui.interact needed.
    let response = child_ui.response();

    // Advance parent cursor so subsequent widgets are laid out correctly.
    ui.advance_cursor_after_rect(child_ui.min_rect());

    egui::InnerResponse {
        inner: inner.inner,
        response,
    }
}
