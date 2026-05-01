use crate::provider::{ApiFormat, ProviderDefinition, ProviderRegistry};
use crate::ui::types::ToastLevel;
use crate::App;

#[derive(Clone, Copy, Debug, PartialEq)]
enum SettingsTab { Provider, Interface, About }

pub fn render_settings_panel(app: &mut App, ctx: &egui::Context) {
    if !app.settings_open { return; }

    let screen = ctx.screen_rect();

    // ── Dimmer + outside-click-to-close ──
    ctx.layer_painter(egui::LayerId::background()).rect_filled(
        screen, egui::CornerRadius::same(0), app.theme.overlay);

    // Click outside the settings window → close
    let mut close_requested = false;
    egui::Area::new("settings_scrim".into())
        .interactable(true)
        .order(egui::Order::Background)
        .show(ctx, |ui| {
            ui.set_min_size(screen.size());
            if ui.allocate_response(screen.size(), egui::Sense::click()).clicked()
                || ctx.input(|i| i.key_pressed(egui::Key::Escape))
            {
                close_requested = true;
            }
        });

    let tabs = [
        (SettingsTab::Provider, app.t("Provider")),
        (SettingsTab::Interface, app.t("Interface")),
        (SettingsTab::About, app.t("About")),
    ];
    let mut at = app.settings_active_tab;

    egui::Window::new(app.t("Settings"))
        .collapsible(false).resizable(false)
        .fixed_size(egui::vec2(560.0, 460.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .frame(egui::Frame::window(&ctx.style())
            .fill(app.theme.surface)
            .corner_radius(egui::CornerRadius::same(app.theme.radius_lg as u8))
            .inner_margin(egui::Margin::same(0)))
        .show(ctx, |ui| {
            // ── Tab bar ──
            egui::Frame::new().fill(app.theme.bg_accent)
                .inner_margin(egui::Margin::symmetric(8, 0))
                .corner_radius(egui::CornerRadius{nw: app.theme.radius_lg as u8, ne: app.theme.radius_lg as u8, sw: 0, se: 0})
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.set_min_height(34.0);
                        for (i, (_t, name)) in tabs.iter().enumerate() {
                            let is = i as u8 == at;
                            let bg = if is { app.theme.surface } else { egui::Color32::TRANSPARENT };
                            let tc = if is { app.theme.text } else { app.theme.text_muted };
                            if ui.add(egui::Button::new(egui::RichText::new(*name).size(13.0).color(tc))
                                .fill(bg).corner_radius(app.theme.radius_sm as u8)
                                .min_size(egui::vec2(90.0, 28.0))).clicked() { at = i as u8; }
                        }
                    });
                });

            // ── Content — fixed height ──
            egui::Frame::new().inner_margin(egui::Margin::symmetric(16, 12))
                .show(ui, |ui| {
                    ui.set_min_height(350.0);
                    match tabs[at as usize].0 {
                        SettingsTab::Provider => render_provider(app, ui),
                        SettingsTab::Interface => render_interface(app, ui),
                        SettingsTab::About => render_about(app, ui),
                    }
                });
        });

    app.settings_active_tab = at;
    if close_requested { app.settings_open = false; }
}

// ============================================================================
// Provider — cards with status dot, URL, format badge
// ============================================================================

