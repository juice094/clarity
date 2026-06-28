use crate::App;
use crate::design_system::{self, Space, TextStyle};
use crate::theme::Theme;

/// Renders the interface UI.
pub fn render_interface(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    design_system::text(ui, app.t("Interface"), TextStyle::Subheading);
    design_system::gap(ui, Space::S3);

    // ── Theme cards ──
    design_system::text(ui, app.t("Theme"), TextStyle::CaptionStrong);
    design_system::gap(ui, Space::S1);

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
        design_system::gap(ui, Space::S1);

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

    design_system::gap(ui, Space::S1);
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
        design_system::gap(ui, Space::S1);

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

    design_system::gap(ui, Space::S1);
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
        design_system::gap(ui, Space::S1);

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
    design_system::gap(ui, Space::S4);

    // ── Font Size ──
    design_system::text(ui, app.t("Font Size"), TextStyle::CaptionStrong);
    design_system::gap(ui, Space::S0);
    let current_scale = app
        .settings_store
        .settings_edit
        .font_scale
        .unwrap_or(crate::theme::Theme::DEFAULT_FONT_SCALE);
    // Snap to the nearest labelled step so toggle_group highlights correctly.
    let steps = [0.775_f32, 0.85, 1.0, 1.15];
    let nearest = steps
        .iter()
        .min_by(|a, b| {
            (current_scale - **a)
                .abs()
                .partial_cmp(&(current_scale - **b).abs())
                .unwrap()
        })
        .copied()
        .unwrap_or(0.85);
    if let Some(scale) = design_system::toggle_group(
        ui,
        &[
            ("Compact", 0.775),
            ("Small", 0.85),
            ("Medium", 1.0),
            ("Large", 1.15),
        ],
        nearest,
    ) {
        app.set_font_scale(scale);
    }
    design_system::gap(ui, Space::S3);

    // ── Content Width ──
    design_system::text(ui, app.t("Content Width"), TextStyle::CaptionStrong);
    design_system::gap(ui, Space::S0);
    let current_width = app
        .settings_store
        .settings_edit
        .content_width
        .unwrap_or(600.0);
    if let Some(width) = design_system::toggle_group(
        ui,
        &[("Narrow", 520.0), ("Medium", 600.0), ("Wide", 760.0)],
        current_width,
    ) {
        app.settings_store.settings_edit.content_width = Some(width);
        app.ui_store.content_max_width = width;
        app.auto_save_settings();
    }
    design_system::gap(ui, Space::S3);

    // ── Layout Debug Overlay ──
    design_system::text(ui, app.t("Layout Debug"), TextStyle::CaptionStrong);
    design_system::gap(ui, Space::S0);
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

    design_system::gap(ui, Space::S3);

    // ── Pretext PoC Probe ──
    design_system::text(ui, app.t("Pretext Probe"), TextStyle::CaptionStrong);
    design_system::gap(ui, Space::S0);
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
    design_system::gap(ui, Space::S1);
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

    design_system::gap(ui, Space::S3);

    // ── Language ──
    design_system::text(ui, app.t("Language"), TextStyle::CaptionStrong);
    design_system::gap(ui, Space::S0);
    // ── Zoom shortcuts hint ──
    design_system::text(
        ui,
        app.t("Use Ctrl + +/- to adjust zoom anytime"),
        TextStyle::Small,
    );
    design_system::gap(ui, Space::S3);

    if let Some(locale) = design_system::toggle_group(
        ui,
        &[
            ("English", crate::i18n::Locale::EnUS),
            ("中文", crate::i18n::Locale::ZhCN),
        ],
        app.ui_store.locale,
    ) {
        app.ui_store.locale = locale;
    }
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
