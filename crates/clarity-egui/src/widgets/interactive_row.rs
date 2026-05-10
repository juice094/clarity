use crate::theme::Theme;

/// Interactive row — full-width clickable container with free child layout.
///
/// Replaces the anti-pattern of `ui.interact(Rect::from_min_size(...))` + manual
/// painter overlay.  Provides theme-consistent hover/selected backgrounds while
/// allowing arbitrary child widgets inside the row.
///
/// # Architecture note
/// `Frame::show` allocates the row rect through egui's layout engine (no manual
/// Rect construction).  The returned `response.rect` is then passed to
/// `ui.interact` with the caller-supplied `id`.  This is the closest possible
/// approach to egui's official paradigm when the framework lacks a built-in
/// "interactive row with custom children" widget.
///
/// # Usage
/// ```ignore
/// let resp = interactive_row(ui, id, true, &theme, |ui| {
///     ui.label("Title");
///     ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
///         ui.label("▸");
///     });
/// });
/// if resp.response.clicked() { /* toggle */ }
/// ```
pub fn interactive_row<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    is_selected: bool,
    theme: &Theme,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let available_width = ui.available_width();

    let outer = egui::Frame::new()
        .fill(egui::Color32::TRANSPARENT)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
        .show(ui, |ui| {
            ui.set_min_width(available_width);

            let is_hovered = ui.rect_contains_pointer(ui.max_rect());
            let fill = if is_selected || is_hovered {
                theme.bg_hover
            } else {
                egui::Color32::TRANSPARENT
            };

            egui::Frame::new()
                .fill(fill)
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                .show(ui, add_contents)
        });

    // Re-register with the caller-supplied id for distinct interaction identity.
    let response = ui.interact(outer.response.rect, id, egui::Sense::click());

    egui::InnerResponse {
        inner: outer.inner.inner,
        response,
    }
}
