//! About app — pilot implementation of the `ClarityApp` trait.
//!
//! Mirrors the content previously rendered by
//! `crates/clarity-egui/src/panels/settings/about_tab.rs`, but receives its
//! dependencies through `ClarityAppContext` instead of directly touching the
//! monolithic `App` struct.

use clarity_shell::{ClarityApp, ClarityAppContext, ClarityAppResponse};

/// The about / credits surface.
#[derive(Clone, Copy, Debug, Default)]
pub struct AboutApp;

impl AboutApp {
    /// Create a new about app instance.
    pub fn new() -> Self {
        Self
    }
}

impl ClarityApp for AboutApp {
    fn id(&self) -> &'static str {
        "about"
    }

    fn title(&self, _ctx: &ClarityAppContext<'_>) -> String {
        "About".to_string()
    }

    fn render(
        &mut self,
        ctx: &mut ClarityAppContext<'_>,
        ui: &mut egui::Ui,
        _egui_ctx: &egui::Context,
    ) -> ClarityAppResponse {
        // Borrow the theme immutably for the duration of the render closure.
        let theme = &*ctx.theme;

        ui.vertical_centered(|ui| {
            ui.add_space(24.0);

            // ── Logo placeholder ──
            let logo_size = 64.0;
            let (logo_rect, _resp) =
                ui.allocate_exact_size(egui::vec2(logo_size, logo_size), egui::Sense::hover());
            ui.painter().rect_filled(
                logo_rect,
                egui::CornerRadius::same(theme.radius_md as u8),
                theme.surface,
            );
            ui.painter().text(
                logo_rect.center(),
                egui::Align2::CENTER_CENTER,
                "◈",
                egui::FontId::new(32.0, egui::FontFamily::Proportional),
                theme.accent,
            );

            ui.add_space(16.0);

            // ── Name & tagline ──
            ui.label(
                egui::RichText::new(ctx.app_name)
                    .size(theme.text_2xl)
                    .strong()
                    .color(theme.text),
            );
            ui.label(
                egui::RichText::new(ctx.app_description)
                    .size(theme.text_base)
                    .color(theme.text_muted),
            );

            ui.add_space(8.0);

            // ── Version ──
            ui.label(
                egui::RichText::new(format!("v{}", ctx.app_version))
                    .size(theme.text_sm)
                    .color(theme.text_dim),
            );

            ui.add_space(24.0);

            // ── Info rows ──
            let row_h = 36.0;
            let max_w = 420.0;

            info_row(ui, max_w, row_h, theme, "License", ctx.app_license);
            info_row(
                ui,
                max_w,
                row_h,
                theme,
                "Copyright",
                &format!("© 2026 {}", env!("CARGO_PKG_AUTHORS")),
            );

            ui.add_space(24.0);

            // ── License text ──
            ui.label(
                egui::RichText::new(LICENSE_SHORT)
                    .size(theme.text_xs)
                    .color(theme.text_dim)
                    .monospace(),
            );
        });

        ClarityAppResponse::None
    }
}

fn info_row(
    ui: &mut egui::Ui,
    max_w: f32,
    row_h: f32,
    theme: &clarity_ui::theme::Theme,
    label: &str,
    value: &str,
) {
    ui.allocate_ui_with_layout(
        egui::vec2(max_w, row_h),
        egui::Layout::left_to_right(egui::Align::Center),
        |ui| {
            ui.label(
                egui::RichText::new(label)
                    .size(theme.text_sm)
                    .color(theme.text_dim)
                    .strong(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(value)
                        .size(theme.text_sm)
                        .color(theme.text),
                );
            });
        },
    );
}

const LICENSE_SHORT: &str = r#"MIT License

Copyright (c) 2026 juice094 and contributors

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software."#;

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_shell::ClarityApp;
    use clarity_ui::theme::Theme;

    struct EmptyState;

    impl clarity_shell::AppState for EmptyState {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
        fn theme(&self) -> &clarity_ui::theme::Theme {
            panic!("EmptyState has no theme")
        }
        fn theme_mut(&mut self) -> &mut clarity_ui::theme::Theme {
            panic!("EmptyState has no theme")
        }
    }

    fn test_context<'a>(theme: &'a mut Theme, state: &'a mut EmptyState) -> ClarityAppContext<'a> {
        ClarityAppContext {
            theme,
            app_name: "Clarity",
            app_version: "0.0.0",
            app_description: "Test description",
            app_license: "AGPL-3.0-or-later",
            state,
        }
    }

    #[test]
    fn about_app_renders_without_panic() {
        let mut theme = Theme::dark();
        let mut state = EmptyState;
        let mut ctx = test_context(&mut theme, &mut state);
        let mut app = AboutApp::new();
        let egui_ctx = egui::Context::default();

        let _output = egui_ctx.run_ui(egui::RawInput::default(), |egui_ctx| {
            egui::Area::new("about_test".into()).show(egui_ctx, |ui| {
                let response = app.render(&mut ctx, ui, egui_ctx);
                assert_eq!(response, ClarityAppResponse::None);
            });
        });
    }

    #[test]
    fn about_app_id_and_title() {
        let mut theme = Theme::dark();
        let mut state = EmptyState;
        let ctx = test_context(&mut theme, &mut state);
        let app = AboutApp::new();

        assert_eq!(app.id(), "about");
        assert_eq!(app.title(&ctx), "About");
    }

    #[test]
    fn about_app_default_methods() {
        let app = AboutApp::new();
        assert_eq!(app.tab_notifications(), 0);
    }
}
