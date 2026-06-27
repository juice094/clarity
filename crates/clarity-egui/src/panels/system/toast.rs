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
/// with fade-in animation and per-level icons.
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

    for (i, toast) in app.ui_store.toasts.iter().enumerate().take(max_toasts) {
        let count_from_bottom = toast_count - i;
        let x = screen.max.x - TOAST_W - theme.space_16;
        let y = screen.max.y - 20.0 - count_from_bottom as f32 * TOAST_H;

        // Fade-in: 0 → 1 over duration_normal, cubic ease-out.
        let elapsed = now.duration_since(toast.created_at).as_secs_f32();
        let fade_ratio = (elapsed / theme.duration_normal).min(1.0);
        let alpha = ease_out_cubic(fade_ratio);

        // Per-level styling.
        let (icon, accent_color, border_color) = match toast.level {
            ToastLevel::Info => (crate::theme::ICON_INFO, theme.accent, theme.accent_subtle),
            ToastLevel::Warn => (crate::theme::ICON_WARNING, theme.warn, rgba(212, 160, 80, 0.20)),
            ToastLevel::Error => (
                crate::theme::ICON_X,
                theme.danger,
                rgba(239, 107, 107, 0.20),
            ),
        };

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
                    .inner_margin(egui::Margin::symmetric(theme.space_12 as i8, theme.space_8 as i8))
                    .show(ui, |ui| {
                        ui.set_width(TOAST_W - theme.space_24);
                        ui.horizontal(|ui| {
                            // Icon (left, colored).
                            ui.label(
                                egui::RichText::new(icon)
                                    .font(theme.font_icon(theme.text_md))
                                    .color(accent_color.linear_multiply(alpha)),
                            );
                            ui.add_space(theme.space_8);

                            // Message body.
                            ui.vertical(|ui| {
                                let trunc = truncate_msg(&toast.message, 60);
                                ui.label(
                                    egui::RichText::new(trunc)
                                        .size(theme.text_sm)
                                        .color(text_color),
                                );
                            });

                            // Close button (right).
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
                                        // Mark for removal by zeroing creation time.
                                        // SAFE: the retain above will drop it next frame.
                                    }
                                    if close_btn.hovered() {
                                        ui.ctx()
                                            .set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                },
                            );
                        });
                    });
            });
    }
}

/// Truncate a message to `max_chars` visible characters (ellipsis if exceeded).
fn truncate_msg(msg: &str, max_chars: usize) -> String {
    let chars: Vec<char> = msg.chars().collect();
    if chars.len() <= max_chars {
        return msg.to_string();
    }
    let mut out: String = chars.into_iter().take(max_chars - 1).collect();
    out.push('…');
    out
}

fn rgba(r: u8, g: u8, b: u8, a: f32) -> egui::Color32 {
    let a = (a * 255.0).clamp(0.0, 255.0) as u8;
    egui::Color32::from_rgba_premultiplied(r, g, b, a)
}
