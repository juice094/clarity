//! OKF Knowledge base browser for the right IDE rail.

use crate::App;
use crate::design_system::{self, Space};
use clarity_core::okf::{OkfBundleCache, OkfConcept};

/// Render the knowledge base panel.
pub fn render(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();

    let bundle_path_hint = app.t("Path to OKF bundle directory");
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(app.t("Bundle path"))
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
    });
    design_system::gap(ui, Space::S0);

    ui.horizontal(|ui| {
        let path_edit = ui.add_sized(
            [ui.available_width() - theme.space_16, theme.size_input],
            egui::TextEdit::singleline(&mut app.knowledge_store.bundle_path)
                .hint_text(bundle_path_hint),
        );
        if path_edit.changed() {
            app.knowledge_store.error = None;
        }
    });
    design_system::gap(ui, Space::S1);

    ui.horizontal(|ui| {
        let can_load = !app.knowledge_store.bundle_path.is_empty() && !app.knowledge_store.loading;
        let button_size = egui::vec2(theme.space_16 * 5.0, theme.size_input);
        if ui
            .add_sized(button_size, egui::Button::new(app.t("Load bundle")))
            .clicked()
            && can_load
        {
            trigger_bundle_load(app);
        }
        if ui
            .add_sized(button_size, egui::Button::new(app.t("Reload")))
            .clicked()
            && can_load
        {
            invalidate_and_reload(app);
        }
    });
    design_system::gap(ui, Space::S2);

    if app.knowledge_store.loading {
        ui.label(
            egui::RichText::new(app.t("Loading…"))
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        return;
    }

    if let Some(err) = &app.knowledge_store.error {
        ui.label(
            egui::RichText::new(format!("{}: {}", app.t("Failed to load bundle"), err))
                .size(theme.text_sm)
                .color(theme.error_text),
        );
        design_system::gap(ui, Space::S2);
    }

    let search_hint = app.t("Search concepts");
    let query_changed = {
        let mut query = app.knowledge_store.query.clone();
        let response = ui.add_sized(
            [ui.available_width(), theme.size_input],
            egui::TextEdit::singleline(&mut query).hint_text(search_hint),
        );
        let changed = response.changed();
        if changed {
            app.knowledge_store.set_query(query);
        }
        changed
    };

    if !app.knowledge_store.bundle_path.is_empty()
        && app.knowledge_store.bundle.is_none()
        && app.knowledge_store.error.is_none()
        && !app.knowledge_store.loading
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

fn render_concept_list(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    let results = app.knowledge_store.results.clone();

    if results.is_empty() {
        ui.label(
            egui::RichText::new(app.t("No concepts found"))
                .size(theme.text_sm)
                .color(theme.text_dim),
        );
        return;
    }

    ui.label(
        egui::RichText::new(format!("{} ({})", app.t("Concepts"), results.len()))
            .size(theme.text_xs)
            .color(theme.text_dim),
    );
    design_system::gap(ui, Space::S0);

    let selected_id = app.knowledge_store.selected_id.clone();
    for concept in results {
        let is_selected = selected_id.as_ref() == Some(&concept.id);
        let summary = concept_summary(&concept);
        let response =
            egui::Frame::new()
                .fill(if is_selected {
                    theme.accent_subtle
                } else {
                    theme.surface
                })
                .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8))
                .inner_margin(egui::Margin::same(theme.space_8 as i8))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(egui::RichText::new(summary).size(theme.text_sm).color(
                        if is_selected {
                            theme.accent
                        } else {
                            theme.text_strong
                        },
                    ));
                })
                .response;
        if response.clicked() {
            app.knowledge_store.select(concept.id.clone());
        }
    }

    if let Some(id) = selected_id {
        if let Some(concept) = app
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
    let theme = app.ui_store.theme.clone();

    ui.label(
        egui::RichText::new(concept_title(concept))
            .size(theme.text_base)
            .strong()
            .color(theme.text_strong),
    );
    design_system::gap(ui, Space::S0);

    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new(format!("{}: {}", app.t("Type"), concept.frontmatter.r#type))
                .size(theme.text_xs)
                .color(theme.text_dim),
        );
        if !concept.frontmatter.tags.is_empty() {
            design_system::gap(ui, Space::S1);
            ui.label(
                egui::RichText::new(format!(
                    "{}: {}",
                    app.t("Tags"),
                    concept.frontmatter.tags.join(", ")
                ))
                .size(theme.text_xs)
                .color(theme.text_dim),
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
        let parsed = crate::ui::markdown::parse_markdown(&concept.body);
        crate::ui::markdown::render_blocks(ui, &parsed, &theme, theme.chat_text);
    }
}

fn trigger_bundle_load(app: &mut App) {
    app.knowledge_store.loading = true;
    app.knowledge_store.error = None;
    app.knowledge_store.bundle = None;
    app.knowledge_store.results.clear();
    app.knowledge_store.selected_id = None;

    let path = app.knowledge_store.bundle_path.clone();
    let tx = app.ui_tx.clone();
    app.runtime.spawn(async move {
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
    let path = app.knowledge_store.bundle_path.clone();
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
