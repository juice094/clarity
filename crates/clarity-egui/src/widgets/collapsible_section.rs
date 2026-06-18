use crate::theme::Theme;
use crate::widgets::interactive_row;

/// Collapsible accordion section with a clickable header row.
///
/// The header shows `[chevron] [icon] [title]`. Clicking toggles `is_expanded`.
/// The body closure is only called when `is_expanded` is true.
///
/// # Requirements
///
/// - `stable_id`: language-independent persistent identifier (e.g. `"nav_web"`),
///   NOT a translated title. Prevents state loss on i18n switch.
/// - `is_expanded`: caller-owned `&mut bool` (typically from `PanelExpansion`).
/// - Keyboard: Enter/Space toggle via `egui::Sense::click()` on the header row.
pub fn collapsible_section<R>(
    ui: &mut egui::Ui,
    stable_id: &str,
    title: &str,
    icon: &str,
    is_expanded: &mut bool,
    theme: &Theme,
    add_contents: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<(bool, Option<R>)> {
    ui.push_id(stable_id, |ui| {
        // Header row — full-width clickable with hover feedback from interactive_row.
        let header_resp = interactive_row(ui, *is_expanded, theme, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = theme.space_8;

                let chevron = if *is_expanded {
                    crate::theme::ICON_CARET_DOWN
                } else {
                    crate::theme::ICON_CARET_RIGHT
                };
                ui.add(
                    egui::Label::new(
                        egui::RichText::new(chevron)
                            .size(theme.text_sm)
                            .color(theme.text_muted),
                    )
                    .selectable(false),
                );

                ui.add(
                    egui::Label::new(
                        egui::RichText::new(icon)
                            .size(theme.text_sm)
                            .color(theme.text_dim),
                    )
                    .selectable(false),
                );

                ui.add(
                    egui::Label::new(
                        egui::RichText::new(title)
                            .size(theme.text_sm)
                            .color(theme.text_strong),
                    )
                    .selectable(false),
                );
            });
        });

        if header_resp.response.clicked() {
            *is_expanded = !*is_expanded;
        }

        let was_expanded = *is_expanded;

        // Subtle separator between header and body when expanded.
        if *is_expanded {
            ui.add_space(theme.space_4);
        }

        // Body: conditionally rendered, indented for visual hierarchy.
        let body = if *is_expanded {
            Some(
                egui::Frame::new()
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(egui::Margin {
                        left: theme.space_16 as i8,
                        ..Default::default()
                    })
                    .show(ui, |ui| add_contents(ui))
                    .inner,
            )
        } else {
            None
        };

        egui::InnerResponse::new((was_expanded, body), header_resp.response)
    })
    .inner
}
