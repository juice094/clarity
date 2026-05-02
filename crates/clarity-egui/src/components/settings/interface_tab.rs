use crate::App;

pub fn render_interface(app: &mut App, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new(app.t("Interface")).color(app.ui_store.theme.text).size(app.ui_store.theme.text_lg).strong());
    ui.add_space(16.0);

    // ── Theme ──
    ui.label(egui::RichText::new(app.t("Theme")).size(app.ui_store.theme.text_sm).color(app.ui_store.theme.text).strong());
    let themes = ["dark","light"];
    let ct = themes.iter().position(|t| *t == app.settings_store.settings_edit.theme).unwrap_or(0);
    let mut ts = ct;
    egui::ComboBox::from_id_salt("st_theme").selected_text(&app.settings_store.settings_edit.theme)
        .show_ui(ui, |ui| { for (i,t) in themes.iter().enumerate() { ui.selectable_value(&mut ts, i, *t); }});
    if ts != ct { app.settings_store.settings_edit.theme = themes[ts].to_string();
        let scale = app.settings_store.settings_edit.font_scale.unwrap_or(1.0);
        app.ui_store.theme = if app.settings_store.settings_edit.theme == "light" { crate::theme::Theme::light().with_font_scale(scale) } else { crate::theme::Theme::dark().with_font_scale(scale) };
        app.auto_save_settings(); }
    ui.add_space(12.0);

    // ── Font Scale ──
    ui.label(egui::RichText::new(app.t("Font Size")).size(app.ui_store.theme.text_sm).color(app.ui_store.theme.text).strong());
    let scales = [(0.9, "Small"), (1.0, "Medium"), (1.15, "Large")];
    let current_scale = app.settings_store.settings_edit.font_scale.unwrap_or(1.0);
    ui.horizontal(|ui| {
        for (val, label) in &scales {
            let active = (current_scale - val).abs() < 0.01;
            if ui.add(egui::Button::new(egui::RichText::new(*label).size(app.ui_store.theme.text_sm))
                .fill(if active { app.ui_store.theme.accent } else { app.ui_store.theme.surface })
                .corner_radius(app.ui_store.theme.radius_sm as u8)).clicked() {
                app.settings_store.settings_edit.font_scale = Some(*val);
                let theme_name = app.settings_store.settings_edit.theme.clone();
                app.ui_store.theme = if theme_name == "light" { crate::theme::Theme::light().with_font_scale(*val) } else { crate::theme::Theme::dark().with_font_scale(*val) };
                app.auto_save_settings();
            }
        }
    });
    ui.add_space(12.0);

    // ── Content Width ──
    ui.label(egui::RichText::new(app.t("Content Width")).size(app.ui_store.theme.text_sm).color(app.ui_store.theme.text).strong());
    let widths = [(600.0, "Narrow"), (720.0, "Medium"), (900.0, "Wide")];
    let current_width = app.settings_store.settings_edit.content_width.unwrap_or(720.0);
    ui.horizontal(|ui| {
        for (val, label) in &widths {
            let active = (current_width - val).abs() < 1.0;
            if ui.add(egui::Button::new(egui::RichText::new(*label).size(app.ui_store.theme.text_sm))
                .fill(if active { app.ui_store.theme.accent } else { app.ui_store.theme.surface })
                .corner_radius(app.ui_store.theme.radius_sm as u8)).clicked() {
                app.settings_store.settings_edit.content_width = Some(*val);
                app.ui_store.content_max_width = *val;
                app.auto_save_settings();
            }
        }
    });
    ui.add_space(12.0);

    // ── Language ──
    ui.label(egui::RichText::new(app.t("Language")).size(app.ui_store.theme.text_sm).color(app.ui_store.theme.text).strong());
    ui.horizontal(|ui| {
        let en = matches!(app.ui_store.locale, crate::i18n::Locale::EnUS);
        let zh = matches!(app.ui_store.locale, crate::i18n::Locale::ZhCN);
        if ui.add(egui::Button::new(egui::RichText::new("English").size(app.ui_store.theme.text_sm))
            .fill(if en { app.ui_store.theme.accent } else { app.ui_store.theme.surface })
            .corner_radius(app.ui_store.theme.radius_sm as u8)).clicked() { app.ui_store.locale = crate::i18n::Locale::EnUS; }
        if ui.add(egui::Button::new(egui::RichText::new("Simplified Chinese").size(app.ui_store.theme.text_sm))
            .fill(if zh { app.ui_store.theme.accent } else { app.ui_store.theme.surface })
            .corner_radius(app.ui_store.theme.radius_sm as u8)).clicked() { app.ui_store.locale = crate::i18n::Locale::ZhCN; }
    });
}
