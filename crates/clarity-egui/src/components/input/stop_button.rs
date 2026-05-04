use crate::theme::Theme;
use crate::widgets;

pub fn stop_button(ui: &mut egui::Ui, theme: &Theme) -> egui::Response {
    widgets::icon_button_primary(
        ui,
        crate::theme::ICON_STOP,
        theme.text_lg,
        theme.danger,
        theme,
    )
    .on_hover_text("Stop generating (Ctrl+C)")
}
