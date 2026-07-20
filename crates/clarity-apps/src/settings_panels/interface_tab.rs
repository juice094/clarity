use crate::settings::SettingsStore;
use clarity_shell::AppState;
use clarity_ui::design_system::{self, Space, TextStyle};
use clarity_ui::i18n::Locale;
use clarity_ui::theme::Theme;

/// Renders the interface UI.
pub fn render_interface(store: &mut SettingsStore, state: &mut dyn AppState, ui: &mut egui::Ui) {
    let theme = state.theme().clone();

    design_system::text(ui, state.t("Interface"), TextStyle::Subheading);
    design_system::gap(ui, Space::S3);

    // ── Theme cards ──
    design_system::text(ui, state.t("Theme"), TextStyle::CaptionStrong);
    design_system::gap(ui, Space::S1);

    let is_dark = store.settings_edit.theme == "dark";
    let is_light = store.settings_edit.theme == "light";

    ui.horizontal(|ui| {
        ui.set_min_width(ui.available_width());
        let card_w = (ui.available_width() - theme.space_8) / 2.0;
        let card_h = 64.0;

        // Dark card
        let dark_bg = Theme::dark().bg;
        let dark_text = Theme::dark().text;
        if clarity_ui::widgets::theme_card(
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
            set_theme(store, state, "dark");
        }
        design_system::gap(ui, Space::S1);

        // Light card
        let light_bg = Theme::light().bg;
        let light_text = Theme::light().text;
        if clarity_ui::widgets::theme_card(
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
            set_theme(store, state, "light");
        }
    });

    design_system::gap(ui, Space::S1);
    // Row 2: Catppuccin + Tokyo Night
    let is_catppuccin = store.settings_edit.theme == "catppuccin";
    let is_tokyo = store.settings_edit.theme == "tokyo_night";
    ui.horizontal(|ui| {
        ui.set_min_width(ui.available_width());
        let card_w = (ui.available_width() - theme.space_8) / 2.0;
        let card_h = 64.0;

        let c_bg = Theme::catppuccin_mocha().bg;
        let c_text = Theme::catppuccin_mocha().text;
        if clarity_ui::widgets::theme_card(
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
            set_theme(store, state, "catppuccin");
        }
        design_system::gap(ui, Space::S1);

        let t_bg = Theme::tokyo_night().bg;
        let t_text = Theme::tokyo_night().text;
        if clarity_ui::widgets::theme_card(
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
            set_theme(store, state, "tokyo_night");
        }
    });

    design_system::gap(ui, Space::S1);
    // Row 3: One Dark + OLED
    let is_one_dark = store.settings_edit.theme == "one_dark";
    let is_oled = store.settings_edit.theme == "oled";
    ui.horizontal(|ui| {
        ui.set_min_width(ui.available_width());
        let card_w = (ui.available_width() - theme.space_8) / 2.0;
        let card_h = 64.0;

        let o_bg = Theme::one_dark().bg;
        let o_text = Theme::one_dark().text;
        if clarity_ui::widgets::theme_card(
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
            set_theme(store, state, "one_dark");
        }
        design_system::gap(ui, Space::S1);

        let ol_bg = Theme::oled_black().bg;
        let ol_text = Theme::oled_black().text;
        if clarity_ui::widgets::theme_card(
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
            set_theme(store, state, "oled");
        }
    });
    design_system::gap(ui, Space::S4);

    // ── Font Size ──
    design_system::text(ui, state.t("Font Size"), TextStyle::CaptionStrong);
    design_system::gap(ui, Space::S0);
    let current_scale = store
        .settings_edit
        .font_scale
        .unwrap_or(Theme::DEFAULT_FONT_SCALE);
    // Snap to the nearest labelled step so toggle_group highlights correctly.
    let steps = [0.775_f32, 0.85, 1.0, 1.15];
    let nearest = steps
        .iter()
        .min_by(|a, b| {
            (current_scale - **a)
                .abs()
                .partial_cmp(&(current_scale - **b).abs())
                // SAFE: font-scale steps and current value are finite non-NaN f32s.
                .unwrap_or(std::cmp::Ordering::Equal)
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
        state.set_font_scale(scale);
    }
    design_system::gap(ui, Space::S3);

    // ── Content Width ──
    design_system::text(ui, state.t("Content Width"), TextStyle::CaptionStrong);
    design_system::gap(ui, Space::S0);
    let current_width = store.settings_edit.content_width.unwrap_or(600.0);
    if let Some(width) = design_system::toggle_group(
        ui,
        &[("Narrow", 520.0), ("Medium", 600.0), ("Wide", 760.0)],
        current_width,
    ) {
        store.settings_edit.content_width = Some(width);
        state.set_content_max_width(width);
        state.auto_save_settings();
    }
    design_system::gap(ui, Space::S3);

    // ── Layout Debug Overlay ──
    design_system::text(ui, state.t("Layout Debug"), TextStyle::CaptionStrong);
    design_system::gap(ui, Space::S0);
    let mut debug_overlay = state.debug_layout_overlay();
    if ui
        .checkbox(
            &mut debug_overlay,
            state.t("Show green/blue/red/yellow layout diagnostics (Ctrl+Shift+L)"),
        )
        .changed()
    {
        state.set_debug_layout_overlay(debug_overlay);
        state.persist_layout_settings();
    }

    design_system::gap(ui, Space::S3);

    // ── Pretext PoC Probe ──
    design_system::text(ui, state.t("Pretext Probe"), TextStyle::CaptionStrong);
    design_system::gap(ui, Space::S0);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(state.t("Zoom"))
                .size(theme.text_sm)
                .color(theme.text_muted),
        );
        if ui
            .button(egui::RichText::new("Ctrl + -").size(theme.text_sm))
            .clicked()
        {
            state.decrease_font_scale();
        }
        if ui
            .button(egui::RichText::new("Ctrl + +").size(theme.text_sm))
            .clicked()
        {
            state.increase_font_scale();
        }
    });
    design_system::gap(ui, Space::S1);
    if ui
        .button(egui::RichText::new("Open Pretext Measurement Probe").size(theme.text_sm))
        .clicked()
    {
        state.set_pretext_probe_open(true);
    }
    let mut pretext_estimate = state.pretext_estimate_enabled();
    if ui
        .checkbox(
            &mut pretext_estimate,
            egui::RichText::new("Use pretext for message height estimation (Phase 1 PoC)"),
        )
        .changed()
    {
        state.set_pretext_estimate_enabled(pretext_estimate);
    }

    design_system::gap(ui, Space::S3);

    // ── Language ──
    design_system::text(ui, state.t("Language"), TextStyle::CaptionStrong);
    design_system::gap(ui, Space::S0);
    // ── Zoom shortcuts hint ──
    design_system::text(
        ui,
        state.t("Use Ctrl + +/- to adjust zoom anytime"),
        TextStyle::Small,
    );
    design_system::gap(ui, Space::S3);

    if let Some(locale) = design_system::toggle_group(
        ui,
        &[("English", Locale::EnUS), ("中文", Locale::ZhCN)],
        state.locale(),
    ) {
        state.set_locale(locale);
        store.settings_edit.language = Some(locale.as_code().to_string());
        state.auto_save_settings();
    }
}

