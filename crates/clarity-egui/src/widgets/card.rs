/// Card container using egui Frame::group.
/// Replaces repeated `Frame::group(...).fill(...).corner_radius(...).stroke(...).inner_margin(...).show(...)` patterns.
pub fn card(
    ui: &mut egui::Ui,
    fill: egui::Color32,
    stroke: egui::Stroke,
    radius: egui::CornerRadius,
    inner_margin: egui::Margin,
    content: impl FnOnce(&mut egui::Ui),
) {
    egui::Frame::group(ui.style())
        .fill(fill)
        .corner_radius(radius)
        .stroke(stroke)
        .inner_margin(inner_margin)
        .show(ui, content);
}
