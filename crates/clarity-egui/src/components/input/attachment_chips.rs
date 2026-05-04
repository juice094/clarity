use crate::theme::Theme;
use crate::ui::types::Attachment;

pub fn attachment_chips(
    ui: &mut egui::Ui,
    attachments: &[Attachment],
    theme: &Theme,
) -> Option<usize> {
    if attachments.is_empty() {
        return None;
    }
    let mut to_remove: Option<usize> = None;
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("Attachments:")
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        for (i, att) in attachments.iter().enumerate() {
            egui::Frame::group(ui.style())
                .fill(theme.surface)
                .corner_radius(egui::CornerRadius::same(theme.radius_full as u8))

                .inner_margin(egui::Margin::symmetric(8, 4))
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(crate::theme::ICON_PAPERCLIP)
                                .font(theme.font_icon(theme.text_sm)),
                        );
                        ui.label(
                            egui::RichText::new(&att.name)
                                .size(theme.text_sm)
                                .color(theme.text)
                                .monospace(),
                        );
                        if ui
                            .add(
                                egui::Button::new(
                                    egui::RichText::new(crate::theme::ICON_X)
                                        .font(theme.font_icon(theme.text_xs)),
                                )
                                .small(),
                            )
                            .clicked()
                        {
                            to_remove = Some(i);
                        }
                    });
                });
        }
    });
    to_remove
}