fn set_theme(store: &mut SettingsStore, state: &mut dyn AppState, name: &str) {
    store.settings_edit.theme = name.to_string();
    let scale = store
        .settings_edit
        .font_scale
        .unwrap_or(Theme::DEFAULT_FONT_SCALE);
    state.set_theme(match name {
        "light" => Theme::light().with_font_scale(scale),
        "catppuccin" => Theme::catppuccin_mocha().with_font_scale(scale),
        "tokyo_night" => Theme::tokyo_night().with_font_scale(scale),
        "one_dark" => Theme::one_dark().with_font_scale(scale),
        "oled" => Theme::oled_black().with_font_scale(scale),
        _ => Theme::dark().with_font_scale(scale),
    });
    state.auto_save_settings();
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_shell::AppState;
    use clarity_ui::theme::Theme;

    struct TestState {
        theme: Theme,
        locale: Locale,
        save_called: bool,
    }

    impl AppState for TestState {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
        fn theme(&self) -> &Theme {
            &self.theme
        }
        fn theme_mut(&mut self) -> &mut Theme {
            &mut self.theme
        }
        fn locale(&self) -> Locale {
            self.locale
        }
        fn set_locale(&mut self, locale: Locale) {
            self.locale = locale;
        }
        fn auto_save_settings(&mut self) {
            self.save_called = true;
        }
    }

    #[test]
    fn locale_change_is_persisted_to_settings() {
        let mut store = SettingsStore::default();
        let mut state = TestState {
            theme: Theme::dark(),
            locale: Locale::EnUS,
            save_called: false,
        };

        // Simulate the body of the locale toggle_group handler: when the user
        // selects a different locale, it must be written to settings and saved.
        let locale = Locale::ZhCN;
        state.set_locale(locale);
        store.settings_edit.language = Some(locale.as_code().to_string());
        state.auto_save_settings();

        assert_eq!(state.locale, Locale::ZhCN);
        assert_eq!(store.settings_edit.language.as_deref(), Some("zh"));
        assert!(state.save_called, "locale change must trigger auto-save");
    }
}
