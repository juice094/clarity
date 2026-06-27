use crate::App;
use crate::ui::types::ToastLevel;
use std::time::{Duration, Instant};

/// Cubic ease-out: decelerates toward the end.
fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Maximum toast lifetime before auto-dismiss.
const TOAST_TTL: Duration = Duration::from_secs(5);
/// Toast card width.
const TOAST_W: f32 = 320.0;
/// Vertical stack spacing.
const TOAST_H: f32 = 56.0;

/// Renders the toasts UI — right-aligned stack near the bottom-right corner
/// with fade-in animation, per-level icons, and click-to-dismiss.
pub fn render_toasts(app: &mut App, ctx: &egui::Context) {
    let now = Instant::now();
    let theme = app.ui_store.theme.clone();

    // Purge expired toasts.
    app.ui_store
        .toasts
        .retain(|t| now.duration_since(t.created_at) < TOAST_TTL);

    if app.ui_store.toasts.is_empty() {
        return;
    }

    let screen = ctx.screen_rect();
    let max_toasts = ((screen.height() - 40.0) / TOAST_H).max(1.0) as usize;
    let toast_count = app.ui_store.toasts.len().min(max_toasts);

    // Snapshot visible toasts for rendering (avoid borrowing issues for dismiss).
    let visible: Vec<_> = app
        .ui_store
        .toasts
        .iter()
        .take(max_toasts)
        .cloned()
        .collect();

    let mut dismissed: Vec<usize> = Vec::new();

    for (i, toast) in visible.iter().enumerate() {
        let count_from_bottom = toast_count - i;
        let x = screen.max.x - TOAST_W - theme.space_16;
        let y = screen.max.y - 20.0 - count_from_bottom as f32 * TOAST_H;

        let elapsed = now.duration_since(toast.created_at).as_secs_f32();
        let fade_ratio = (elapsed / theme.duration_normal).min(1.0);
        let alpha = ease_out_cubic(fade_ratio);

        let (icon, accent_color, border_color) = match toast.level {
            ToastLevel::Info => (crate::theme::ICON_INFO, theme.accent, theme.accent_subtle),
            ToastLevel::Warn => (
                crate::theme::ICON_WARNING,
                theme.warn,
                rgba(212, 160, 80, 0.20),
            ),
            ToastLevel::Error => (
                crate::theme::ICON_X,
                theme.danger,
                rgba(239, 107, 107, 0.20),
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

                egui::Frame::new()
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
                            ui.label(
                                egui::RichText::new(icon)
                                    .font(theme.font_icon(theme.text_md))
                                    .color(accent_color.linear_multiply(alpha)),
                            );
                            ui.add_space(theme.space_8);

                            ui.vertical(|ui| {
                                let trunc = crate::ui::truncate::truncate(&toast.message, 60);
                                ui.label(
                                    egui::RichText::new(trunc)
                                        .size(theme.text_sm)
                                        .color(text_color),
                                );
                            });

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let close_btn = ui.add_sized(
                                        [20.0, 20.0],
                                        egui::Button::new(
                                            egui::RichText::new(crate::theme::ICON_X)
                                                .size(theme.text_xs)
                                                .color(dim),
                                        )
                                        .fill(egui::Color32::TRANSPARENT)
                                        .frame(false),
                                    );
                                    if close_btn.clicked() {
                                        dismissed.push(toast_id);
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

    // Remove dismissed toasts (in reverse order to preserve indices).
    dismissed.sort_unstable_by(|a, b| b.cmp(a));
    for idx in dismissed {
        if idx < app.ui_store.toasts.len() {
            app.ui_store.toasts.remove(idx);
        }
    }
}

fn rgba(r: u8, g: u8, b: u8, a: f32) -> egui::Color32 {
    let a = (a * 255.0).clamp(0.0, 255.0) as u8;
    egui::Color32::from_rgba_premultiplied(r, g, b, a)
}