fn render_provider(app: &mut App, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new(app.t("Provider")).color(app.theme.text).size(15.0).strong());
    ui.add_space(4.0);
    ui.label(egui::RichText::new("Connect to an AI service").size(11.0).color(app.theme.text_dim));
    ui.add_space(12.0);

    let all: Vec<ProviderDefinition> = app.provider_registry.list().into_iter().cloned().collect();
    let current = app.settings_edit.provider.clone();

    egui::ScrollArea::vertical().max_height(240.0).show(ui, |ui| {
        for p in &all {
            let is_active = p.id == current;
            let id = p.id.clone();
            let h = 48.0;
            let s = egui::Stroke::new(if is_active { 1.5 } else { 1.0 },
                if is_active { app.theme.accent } else { app.theme.border });

            let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), h), egui::Sense::click());
            let bg = if resp.hovered() { app.theme.surface_strong } else { app.theme.surface };
            ui.painter().rect_filled(rect, egui::CornerRadius::same(app.theme.radius_md as u8), bg);
            ui.painter().rect_stroke(rect, egui::CornerRadius::same(app.theme.radius_md as u8), s, egui::StrokeKind::Outside);

            let cx = rect.left() + 12.0;
            let cy = rect.center().y;

            // Status dot
            let has_key = !p.api_key_ref.is_empty();
            let dot = if has_key { app.theme.status_online } else { app.theme.text_dim };
            ui.painter().circle_filled(egui::pos2(cx + 4.0, cy), 4.0, dot);

            // Name
            ui.painter().text(egui::pos2(cx + 16.0, cy - 8.0), egui::Align2::LEFT_CENTER,
                p.display(), egui::FontId::new(13.0, egui::FontFamily::Proportional),
                if is_active { app.theme.accent } else { app.theme.text });

            // URL
            let url = if p.base_url.len() > 40 { format!("{}...", &p.base_url[..37]) } else { p.base_url.clone() };
            ui.painter().text(egui::pos2(cx + 16.0, cy + 8.0), egui::Align2::LEFT_CENTER,
                &url, egui::FontId::new(10.0, egui::FontFamily::Monospace), app.theme.text_dim);

            // API badge
            let badge = p.api_format.as_str();
            let bw = badge.len() as f32 * 7.0 + 14.0;
            let br = egui::Rect::from_min_size(egui::pos2(rect.right() - bw - 10.0, cy - 9.0), egui::vec2(bw, 18.0));
            ui.painter().rect_filled(br, egui::CornerRadius::same(4), app.theme.bg_hover);
            ui.painter().text(br.center(), egui::Align2::CENTER_CENTER, badge, egui::FontId::new(9.0, egui::FontFamily::Monospace), app.theme.text_dim);

            // Active badge
            if is_active {
                ui.painter().text(egui::pos2(rect.right() - 10.0, cy + 8.0), egui::Align2::RIGHT_CENTER,
                    "Active", egui::FontId::new(10.0, egui::FontFamily::Proportional), app.theme.ok);
            }

            if resp.clicked() && !is_active {
                app.settings_edit.provider = id.clone();
                if let Some(prov) = app.provider_registry.get(&id) {
                    if !prov.models.is_empty() { app.settings_edit.model = prov.models[0].clone(); }
                }
                app.auto_save_settings();
            }
            ui.add_space(2.0);
        }
    });

    ui.add_space(12.0);

    // ── Model for active provider ──
    if let Some(prov) = app.provider_registry.get(&current) {
        if !prov.models.is_empty() {
            ui.label(egui::RichText::new(app.t("Model")).size(12.0).color(app.theme.text).strong());
            let mut models = prov.models.clone();
            if !models.contains(&app.settings_edit.model) { models.push(app.settings_edit.model.clone()); }
            let cur = models.iter().position(|m| *m == app.settings_edit.model).unwrap_or(0);
            let mut sel = cur;
            egui::ComboBox::from_id_salt("st_model").selected_text(&app.settings_edit.model)
                .show_ui(ui, |ui| { for (i, m) in models.iter().enumerate() { ui.selectable_value(&mut sel, i, m.as_str()); }});
            if sel != cur && sel < models.len() { app.settings_edit.model = models[sel].clone(); app.auto_save_settings(); }
        }
    }

    ui.add_space(app.theme.space_8);

    // ── Approval mode ──
    ui.label(egui::RichText::new(app.t("Approval Mode")).size(12.0).color(app.theme.text).strong());
    let modes = ["interactive","smart","plan","yolo"];
    let cur = modes.iter().position(|m| *m == app.settings_edit.approval_mode).unwrap_or(0);
    let mut ms = cur;
    egui::ComboBox::from_id_salt("st_amode").selected_text(&app.settings_edit.approval_mode)
        .show_ui(ui, |ui| { for (i, m) in modes.iter().enumerate() { ui.selectable_value(&mut ms, i, *m); }});
    if ms != cur { app.settings_edit.approval_mode = modes[ms].to_string(); app.auto_save_settings(); }

    ui.add_space(12.0);

    // ── Add custom ──
    if ui.add(app.theme.primary_button("+ Add Custom")).clicked() { app.show_add_provider = !app.show_add_provider; }
    if app.show_add_provider { ui.add_space(8.0); render_add_form(app, ui); }
}

