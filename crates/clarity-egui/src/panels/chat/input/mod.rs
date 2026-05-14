use crate::stores::InputStyle;
use crate::App;

mod gui_style;
mod tui_style;

/// Route to the appropriate input renderer based on `UiStore.input_style`.
pub fn render_input(app: &mut App, ui: &mut egui::Ui) {
    match app.ui_store.input_style {
        InputStyle::Gui => gui_style::render_gui_input(app, ui),
        InputStyle::Tui => tui_style::render_tui_input(app, ui),
    }
}
