use crate::App;

pub fn render_about(app: &mut App, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(24.0);

        // ── Logo placeholder ──
        let theme = &app.ui_store.theme;
        let logo_size = 64.0;
        let (logo_rect, _resp) = ui.allocate_exact_size(egui::vec2(logo_size, logo_size), egui::Sense::hover());
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
        ui.label(egui::RichText::new("Clarity").size(theme.text_2xl).strong().color(theme.text));
        ui.label(
            egui::RichText::new("Local-first AI agent runtime")
                .size(theme.text_base)
                .color(theme.text_muted),
        );

        ui.add_space(8.0);

        // ── Version ──
        ui.label(
            egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION")))
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        ui.label(
            egui::RichText::new("egui 0.31 · glow · Rust")
                .size(theme.text_sm)
                .color(theme.text_dim),
        );

        ui.add_space(24.0);

        // ── Info rows ──
        let row_h = 36.0;
        let max_w = 420.0;

        ui.allocate_ui_with_layout(egui::vec2(max_w, row_h), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new("License").size(theme.text_sm).color(theme.text_dim).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("MIT").size(theme.text_sm).color(theme.text));
            });
        });

        ui.allocate_ui_with_layout(egui::vec2(max_w, row_h), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new("Copyright").size(theme.text_sm).color(theme.text_dim).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(egui::RichText::new("© 2026 juice094 and contributors").size(theme.text_sm).color(theme.text));
            });
        });

        ui.allocate_ui_with_layout(egui::vec2(max_w, row_h), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new("GitHub").size(theme.text_sm).color(theme.text_dim).strong());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.hyperlink_to(
                    egui::RichText::new("github.com/juice094/clarity ↗")
                        .size(theme.text_sm)
                        .color(theme.accent),
                    "https://github.com/juice094/clarity",
                );
            });
        });

        ui.add_space(24.0);

        // ── License text ──
        ui.label(
            egui::RichText::new(LICENSE_SHORT)
                .size(theme.text_xs)
                .color(theme.text_dim)
                .monospace(),
        );
    });
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
