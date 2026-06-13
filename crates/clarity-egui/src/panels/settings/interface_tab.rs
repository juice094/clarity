use crate::theme::Theme;
use crate::App;

pub fn render_interface(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    ui.label(
        egui::RichText::new(app.t("Interface"))
            .color(theme.text)
            .size(theme.text_lg)
            .strong(),
    );
    ui.add_space(theme.space_16);

    // ── Theme cards ──
    ui.label(
        egui::RichText::new(app.t("Theme"))
            .size(theme.text_sm)
            .color(theme.text)
            .strong(),
    );
    ui.add_space(theme.space_8);

    let is_dark = app.settings_store.settings_edit.theme == "dark";
    let is_light = app.settings_store.settings_edit.theme == "light";

    ui.horizontal(|ui| {
        ui.set_min_width(ui.available_width());
        let card_w = (ui.available_width() - theme.space_8) / 2.0;
        let card_h = 64.0;

        // Dark card
        let dark_bg = Theme::dark().bg;
        let dark_text = Theme::dark().text;
        if crate::widgets::theme_card(
            ui,
            card_w,
            card_h,
            dark_bg,
            dark_text,
            "Dark",
            "Deep black canvas",
            is_dark,
            &theme,
        )
        .clicked()
        {
            set_theme(app, "dark");
        }
        ui.add_space(theme.space_8);

        // Light card
        let light_bg = Theme::light().bg;
        let light_text = Theme::light().text;
        if crate::widgets::theme_card(
            ui,
            card_w,
            card_h,
            light_bg,
            light_text,
            "Light",
            "Cool off-white",
            is_light,
            &theme,
        )
        .clicked()
        {
            set_theme(app, "light");
        }
    });
    ui.add_space(theme.space_20);

    // ── Font Size ──
    ui.label(
        egui::RichText::new(app.t("Font Size"))
            .size(theme.text_sm)
            .color(theme.text)
            .strong(),
    );
    ui.add_space(theme.space_4);
    let scales = [(0.9, "Small"), (1.0, "Medium"), (1.15, "Large")];
    let current_scale = app.settings_store.settings_edit.font_scale.unwrap_or(1.0);
    ui.horizontal(|ui| {
        for (val, label) in &scales {
            let active = (current_scale - val).abs() < 0.01;
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(*label).size(theme.text_sm))
                        .fill(if active { theme.accent } else { theme.surface })
                        .stroke(if active {
                            egui::Stroke::NONE
                        } else {
                            egui::Stroke::new(1.0_f32, theme.border)
                        })
                        .corner_radius(theme.radius_sm as u8),
                )
                .clicked()
            {
                app.settings_store.settings_edit.font_scale = Some(*val);
                let theme_name = app.settings_store.settings_edit.theme.clone();
                app.ui_store.theme = if theme_name == "light" {
                    Theme::light().with_font_scale(*val)
                } else {
                    Theme::dark().with_font_scale(*val)
                };
                app.auto_save_settings();
            }
        }
    });
    ui.add_space(theme.space_16);

    // ── Content Width ──
    ui.label(
        egui::RichText::new(app.t("Content Width"))
            .size(theme.text_sm)
            .color(theme.text)
            .strong(),
    );
    ui.add_space(theme.space_4);
    let widths = [(600.0, "Narrow"), (720.0, "Medium"), (900.0, "Wide")];
    let current_width = app
        .settings_store
        .settings_edit
        .content_width
        .unwrap_or(720.0);
    ui.horizontal(|ui| {
        for (val, label) in &widths {
            let active = (current_width - val).abs() < 1.0;
            if ui
                .add(
                    egui::Button::new(egui::RichText::new(*label).size(theme.text_sm))
                        .fill(if active { theme.accent } else { theme.surface })
                        .stroke(if active {
                            egui::Stroke::NONE
                        } else {
                            egui::Stroke::new(1.0_f32, theme.border)
                        })
                        .corner_radius(theme.radius_sm as u8),
                )
                .clicked()
            {
                app.settings_store.settings_edit.content_width = Some(*val);
                app.ui_store.content_max_width = *val;
                app.auto_save_settings();
            }
        }
    });
    ui.add_space(theme.space_16);

    // ── Language ──
    ui.label(
        egui::RichText::new(app.t("Language"))
            .size(theme.text_sm)
            .color(theme.text)
            .strong(),
    );
    ui.add_space(theme.space_4);
    ui.horizontal(|ui| {
        let en = matches!(app.ui_store.locale, crate::i18n::Locale::EnUS);
        let zh = matches!(app.ui_store.locale, crate::i18n::Locale::ZhCN);
        if ui
            .add(
                egui::Button::new(egui::RichText::new("English").size(theme.text_sm))
                    .fill(if en { theme.accent } else { theme.surface })
                    .stroke(if en {
                        egui::Stroke::NONE
                    } else {
                        egui::Stroke::new(1.0_f32, theme.border)
                    })
                    .corner_radius(theme.radius_sm as u8),
            )
            .clicked()
        {
            app.ui_store.locale = crate::i18n::Locale::EnUS;
        }
        if ui
            .add(
                egui::Button::new(egui::RichText::new("Simplified Chinese").size(theme.text_sm))
                    .fill(if zh { theme.accent } else { theme.surface })
                    .stroke(if zh {
                        egui::Stroke::NONE
                    } else {
                        egui::Stroke::new(1.0_f32, theme.border)
                    })
                    .corner_radius(theme.radius_sm as u8),
            )
            .clicked()
        {
            app.ui_store.locale = crate::i18n::Locale::ZhCN;
        }
    });
}

fn set_theme(app: &mut App, name: &str) {
    app.settings_store.settings_edit.theme = name.to_string();
    let scale = app.settings_store.settings_edit.font_scale.unwrap_or(1.0);
    app.ui_store.theme = if name == "light" {
        Theme::light().with_font_scale(scale)
    } else {
        Theme::dark().with_font_scale(scale)
    };
    app.auto_save_settings();
}
