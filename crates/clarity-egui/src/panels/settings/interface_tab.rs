use crate::App;
use crate::theme::Theme;

/// Renders the interface UI.
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

    ui.add_space(theme.space_8);
    // Row 2: Catppuccin + Tokyo Night
    let is_catppuccin = app.settings_store.settings_edit.theme == "catppuccin";
    let is_tokyo = app.settings_store.settings_edit.theme == "tokyo_night";
    ui.horizontal(|ui| {
        ui.set_min_width(ui.available_width());
        let card_w = (ui.available_width() - theme.space_8) / 2.0;
        let card_h = 64.0;

        let c_bg = Theme::catppuccin_mocha().bg;
        let c_text = Theme::catppuccin_mocha().text;
        if crate::widgets::theme_card(
            ui,
            card_w,
            card_h,
            c_bg,
            c_text,
            "Catppuccin",
            "Warm pastel lavender",
            is_catppuccin,
            &theme,
        )
        .clicked()
        {
            set_theme(app, "catppuccin");
        }
        ui.add_space(theme.space_8);

        let t_bg = Theme::tokyo_night().bg;
        let t_text = Theme::tokyo_night().text;
        if crate::widgets::theme_card(
            ui,
            card_w,
            card_h,
            t_bg,
            t_text,
            "Tokyo Night",
            "Deep blue-black",
            is_tokyo,
            &theme,
        )
        .clicked()
        {
            set_theme(app, "tokyo_night");
        }
    });

    ui.add_space(theme.space_8);
    // Row 3: One Dark + OLED
    let is_one_dark = app.settings_store.settings_edit.theme == "one_dark";
    let is_oled = app.settings_store.settings_edit.theme == "oled";
    ui.horizontal(|ui| {
        ui.set_min_width(ui.available_width());
        let card_w = (ui.available_width() - theme.space_8) / 2.0;
        let card_h = 64.0;

        let o_bg = Theme::one_dark().bg;
        let o_text = Theme::one_dark().text;
        if crate::widgets::theme_card(
            ui,
            card_w,
            card_h,
            o_bg,
            o_text,
            "One Dark",
            "Atom classic blue",
            is_one_dark,
            &theme,
        )
        .clicked()
        {
            set_theme(app, "one_dark");
        }
        ui.add_space(theme.space_8);

        let ol_bg = Theme::oled_black().bg;
        let ol_text = Theme::oled_black().text;
        if crate::widgets::theme_card(
            ui,
            card_w,
            card_h,
            ol_bg,
            ol_text,
            "OLED",
            "Pure black pixels-off",
            is_oled,
            &theme,
        )
        .clicked()
        {
            set_theme(app, "oled");
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
    let scales = [
        (0.775, "Compact"),
        (0.85, "Small"),
        (1.0, "Medium"),
        (1.15, "Large"),
    ];
    let current_scale = app
        .settings_store
        .settings_edit
        .font_scale
        .unwrap_or(crate::theme::Theme::DEFAULT_FONT_SCALE);
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
                app.set_font_scale(*val);
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
    let widths = [(520.0, "Narrow"), (600.0, "Medium"), (760.0, "Wide")];
    let current_width = app
        .settings_store
        .settings_edit
        .content_width
        .unwrap_or(600.0);
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

    // ── Layout Debug Overlay ──
    ui.label(
        egui::RichText::new(app.t("Layout Debug"))
            .size(theme.text_sm)
            .color(theme.text)
            .strong(),
    );
    ui.add_space(theme.space_4);
    let mut debug_overlay = app.view_state.debug_layout_overlay;
    if ui
        .checkbox(
            &mut debug_overlay,
            app.t("Show green/blue/red/yellow layout diagnostics (Ctrl+Shift+L)"),
        )
        .changed()
    {
        app.view_state.debug_layout_overlay = debug_overlay;
        app.persist_layout_settings();
    }

    ui.add_space(theme.space_16);

    // ── Pretext PoC Probe ──
    ui.label(
        egui::RichText::new(app.t("Pretext Probe"))
            .size(theme.text_sm)
            .color(theme.text)
            .strong(),
    );
    ui.add_space(theme.space_4);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(app.t("Zoom"))
                .size(theme.text_sm)
                .color(theme.text_muted),
        );
        if ui
            .button(egui::RichText::new("Ctrl + -").size(theme.text_sm))
            .clicked()
        {
            app.decrease_font_scale();
        }
        if ui
            .button(egui::RichText::new("Ctrl + +").size(theme.text_sm))
            .clicked()
        {
            app.increase_font_scale();
        }
    });
    ui.add_space(theme.space_8);
    if ui
        .button(egui::RichText::new("Open Pretext Measurement Probe").size(theme.text_sm))
        .clicked()
    {
        app.ui_store.pretext_probe_open = true;
    }
    let mut pretext_estimate = app.ui_store.pretext_estimate_enabled;
    if ui
        .checkbox(
            &mut pretext_estimate,
            egui::RichText::new("Use pretext for message height estimation (Phase 1 PoC)"),
        )
        .changed()
    {
        app.ui_store.pretext_estimate_enabled = pretext_estimate;
    }

    ui.add_space(theme.space_16);

    // ── Language ──
    ui.label(
        egui::RichText::new(app.t("Language"))
            .size(theme.text_sm)
            .color(theme.text)
            .strong(),
    );
    ui.add_space(theme.space_4);
    // ── Zoom shortcuts hint ──
    ui.label(
        egui::RichText::new(app.t("Use Ctrl + +/- to adjust zoom anytime"))
            .size(theme.text_xs)
            .color(theme.text_dim),
    );
    ui.add_space(theme.space_16);

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
    let scale = app
        .settings_store
        .settings_edit
        .font_scale
        .unwrap_or(crate::theme::Theme::DEFAULT_FONT_SCALE);
    app.ui_store.theme = match name {
        "light" => Theme::light().with_font_scale(scale),
        "catppuccin" => Theme::catppuccin_mocha().with_font_scale(scale),
        "tokyo_night" => Theme::tokyo_night().with_font_scale(scale),
        "one_dark" => Theme::one_dark().with_font_scale(scale),
        "oled" => Theme::oled_black().with_font_scale(scale),
        _ => Theme::dark().with_font_scale(scale),
    };
    app.auto_save_settings();
}
