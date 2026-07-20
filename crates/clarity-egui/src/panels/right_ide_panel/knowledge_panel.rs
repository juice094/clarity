//! OKF Knowledge base browser and knowledge-field explorer for the right IDE rail.

use crate::App;
use crate::design_system::{self, Space, TextStyle};
use clarity_core::okf::{OkfBundleCache, OkfConcept};
use clarity_knowledge::FieldResult;
use clarity_ui::widgets::button::Button;
use clarity_ui::widgets::text_input::TextInput;

/// Render the knowledge base panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.context.ui_store.theme.clone();

    render_field_section(app, ui);

    design_system::gap(ui, Space::S2);
    ui.separator();
    design_system::gap(ui, Space::S2);

    let bundle_path_hint = app.t("Path to OKF bundle directory");
    design_system::field_label(ui, app.t("Bundle path"));
    design_system::gap(ui, Space::S0);

    ui.horizontal(|ui| {
        let path_edit = ui.add_sized(
            [ui.available_width() - theme.space_16, theme.size_input],
            TextInput::singleline(&mut app.context.knowledge_store.bundle_path)
                .hint_text(bundle_path_hint),
        );
        if path_edit.changed() {
            app.context.knowledge_store.error = None;
        }
    });
    design_system::gap(ui, Space::S1);

    ui.horizontal(|ui| {
        let can_load = !app.context.knowledge_store.bundle_path.is_empty()
            && !app.context.knowledge_store.loading;
        let button_size = egui::vec2(theme.space_16 * 5.0, theme.size_input);
        if ui
            .add_sized(button_size, Button::new(app.t("Load bundle")).primary())
            .clicked()
            && can_load
        {
            trigger_bundle_load(app);
        }
        if ui
            .add_sized(button_size, Button::new(app.t("Reload")).primary())
            .clicked()
            && can_load
        {
            invalidate_and_reload(app);
        }
    });
    design_system::gap(ui, Space::S2);

    if app.context.knowledge_store.loading {
        ui.label(
            egui::RichText::new(app.t("Loading…"))
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        return;
    }

    if let Some(err) = &app.context.knowledge_store.error {
        ui.label(
            egui::RichText::new(format!("{}: {}", app.t("Failed to load bundle"), err))
                .size(theme.text_sm)
                .color(theme.error_text),
        );
        design_system::gap(ui, Space::S2);
    }

    let search_hint = app.t("Search concepts");
    let query_changed = {
        let mut query = app.context.knowledge_store.query.clone();
        let response = ui.add_sized(
            [ui.available_width(), theme.size_input],
            TextInput::singleline(&mut query).hint_text(search_hint),
        );
        let changed = response.changed();
        if changed {
            app.context.knowledge_store.set_query(query);
        }
        changed
    };

    if !app.context.knowledge_store.bundle_path.is_empty()
        && app.context.knowledge_store.bundle.is_none()
        && app.context.knowledge_store.error.is_none()
        && !app.context.knowledge_store.loading
    {
        design_system::gap(ui, Space::S1);
        ui.label(
            egui::RichText::new(app.t("Enter a bundle path and click Load bundle"))
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
    }

    design_system::gap(ui, Space::S2);

    egui::ScrollArea::vertical()
        .id_salt("knowledge_concept_list")
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            render_concept_list(app, ui);
        });

    if query_changed {
        ui.ctx().request_repaint();
    }
}

fn render_field_section(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.context.ui_store.theme.clone();

    design_system::text(ui, app.t("Knowledge Field"), TextStyle::Small);
    design_system::gap(ui, Space::S0);

    let vault_hint = app.t("Path to markdown vault");
    let vault_response = ui.add_sized(
        [ui.available_width(), theme.size_input],
        TextInput::singleline(&mut app.context.knowledge_store.vault_path).hint_text(vault_hint),
    );
    if vault_response.changed() {
        app.context.knowledge_store.error = None;
    }

    design_system::gap(ui, Space::S1);
    ui.horizontal(|ui| {
        let button_size = egui::vec2(theme.space_16 * 5.0, theme.size_input);
        if ui
            .add_sized(button_size, Button::new(app.t("Index vault")).primary())
            .clicked()
            && !app.context.knowledge_store.vault_path.is_empty()
        {
            let indexed = app.context.knowledge_store.index_vault();
            tracing::info!("Indexed {} vault files into knowledge field", indexed);
        }

        let watching = app.context.knowledge_store.vault_watching;
        let watch_label = if watching {
            app.t("Stop watching")
        } else {
            app.t("Watch vault")
        };
        if ui
            .add_sized(button_size, Button::new(watch_label).primary())
            .clicked()
            && !app.context.knowledge_store.vault_path.is_empty()
        {
            if watching {
                app.context.knowledge_store.stop_watching_vault();
            } else {
                let ui_tx = app.context.ui_tx.clone();
                app.context
                    .knowledge_store
                    .start_watching_vault(&app.context.runtime, ui_tx);
            }
        }
    });

    design_system::gap(ui, Space::S2);

    let search_hint = app.t("Search field");
    let mut query = app.context.knowledge_store.field_query.clone();
    let response = ui.add_sized(
        [ui.available_width(), theme.size_input],
        TextInput::singleline(&mut query).hint_text(search_hint),
    );
    if response.changed() {
        app.context.knowledge_store.set_field_query(query);
    }

    design_system::gap(ui, Space::S1);
    ui.horizontal(|ui| {
        let button_size = egui::vec2(theme.space_16 * 5.0, theme.size_input);
        if ui
            .add_sized(button_size, Button::new(app.t("Search")).primary())
            .clicked()
        {
            app.context.knowledge_store.search_field();
        }
        if ui
            .add_sized(button_size, Button::new(app.t("Top active")).primary())
            .clicked()
        {
            app.context.knowledge_store.refresh_top_activated(10);
        }
    });

    design_system::gap(ui, Space::S2);

    egui::ScrollArea::vertical()
        .id_salt("knowledge_field_results")
        .auto_shrink([false; 2])
        .max_height(240.0)
        .show(ui, |ui| {
            render_field_results(app, ui);
        });
}

