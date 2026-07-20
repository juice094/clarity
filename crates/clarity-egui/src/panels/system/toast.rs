use crate::App;
use crate::ui::types::ToastLevel;
use std::time::{Duration, Instant};

/// Maximum toast lifetime before auto-dismiss.
const TOAST_TTL: Duration = Duration::from_secs(5);
/// Toast card width.
const TOAST_W: f32 = 320.0;
/// Vertical stack spacing.
const TOAST_H: f32 = 56.0;
/// Horizontal slide distance for toast enter / exit animation.
const SLIDE_X: f32 = 48.0;

/// Renders the toasts UI — right-aligned stack near the bottom-right corner
/// with fade-in animation, per-level icons, and click-to-dismiss.
pub fn render_toasts(app: &mut App, ctx: &egui::Context) {
    let now = Instant::now();
    let theme = app.context.ui_store.theme.clone();

    // Expire toasts that reached their TTL so they begin the fade-out.
    for toast in &mut app.context.ui_store.toasts {
        if now.saturating_duration_since(toast.created_at) >= TOAST_TTL
            && toast.dismissed_at.is_none()
        {
            toast.dismissed_at = Some(now);
        }
    }

    if app.context.ui_store.toasts.is_empty() {
        return;
    }

    let screen = ctx.input(|i| i.viewport_rect());
    let max_toasts = ((screen.height() - 40.0) / TOAST_H).max(1.0) as usize;
    let toast_count = app.context.ui_store.toasts.len().min(max_toasts);

    // Snapshot visible toasts for rendering (avoid borrowing issues for dismiss).
    let visible: Vec<_> = app
        .context
        .ui_store
        .toasts
        .iter()
        .take(max_toasts)
        .cloned()
        .collect();

    let mut clicked: Vec<usize> = Vec::new();

    for (i, toast) in visible.iter().enumerate() {
        let alpha = if let Some(dismissed_at) = toast.dismissed_at {
            let elapsed = now.saturating_duration_since(dismissed_at).as_secs_f32();
            let t = (elapsed / theme.duration_normal).min(1.0);
            1.0 - crate::animation::ease_out_cubic(t)
        } else {
            let elapsed = now
                .saturating_duration_since(toast.created_at)
                .as_secs_f32();
            crate::animation::ease_out_cubic((elapsed / theme.duration_normal).min(1.0))
        };

        if alpha <= 0.0 {
            continue;
        }

        let count_from_bottom = toast_count - i;
        let base_x = screen.max.x - TOAST_W - theme.space_16;
        let y = screen.max.y - 20.0 - count_from_bottom as f32 * TOAST_H;

        // Slide in from the right on enter, slide out to the right on exit.
        let x = base_x + (1.0 - alpha) * SLIDE_X;

        let (icon, accent_color, border_color) = match toast.level {
            ToastLevel::Info => (crate::theme::ICON_INFO, theme.accent, theme.accent_subtle),
            ToastLevel::Warn => (
                crate::theme::ICON_WARNING,
                theme.warn,
                crate::theme::rgba(212, 160, 80, 0.20),
            ),
            ToastLevel::Error => (
                crate::theme::ICON_X,
                theme.danger,
                crate::theme::rgba(239, 107, 107, 0.20),
            ),
        };

        let toast_id = i; // capture for closure

        egui::Area::new(egui::Id::new(("toast_v2", i)))
            .fixed_pos(egui::pos2(x, y))
            .order(egui::Order::Foreground)
            .show(ctx, |ui| {
                let bg = theme.surface_strong.linear_multiply(alpha);
                let text_color = theme.text.linear_multiply(alpha);
                let dim = theme.text_dim.linear_multiply(alpha);

                clarity_ui::design_system::Elevation::Toast
                    .frame(&theme)
                    .fill(bg)
                    .stroke(egui::Stroke::new(1.0, border_color.linear_multiply(alpha)))
                    .corner_radius(egui::CornerRadius::same(theme.radius_md as u8))
                    .shadow(theme.shadow_toast)
                    .inner_margin(egui::Margin::symmetric(
                        theme.space_12 as i8,
                        theme.space_8 as i8,
                    ))
                    .show(ui, |ui| {
                        ui.set_width(TOAST_W - theme.space_24);
                        ui.horizontal(|ui| {
                            clarity_ui::design_system::icon_with_color(
                                ui,
                                icon,
                                theme.text_md,
                                accent_color.linear_multiply(alpha),
                            );
                            crate::design_system::gap(ui, crate::design_system::Space::S1);

                            ui.vertical(|ui| {
                                let trunc = crate::ui::truncate::truncate(&toast.message, 60);
                                clarity_ui::design_system::text_with_color(
                                    ui,
                                    trunc,
                                    clarity_ui::design_system::TextStyle::Small,
                                    text_color,
                                );
                            });

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let close_btn =
                                        clarity_ui::widgets::icon_button::icon_button_with_color(
                                            ui,
                                            crate::theme::ICON_X,
                                            theme.text_xs,
                                            egui::Color32::TRANSPARENT,
                                            dim,
                                            egui::CornerRadius::same(4),
                                            &theme,
                                        );
                                    if close_btn.clicked() {
                                        clicked.push(toast_id);
                                    }
                                    if close_btn.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                },
                            );
                        });
                    });
            });
    }

    // Mark clicked toasts as dismissed so they fade out next frame.
    for idx in clicked {
        if let Some(toast) = app.context.ui_store.toasts.get_mut(idx) {
            toast.dismissed_at = Some(now);
        }
    }

    // Remove toasts that finished their exit fade.
    app.context.ui_store.toasts.retain(|t| {
        t.dismissed_at.is_none_or(|d| {
            now.saturating_duration_since(d) < Duration::from_secs_f32(theme.duration_normal)
        })
    });
}
