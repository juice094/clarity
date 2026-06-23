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
        // Header row — full-width clickable with hover feedback.
        // It is never shown as "selected"; the expand/collapse state is only
        // communicated by the chevron direction.
        let header_resp = interactive_row(ui, false, theme, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;

                // Fixed-width icon rail: chevron + section icon centered so the
                // title starts on the same grid as nav item text.
                let chevron = if *is_expanded {
                    crate::theme::ICON_CARET_DOWN
                } else {
                    crate::theme::ICON_CARET_RIGHT
                };
                ui.allocate_ui_with_layout(
                    egui::vec2(theme.size_nav_icon_rail, theme.size_nav_row_h),
                    egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
                    |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = theme.space_4;
                            ui.label(
                                egui::RichText::new(chevron)
                                    .font(theme.font_icon(theme.text_sm))
                                    .color(theme.text_muted),
                            );
                            ui.label(
                                egui::RichText::new(icon)
                                    .font(theme.font_icon(theme.text_sm))
                                    .color(theme.text_dim),
                            );
                        });
                    },
                );

                ui.add_space(theme.space_8);

                ui.label(
                    egui::RichText::new(title)
                        .size(theme.text_sm)
                        .color(theme.text),
                );
            });
        });

        if header_resp.response.clicked() {
            *is_expanded = !*is_expanded;
        }

        let was_expanded = *is_expanded;

        // Body: rows use the same interactive_row layout as the header, so they
        // already share the icon rail grid. No extra indent is needed; this
        // keeps the sidebar flat and maximizes usable width.
        let body = if *is_expanded {
            Some(add_contents(ui))
        } else {
            None
        };

        egui::InnerResponse::new((was_expanded, body), header_resp.response)
    })
    .inner
}
