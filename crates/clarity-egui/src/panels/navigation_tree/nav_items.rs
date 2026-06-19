//! Primary navigation items: New Task CTA, Skills, and Plugins links.
//!
//! Rows use the shared `interactive_row` component, which provides a fixed
//! left accent bar and aligns icons on the navigation icon rail so the text
//! grid matches collapsible section headers and bot rows.

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
    .min_size(egui::vec2(ui.available_width(), theme.size_nav_row_h))
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
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.space_8;
            crate::widgets::nav_icon_rail(
                ui,
                &theme,
                crate::theme::ICON_BOOK,
                if is_skills_open {
                    theme.accent
                } else {
                    theme.text_dim
                },
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
    });
    if skills_resp.response.clicked() {
        app.view_state
            .open_modal(clarity_core::ui::ModalType::Skill);
    }

    // ── Plugins ──
    let is_plugins_open = matches!(app.view_state.modal, Some(clarity_core::ui::ModalType::Mcp));
    let plugins_resp = crate::widgets::interactive_row(ui, is_plugins_open, &theme, |ui| {
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = theme.space_8;
            crate::widgets::nav_icon_rail(
                ui,
                &theme,
                crate::theme::ICON_LAYERS,
                if is_plugins_open {
                    theme.accent
                } else {
                    theme.text_dim
                },
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
    });
    if plugins_resp.response.clicked() {
        app.view_state.open_modal(clarity_core::ui::ModalType::Mcp);
    }
}
