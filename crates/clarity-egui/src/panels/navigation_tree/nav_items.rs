//! Primary navigation items: New Task CTA, Skills, and Plugins links.
//!
//! Icon sizing: top-level nav items use `theme.text_base` for icons
//! (vs `theme.text_sm` inside collapsible sections) to establish a
//! deliberate visual hierarchy — primary actions are larger and more
//! prominent than secondary list entries.

use crate::App;

/// Render the "New Task" button and Skills/Plugins navigation links.
pub fn render_nav_items(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    let is_loading = matches!(app.view_state.turn, clarity_core::ui::TurnState::Loading);

    // ── New Task ──
    let new_task_btn = egui::Button::new(
        egui::RichText::new(format!(
            "{}  {}",
            crate::theme::ICON_PLUS,
            app.t("New Task")
        ))
        .size(theme.text_sm)
        .color(theme.nav_cta_text),
    )
    .fill(theme.accent)
    .min_size(egui::vec2(ui.available_width(), 40.0))
    .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
    let mut resp = ui.add_enabled(!is_loading, new_task_btn);
    if !is_loading {
        resp = resp.on_hover_text(app.t("New session (Ctrl+N)"));
    }
    if resp.clicked() {
        app.new_session();
    }

    ui.add_space(theme.space_12);

    // ── Skills ──
    let is_skills_open = matches!(
        app.view_state.modal,
        Some(clarity_core::ui::ModalType::Skill)
    );
    let skills_resp = crate::widgets::interactive_row(ui, is_skills_open, &theme, |ui| {
        ui.add_space(theme.space_4);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.space_12;
            ui.label(
                egui::RichText::new(crate::theme::ICON_BOOK)
                    .size(theme.text_base)
                    .color(if is_skills_open {
                        theme.accent
                    } else {
                        theme.text_dim
                    }),
            );
            ui.label(
                egui::RichText::new(app.t("Skills"))
                    .size(theme.text_sm)
                    .color(if is_skills_open {
                        theme.text_strong
                    } else {
                        theme.text
                    }),
            );
        });
        ui.add_space(theme.space_4);
    });
    if skills_resp.response.clicked() {
        app.view_state
            .open_modal(clarity_core::ui::ModalType::Skill);
    }

    // ── Plugins ──
    let is_plugins_open = matches!(app.view_state.modal, Some(clarity_core::ui::ModalType::Mcp));
    let plugins_resp = crate::widgets::interactive_row(ui, is_plugins_open, &theme, |ui| {
        ui.add_space(theme.space_4);
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.space_12;
            ui.label(
                egui::RichText::new(crate::theme::ICON_LAYERS)
                    .size(theme.text_base)
                    .color(if is_plugins_open {
                        theme.accent
                    } else {
                        theme.text_dim
                    }),
            );
            ui.label(
                egui::RichText::new(app.t("Plugins"))
                    .size(theme.text_sm)
                    .color(if is_plugins_open {
                        theme.text_strong
                    } else {
                        theme.text
                    }),
            );
        });
        ui.add_space(theme.space_4);
    });
    if plugins_resp.response.clicked() {
        app.view_state.open_modal(clarity_core::ui::ModalType::Mcp);
    }
}
