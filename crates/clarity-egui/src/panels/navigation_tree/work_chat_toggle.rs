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

/// Render the work/chat segmented toggle as a single pill.
///
/// Clicking a segment updates `app.nav_context` and persists the choice.
/// Context-sensitive modals (web link / template editors) are closed on
/// switch to prevent editing a list that no longer matches the visible mode.
pub fn render_work_chat_toggle(app: &mut App, ui: &mut egui::Ui) {
    let theme = app.ui_store.theme.clone();
    let is_work = app.nav_context == crate::settings::NavContext::Work;
    let available = ui.available_width();
    let segment_w = (available - theme.space_4) / 2.0;
    let segment_h = theme.size_nav_row_h;
    let full = theme.radius_full as u8;
    let inner = theme.radius_sm as u8;

    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 0.0;

        let work_radius = egui::CornerRadius {
            nw: full,
            ne: inner,
            sw: full,
            se: inner,
        };
        let work_resp = segment_button(
            ui,
            &theme,
            crate::theme::ICON_MONITOR,
            app.t("Work"),
            segment_w,
            segment_h,
            work_radius,
            is_work,
        );
        if work_resp.clicked() && !is_work {
            app.nav_context = crate::settings::NavContext::Work;
            close_context_sensitive_modals(app);
            app.persist_layout_settings();
        }

        let chat_radius = egui::CornerRadius {
            nw: inner,
            ne: full,
            sw: inner,
            se: full,
        };
        let chat_resp = segment_button(
            ui,
            &theme,
            crate::theme::ICON_CHAT,
            app.t("Chat"),
            segment_w,
            segment_h,
            chat_radius,
            !is_work,
        );
        if chat_resp.clicked() && is_work {
            app.nav_context = crate::settings::NavContext::Chat;
            close_context_sensitive_modals(app);
            app.persist_layout_settings();
        }
    });
}

#[allow(clippy::too_many_arguments)]
fn segment_button(
    ui: &mut egui::Ui,
    theme: &crate::theme::Theme,
    icon: &str,
    text: &str,
    width: f32,
    height: f32,
    radius: egui::CornerRadius,
    selected: bool,
) -> egui::Response {
    let text_color = if selected {
        theme.toggle_active_text
    } else {
        theme.text_dim
    };
    let fill = if selected {
        theme.accent
    } else {
        theme.surface
    };

    let mut job = egui::text::LayoutJob::default();
    job.append(
        icon,
        0.0,
        egui::text::TextFormat {
            font_id: theme.font_icon(theme.text_sm),
            color: text_color,
            ..Default::default()
        },
    );
    job.append(" ", theme.space_4, egui::text::TextFormat::default());
    job.append(
        text,
        0.0,
        egui::text::TextFormat {
            font_id: egui::FontId::new(theme.text_sm, egui::FontFamily::Proportional),
            color: text_color,
            ..Default::default()
        },
    );

    let button = egui::Button::new(job)
        .fill(fill)
        .stroke(egui::Stroke::NONE)
        .corner_radius(radius)
        .min_size(egui::vec2(width, height));

    ui.add_sized([width, height], button)
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
