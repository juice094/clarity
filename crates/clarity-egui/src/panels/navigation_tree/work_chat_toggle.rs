//! Work/Chat mode toggle at the top of the left navigation tree.
//!
//! # Design: mode ≠ sidebar structure
//!
//! The Work/Chat toggle does **not** change which sections appear in the
//! sidebar. Both modes show the same layout (web bookmarks, templates,
//! projects, claw, history). The toggle affects only **session initialization
//! context**: when `new_session()` is called, the active mode determines
//! default system prompt, tool presets, and project association.
//!
//! This keeps the sidebar predictable and avoids the common "disappearing UI"
//! anti-pattern where switching modes hides navigation targets.

use crate::App;

/// Render the work/chat segmented toggle as two equal-width pill segments.
///
/// Clicking a segment updates `app.nav_context` and persists the choice.
/// Context-sensitive modals (web link / template editors) are closed on
/// switch to prevent editing a list that no longer matches the visible mode.
pub fn render_work_chat_toggle(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    let is_work = app.nav_context == crate::settings::NavContext::Work;
    let available = ui.available_width();
    let segment_w = (available - theme.space_4) / 2.0;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = theme.space_4;

        // Work segment
        let work_fill = if is_work {
            theme.accent
        } else {
            theme.bg_hover
        };
        let work_fg = if is_work {
            theme.toggle_active_text
        } else {
            theme.text_dim
        };
        let work_btn = egui::Button::new(
            egui::RichText::new(format!("{} {}", crate::theme::ICON_MONITOR, app.t("Work")))
                .size(theme.text_sm)
                .color(work_fg),
        )
        .fill(work_fill)
        .min_size(egui::vec2(segment_w, 28.0))
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
        if ui.add(work_btn).clicked() && !is_work {
            app.nav_context = crate::settings::NavContext::Work;
            close_context_sensitive_modals(app);
            app.persist_layout_settings();
        }

        // Chat segment
        let chat_fill = if !is_work {
            theme.accent
        } else {
            theme.bg_hover
        };
        let chat_fg = if !is_work {
            theme.toggle_active_text
        } else {
            theme.text_dim
        };
        let chat_btn = egui::Button::new(
            egui::RichText::new(format!("{} {}", crate::theme::ICON_CHAT, app.t("Chat")))
                .size(theme.text_sm)
                .color(chat_fg),
        )
        .fill(chat_fill)
        .min_size(egui::vec2(segment_w, 28.0))
        .corner_radius(egui::CornerRadius::same(theme.radius_sm as u8));
        if ui.add(chat_btn).clicked() && is_work {
            app.nav_context = crate::settings::NavContext::Chat;
            close_context_sensitive_modals(app);
            app.persist_layout_settings();
        }
    });
}

/// Close any modal whose editing target no longer matches the current context.
fn close_context_sensitive_modals(app: &mut App) {
    if matches!(
        app.view_state.modal,
        Some(
            clarity_core::ui::ModalType::ManageWebLinksChat
                | clarity_core::ui::ModalType::ManageWebLinksWork
                | clarity_core::ui::ModalType::ManageWorkTemplates
        )
    ) {
        app.view_state.close_modal();
    }
}
