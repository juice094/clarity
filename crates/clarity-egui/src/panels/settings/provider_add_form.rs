//! Add custom provider form.
use crate::App;
use crate::design_system::{self, Space, TextStyle};
use crate::provider::{ApiFormat, AuthMode, ProviderDefinition, ProviderRegistry};
use crate::ui::types::ToastLevel;

pub(super) fn render_add_form(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    egui::Frame::new()
        .fill(theme.bg_accent)
        .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
        .inner_margin(egui::Margin::same(12))
        .show(ui, |ui| {
            design_system::text(ui, "Add Custom Provider", TextStyle::Title);
            ui.add_space(8.0);

            ui.label(
                egui::RichText::new("Name")
                    .size(theme.text_sm)
                    .color(theme.text),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.settings_store.add_provider_name)
                    .hint_text("my-provider")
                    .desired_width(240.0),
            );
            ui.add_space(4.0);

            ui.label(
                egui::RichText::new("Base URL")
                    .size(theme.text_sm)
                    .color(theme.text),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.settings_store.add_provider_url)
                    .hint_text("https://...")
                    .desired_width(240.0),
            );
            ui.add_space(4.0);

            ui.label(
                egui::RichText::new("API Format")
                    .size(theme.text_sm)
                    .color(theme.text),
            );
            let fmts = ["openai-completions", "anthropic-messages"];
            let mut fi = fmts
                .iter()
                .position(|f| *f == app.settings_store.add_provider_format)
                .unwrap_or(0);
            egui::ComboBox::from_id_salt("add_fmt")
                .selected_text(app.settings_store.add_provider_format.as_str())
                .show_ui(ui, |ui| {
                    for (i, f) in fmts.iter().enumerate() {
                        ui.selectable_value(&mut fi, i, *f);
                    }
                });
            if fi < fmts.len() {
                app.settings_store.add_provider_format = fmts[fi].to_string();
            }
            ui.add_space(4.0);

            ui.label(
                egui::RichText::new("API Key")
                    .size(theme.text_sm)
                    .color(theme.text),
            );
            ui.add(
                egui::TextEdit::singleline(&mut app.settings_store.add_provider_key)
                    .hint_text("${env:KEY}")
                    .desired_width(240.0),
            );
            ui.add_space(8.0);

            ui.horizontal(|ui| {
                if ui.add(theme.primary_button("Save")).clicked() {
                    let name = app
                        .settings_store
                        .add_provider_name
                        .trim()
                        .to_lowercase()
                        .replace(' ', "-");
                    if !name.is_empty() && !app.settings_store.add_provider_url.trim().is_empty() {
                        let def = ProviderDefinition {
                            id: name.clone(),
                            display_name: app.settings_store.add_provider_name.trim().into(),
                            base_url: app.settings_store.add_provider_url.trim().into(),
                            api_format: ApiFormat::from_str(
                                &app.settings_store.add_provider_format,
                            ),
                            auth_type: crate::provider::AuthType::ApiKey,
                            api_key_ref: app.settings_store.add_provider_key.trim().into(),
                            auth_token_key: String::new(),
                            models: vec![],
                            builtin: false,
                            tags: vec![],
                            ..Default::default()
                        };
                        match def.validate_api_key_prefix() {
                            Err(e) => app.push_toast(e, ToastLevel::Warn),
                            Ok(()) => {
                                match app.settings_store.provider_registry.save_custom(&def) {
                                    Ok(()) => {
                                        app.settings_store.provider_registry =
                                            ProviderRegistry::load();
                                        app.push_toast(
                                            format!("Added: {}", name),
                                            ToastLevel::Info,
                                        );
                                        app.settings_store.add_provider_name.clear();
                                        app.settings_store.add_provider_url.clear();
                                        app.settings_store.add_provider_key.clear();
                                        app.settings_store.show_add_provider = false;
                                        app.settings_store.settings_edit.provider = name.clone();
                                        if let Some(prov) =
                                            app.settings_store.provider_registry.get(&name)
                                        {
                                            if !prov.models.is_empty() {
                                                app.settings_store.settings_edit.model =
                                                    prov.models[0].clone();
                                            }
                                        }
                                    }
                                    Err(e) => app.push_toast(e.to_string(), ToastLevel::Error),
                                }
                            }
                        }
                    }
                }
                if ui.add(theme.secondary_button("Cancel")).clicked() {
                    app.settings_store.show_add_provider = false;
                }
            });
        });
}
