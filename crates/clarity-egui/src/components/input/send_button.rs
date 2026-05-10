use crate::theme::Theme;
use crate::widgets;

pub fn send_button(ui: &mut egui::Ui, theme: &Theme) -> egui::Response {
    widgets::icon_button_primary(
        ui,
        crate::theme::ICON_SEND,
        theme.text_lg,
        theme.accent,
        theme,
    )
    .on_hover_text("Send message")
}