fn path_title(path: &std::path::Path) -> Option<String> {
    path.file_stem().map(|s| s.to_string_lossy().to_string())
}

fn render_field_results(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.context.ui_store.theme.clone();
    let results = app.context.knowledge_store.field_results.clone();

    if results.is_empty() {
        ui.label(
            egui::RichText::new(app.t("No field results"))
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        return;
    }

    design_system::text(
        ui,
        format!("{} ({})", app.t("Results"), results.len()),
        TextStyle::Small,
    );
    design_system::gap(ui, Space::S0);

    let selected_path = app.context.knowledge_store.selected_field_path.clone();
    for result in results {
        let is_selected = selected_path.as_ref() == Some(&result.path);
        let label = result
            .title
            .clone()
            .or_else(|| path_title(&result.path))
            .unwrap_or_else(|| result.path.to_string_lossy().to_string());
        let summary = format!("{:.2} {}", result.activation, label);

        let response = clarity_ui::design_system::Elevation::Elevated
            .frame(&theme)
            .fill(if is_selected {
                theme.accent_subtle
            } else {
                theme.surface
            })
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
            .inner_margin(egui::Margin::same(theme.space_8 as i8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                clarity_ui::design_system::text_with_color(
                    ui,
                    summary,
                    clarity_ui::design_system::TextStyle::Small,
                    if is_selected {
                        theme.accent
                    } else {
                        theme.text_strong
                    },
                );
            })
            .response;
        if response.clicked() {
            app.context
                .knowledge_store
                .select_field_path(result.path.clone());
        }
    }

    if let Some(path) = selected_path {
        if let Some(result) = app
            .context
            .knowledge_store
            .field_results
            .iter()
            .find(|r| r.path == path)
            .cloned()
        {
            design_system::gap(ui, Space::S3);
            render_field_detail(app, ui, &result);
        }
    }
}

fn render_field_detail(app: &mut App, ui: &mut egui::Ui, result: &FieldResult) {
    let theme = app.context.ui_store.theme.clone();

    ui.label(
        egui::RichText::new(
            result
                .title
                .clone()
                .or_else(|| path_title(&result.path))
                .unwrap_or_else(|| result.path.to_string_lossy().to_string()),
        )
        .size(theme.text_base)
        .strong()
        .color(theme.text_strong),
    );
    design_system::gap(ui, Space::S0);

    ui.label(
        egui::RichText::new(result.path.to_string_lossy().to_string())
            .size(theme.text_xs)
            .color(theme.text_dim),
    );
    design_system::gap(ui, Space::S1);

    if !result.snippet.is_empty() {
        ui.label(
            egui::RichText::new(&result.snippet)
                .size(theme.text_sm)
                .color(theme.text_muted),
        );
    }

    if !result.matched_tags.is_empty() {
        design_system::gap(ui, Space::S1);
        ui.label(
            egui::RichText::new(format!(
                "{}: {}",
                app.t("Tags"),
                result.matched_tags.join(", ")
            ))
            .size(theme.text_xs)
            .color(theme.text_dim),
        );
    }
}

fn render_concept_list(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.context.ui_store.theme.clone();
    let results = app.context.knowledge_store.results.clone();

    if results.is_empty() {
        ui.label(
            egui::RichText::new(app.t("No concepts found"))
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        return;
    }

    design_system::text(
        ui,
        format!("{} ({})", app.t("Concepts"), results.len()),
        TextStyle::Small,
    );
    design_system::gap(ui, Space::S0);

    let selected_id = app.context.knowledge_store.selected_id.clone();
    for concept in results {
        let is_selected = selected_id.as_ref() == Some(&concept.id);
        let summary = concept_summary(&concept);
        let response = clarity_ui::design_system::Elevation::Elevated
            .frame(&theme)
            .fill(if is_selected {
                theme.accent_subtle
            } else {
                theme.surface
            })
            .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
            .inner_margin(egui::Margin::same(theme.space_8 as i8))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                clarity_ui::design_system::text_with_color(
                    ui,
                    summary,
                    clarity_ui::design_system::TextStyle::Small,
                    if is_selected {
                        theme.accent
                    } else {
                        theme.text_strong
                    },
                );
            })
            .response;
        if response.clicked() {
            app.context.knowledge_store.select(concept.id.clone());
        }
    }

    if let Some(id) = selected_id {
        if let Some(concept) = app
            .context
            .knowledge_store
            .bundle
            .as_ref()
            .and_then(|b| b.get(&id))
            .cloned()
        {
            design_system::gap(ui, Space::S3);
            render_concept_detail(app, ui, &concept);
        }
    }
}