fn render_add_form(app: &mut App, ui: &mut egui::Ui) {
    egui::Frame::new().fill(app.theme.bg_accent)
        .corner_radius(app.theme.radius_md as u8).stroke(egui::Stroke::new(1.0, app.theme.border))
        .inner_margin(egui::Margin::same(12)).show(ui, |ui| {
        ui.label(egui::RichText::new("Add Custom Provider").strong().color(app.theme.text).size(13.0));
        ui.add_space(8.0);
        ui.label(egui::RichText::new("Name").size(11.0).color(app.theme.text));
        ui.add(egui::TextEdit::singleline(&mut app.add_provider_name).hint_text("my-provider").desired_width(240.0));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("Base URL").size(11.0).color(app.theme.text));
        ui.add(egui::TextEdit::singleline(&mut app.add_provider_url).hint_text("https://...").desired_width(240.0));
        ui.add_space(4.0);
        ui.label(egui::RichText::new("API Format").size(11.0).color(app.theme.text));
        let fmts = ["openai-completions","anthropic-messages"];
        let mut fi = fmts.iter().position(|f| *f == app.add_provider_format).unwrap_or(0);
        egui::ComboBox::from_id_salt("add_fmt").selected_text(app.add_provider_format.as_str())
            .show_ui(ui, |ui| { for (i,f) in fmts.iter().enumerate() { ui.selectable_value(&mut fi, i, *f); }});
        if fi < fmts.len() { app.add_provider_format = fmts[fi].to_string(); }
        ui.add_space(4.0);
        ui.label(egui::RichText::new("API Key").size(11.0).color(app.theme.text));
        ui.add(egui::TextEdit::singleline(&mut app.add_provider_key).hint_text("${env:KEY}").desired_width(240.0));
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            if ui.add(app.theme.primary_button("Save")).clicked() {
                let name = app.add_provider_name.trim().to_lowercase().replace(' ', "-");
                if !name.is_empty() && !app.add_provider_url.trim().is_empty() {
                    let def = ProviderDefinition { id: name.clone(), display_name: app.add_provider_name.trim().into(),
                        base_url: app.add_provider_url.trim().into(), api_format: ApiFormat::from_str(&app.add_provider_format),
                        api_key_ref: app.add_provider_key.trim().into(), models: vec![], builtin: false };
                    match app.provider_registry.save_custom(&def) {
                        Ok(()) => { app.provider_registry = ProviderRegistry::load();
                            app.push_toast(format!("Added: {}", name), ToastLevel::Info);
                            app.add_provider_name.clear(); app.add_provider_url.clear();
                            app.add_provider_key.clear(); app.show_add_provider = false; }
                        Err(e) => app.push_toast(format!("{}", e), ToastLevel::Error),
                    }
                }
            }
            if ui.add(app.theme.secondary_button("Cancel")).clicked() { app.show_add_provider = false; }
        });
    });
}

// ============================================================================
// Interface
// ============================================================================

fn render_interface(app: &mut App, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new(app.t("Interface")).color(app.theme.text).size(15.0).strong());
    ui.add_space(16.0);
    ui.label(egui::RichText::new(app.t("Theme")).size(12.0).color(app.theme.text).strong());
    let themes = ["dark","light"];
    let ct = themes.iter().position(|t| *t == app.settings_edit.theme).unwrap_or(0);
    let mut ts = ct;
    egui::ComboBox::from_id_salt("st_theme").selected_text(&app.settings_edit.theme)
        .show_ui(ui, |ui| { for (i,t) in themes.iter().enumerate() { ui.selectable_value(&mut ts, i, *t); }});
    if ts != ct { app.settings_edit.theme = themes[ts].to_string();
        app.theme = if app.settings_edit.theme == "light" { crate::theme::Theme::light() } else { crate::theme::Theme::dark() };
        app.auto_save_settings(); }
    ui.add_space(12.0);
    ui.label(egui::RichText::new(app.t("Language")).size(12.0).color(app.theme.text).strong());
    ui.horizontal(|ui| {
        let en = matches!(app.locale, crate::i18n::Locale::EnUS);
        let zh = matches!(app.locale, crate::i18n::Locale::ZhCN);
        if ui.add(egui::Button::new(egui::RichText::new("English").size(12.0))
            .fill(if en { app.theme.accent } else { app.theme.surface })
            .corner_radius(app.theme.radius_sm as u8)).clicked() { app.locale = crate::i18n::Locale::EnUS; }
        if ui.add(egui::Button::new(egui::RichText::new("Simplified Chinese").size(12.0))
            .fill(if zh { app.theme.accent } else { app.theme.surface })
            .corner_radius(app.theme.radius_sm as u8)).clicked() { app.locale = crate::i18n::Locale::ZhCN; }
    });
}

// ============================================================================
// About
// ============================================================================

fn render_about(app: &mut App, ui: &mut egui::Ui) {
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new("Clarity").size(26.0).strong().color(app.theme.text));
        ui.label(egui::RichText::new("Local-first AI agent runtime").size(13.0).color(app.theme.text_muted));
        ui.add_space(12.0);
        ui.label(egui::RichText::new(format!("v{}", env!("CARGO_PKG_VERSION"))).size(12.0).color(app.theme.text_dim));
        ui.label(egui::RichText::new("egui 0.31 · glow").size(11.0).color(app.theme.text_dim));
        ui.add_space(8.0);
        ui.hyperlink_to(egui::RichText::new("github.com/juice094/clarity").size(11.0).color(app.theme.accent),
            "https://github.com/juice094/clarity");
    });
}
