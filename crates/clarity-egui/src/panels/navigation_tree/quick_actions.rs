//! Quick-action entries at the top of the left navigation tree.

use crate::App;

/// Render the quick-action section.
pub fn render_quick_actions(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.space_8;

        // New session.
        let new_session_btn = egui::Button::new(
            egui::RichText::new(format!(
                "{} {}",
                crate::theme::ICON_PLUS,
                app.t("New Session")
            ))
            .size(theme.text_sm)
            .color(theme.text),
        )
        .fill(theme.bg_hover)
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
        if ui
            .add(new_session_btn)
            .on_hover_text(app.t("New session (Ctrl+N)"))
            .clicked()
            && app.view_state.turn != clarity_core::ui::TurnState::Loading
        {
            app.new_session();
        }

        // Skills.
        if crate::widgets::icon_button_toolbar(ui, crate::theme::ICON_BOOK, theme.text_base, &theme)
            .on_hover_text(app.t("Skills"))
            .clicked()
        {
            app.view_state
                .open_modal(clarity_core::ui::ModalType::Skill);
        }

        // Cron.
        if crate::widgets::icon_button_toolbar(
            ui,
            crate::theme::ICON_HOURGLASS,
            theme.text_base,
            &theme,
        )
        .on_hover_text(app.t("New cron schedule"))
        .clicked()
        {
            app.view_state
                .open_modal(clarity_core::ui::ModalType::CronCreate);
        }

        // Settings.
        if crate::widgets::icon_button_toolbar(
            ui,
            crate::theme::ICON_SETTINGS,
            theme.text_base,
            &theme,
        )
        .on_hover_text(app.t("Settings"))
        .clicked()
        {
            app.view_state.main = clarity_core::ui::AppView::Settings;
            app.settings_store.settings_edit = {
                let guard = app.state.cached_settings.lock();
                guard.clone()
            };
        }
    });

    ui.add_space(theme.space_12);

    // Placeholder web/external link section until web tabs are migrated.
    render_section_header(ui, &theme, app.t("Web"));
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing.x = theme.space_8;
        for label in [app.t("Docs"), app.t("GitHub")] {
            let chip = egui::Button::new(
                egui::RichText::new(label)
                    .size(theme.text_xs)
                    .color(theme.text_dim),
            )
            .fill(theme.surface)
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
            let _ = ui.add(chip);
        }
    });
}

fn render_section_header(ui: &mut egui::Ui, theme: &crate::theme::Theme, text: &str) {
    ui.label(
        egui::RichText::new(text)
            .size(theme.text_xs)
            .color(theme.text_dim)
            .strong(),
    );
    ui.add_space(theme.space_4);
}
