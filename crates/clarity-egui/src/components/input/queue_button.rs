use crate::theme::Theme;
use crate::widgets;

pub fn queue_button(ui: &mut egui::Ui, can_queue: bool, theme: &Theme) -> egui::Response {
    let fill = if can_queue {
        theme.accent
    } else {
        theme.bg_elevated
    };
    let _text_color = if can_queue {
        theme.text
    } else {
        theme.text_dim
    };
    let btn = widgets::icon_button(
        ui,
        crate::theme::ICON_PLAY,
        theme.text_lg,
        fill,
        egui::CornerRadius::same(theme.radius_full as u8),
        theme,
    );
    if can_queue {
        btn.on_hover_text("Steer — cancel current response and send immediately")
    } else {
        btn.on_hover_text("Type a message to steer")
    }
}
