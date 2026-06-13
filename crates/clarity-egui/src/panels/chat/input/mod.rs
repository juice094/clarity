use crate::App;

mod tui_style;

/// Renders the input UI.
pub fn render_input(app: &mut App, ui: &mut egui::Ui) {
    tui_style::render_tui_input(app, ui);
}
