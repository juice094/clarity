use crate::theme::Theme;
use crate::widgets;

pub fn send_button(ui: &mut egui::Ui, theme: &Theme) -> egui::Response {
    widgets::icon_button(
        ui,
        crate::theme::ICON_SEND,
        theme.text_lg,
        theme.accent,
        egui::CornerRadius::same(theme.radius_md as u8),
        theme,
    )
    .on_hover_text("Send message")
}
