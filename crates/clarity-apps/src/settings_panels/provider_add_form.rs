//! Add custom provider form.
use crate::provider::{ApiFormat, ProviderDefinition, ProviderRegistry};
use crate::settings::SettingsStore;
use clarity_shell::{AppState, ToastLevel};
use clarity_ui::design_system::{self, TextStyle};
use clarity_ui::widgets::text_input::TextInput;

pub(super) fn render_add_form(
    store: &mut SettingsStore,
    state: &mut dyn AppState,
    theme: &clarity_ui::theme::Theme,
    ui: &mut egui::Ui,
) {
    design_system::surface_panel(ui, |ui| {
        design_system::text(ui, "Add Custom Provider", TextStyle::Title);
        design_system::gap(ui, design_system::Space::S1);

        design_system::field_label(ui, "Name");
        ui.add(
            TextInput::singleline(&mut store.add_provider_name)
                .hint_text("my-provider")
                .width(240.0),
        );
        design_system::gap(ui, design_system::Space::S0);

        design_system::field_label(ui, "Base URL");
        ui.add(
            TextInput::singleline(&mut store.add_provider_url)
                .hint_text("https://...")
                .width(240.0),
        );
        design_system::gap(ui, design_system::Space::S0);

        design_system::field_label(ui, "API Format");
        let fmts = ["openai-completions", "anthropic-messages"];
        let mut fi = fmts
            .iter()
            .position(|f| *f == store.add_provider_format)
            .unwrap_or(0);
        egui::ComboBox::from_id_salt("add_fmt")
            .selected_text(store.add_provider_format.as_str())
            .show_ui(ui, |ui| {
                for (i, f) in fmts.iter().enumerate() {
                    ui.selectable_value(&mut fi, i, *f);
                }
            });
        if fi < fmts.len() {
            store.add_provider_format = fmts[fi].to_string();
        }
        design_system::gap(ui, design_system::Space::S0);

        design_system::field_label(ui, "API Key");
        ui.add(
            TextInput::singleline(&mut store.add_provider_key)
                .hint_text("${env:KEY}")
                .width(240.0),
        );
        design_system::gap(ui, design_system::Space::S1);

        ui.horizontal(|ui| {
            if ui.add(theme.primary_button("Save")).clicked() {
                let name = store
                    .add_provider_name
                    .trim()
                    .to_lowercase()
                    .replace(' ', "-");
                if !name.is_empty() && !store.add_provider_url.trim().is_empty() {
                    let def = ProviderDefinition {
                        id: name.clone(),
                        display_name: store.add_provider_name.trim().into(),
                        base_url: store.add_provider_url.trim().into(),
                        api_format: ApiFormat::from_str(&store.add_provider_format),
                        auth_type: crate::provider::AuthType::ApiKey,
                        api_key_ref: store.add_provider_key.trim().into(),
                        auth_token_key: String::new(),
                        models: vec![],
                        builtin: false,
                        tags: vec![],
                        ..Default::default()
                    };
                    match def.validate_api_key_prefix() {
                        Err(e) => state.push_toast(e, ToastLevel::Warn),
                        Ok(()) => match store.provider_registry.save_custom(&def) {
                            Ok(()) => {
                                store.provider_registry = ProviderRegistry::load();
                                state.push_toast(format!("Added: {}", name), ToastLevel::Info);
                                store.add_provider_name.clear();
                                store.add_provider_url.clear();
                                store.add_provider_key.clear();
                                store.show_add_provider = false;
                                store.settings_edit.provider = name.clone();
                                if let Some(prov) = store.provider_registry.get(&name) {
                                    if !prov.models.is_empty() {
                                        store.settings_edit.model = prov.models[0].clone();
                                    }
                                }
                            }
                            Err(e) => state.push_toast(e.to_string(), ToastLevel::Error),
                        },
                    }
                }
            }
            if ui.add(theme.secondary_button("Cancel")).clicked() {
                store.show_add_provider = false;
            }
        });
    });
}
