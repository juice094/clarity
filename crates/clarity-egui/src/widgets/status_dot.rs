use crate::theme::Theme;

/// Status indicator dot (online / offline).
/// Replaces `ui.painter().circle_filled(...)` with a standard widget.
pub fn status_dot(ui: &mut egui::Ui, online: bool, theme: &Theme) -> egui::Response {
    let color = if online {
        theme.status_online
    } else {
        theme.text_dim
    };
    ui.label(egui::RichText::new("●").size(8.0).color(color))
}