fn render_concept_detail(app: &mut App, ui: &mut egui::Ui, concept: &OkfConcept) {
    let theme = app.context.ui_store.theme.clone();

    ui.label(
        egui::RichText::new(concept_title(concept))
            .size(theme.text_base)
            .strong()
            .color(theme.text_strong),
    );
    design_system::gap(ui, Space::S0);

    ui.horizontal_wrapped(|ui| {
        design_system::text(
            ui,
            format!("{}: {}", app.t("Type"), concept.frontmatter.r#type),
            TextStyle::Small,
        );
        if !concept.frontmatter.tags.is_empty() {
            design_system::gap(ui, Space::S1);
            design_system::text(
                ui,
                format!("{}: {}", app.t("Tags"), concept.frontmatter.tags.join(", ")),
                TextStyle::Small,
            );
        }
    });
    design_system::gap(ui, Space::S1);

    if let Some(resource) = &concept.frontmatter.resource {
        if ui
            .hyperlink_to(
                egui::RichText::new(resource)
                    .size(theme.text_xs)
                    .color(theme.accent),
                resource,
            )
            .clicked()
        {
            ui.ctx().copy_text(resource.clone());
        }
        design_system::gap(ui, Space::S1);
    }

    if let Some(description) = &concept.frontmatter.description {
        ui.label(
            egui::RichText::new(description)
                .size(theme.text_sm)
                .color(theme.text_muted)
                .italics(),
        );
        design_system::gap(ui, Space::S1);
    }

    if !concept.body.is_empty() {
        crate::ui::markdown::render_markdown(ui, &concept.body, theme.chat_text);
    }
}

fn trigger_bundle_load(app: &mut App) {
    app.context.knowledge_store.loading = true;
    app.context.knowledge_store.error = None;
    app.context.knowledge_store.bundle = None;
    app.context.knowledge_store.results.clear();
    app.context.knowledge_store.selected_id = None;

    let path = app.context.knowledge_store.bundle_path.clone();
    let tx = app.context.ui_tx.clone();
    app.context.runtime.spawn(async move {
        let result = tokio::task::spawn_blocking({
            let path = path.clone();
            move || OkfBundleCache::global().get_or_load(path)
        })
        .await;
        let mapped = match result {
            Ok(Ok(bundle)) => Ok(bundle),
            Ok(Err(e)) => Err(e.to_string()),
            Err(e) => Err(format!("Load task panicked: {}", e)),
        };
        let _ = tx.send(crate::ui::types::UiEvent::KnowledgeLoaded {
            path,
            result: mapped,
        });
    });
}

fn invalidate_and_reload(app: &mut App) {
    let path = app.context.knowledge_store.bundle_path.clone();
    let _ = OkfBundleCache::global().invalidate(&path);
    trigger_bundle_load(app);
}

fn concept_summary(concept: &OkfConcept) -> String {
    let title = concept_title(concept);
    if title == concept.id {
        concept.id.clone()
    } else {
        format!("{} — {}", title, concept.id)
    }
}

fn concept_title(concept: &OkfConcept) -> String {
    concept
        .frontmatter
        .title
        .as_deref()
        .map(|t| t.to_string())
        .unwrap_or_else(|| concept.id.clone())
}

// ── Panel trait implementation ──

/// Knowledge base panel renderer.
pub struct KnowledgePanel;

impl crate::design_system::Panel for KnowledgePanel {
    fn title(&self, app: &crate::App) -> &str {
        app.t("Knowledge")
    }
    fn render(&mut self, app: &mut crate::App, ui: &mut egui::Ui) {
        render(app, ui);
    }
}
