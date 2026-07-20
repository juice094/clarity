use crate::App;

mod tui_style;

/// Renders the input UI.
pub fn render_input(app: &mut App, ui: &mut egui::Ui) {
    tui_style::render_tui_input(app, ui);
}

/// Estimate the height the active-state composer needs inside
/// `render_input_panel`.
pub fn estimate_height(app: &App) -> f32 {
    tui_style::estimate_input_height(app)
}
