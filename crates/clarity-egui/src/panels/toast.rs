use crate::App;
use std::time::{Duration, Instant};

/// Cubic ease-out: decelerates toward the end.
fn ease_out_cubic(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

pub fn render_toasts(app: &mut App, ctx: &egui::Context) {
    let now = Instant::now();
    app.ui_store
        .toasts
        .retain(|t| now.duration_since(t.created_at) < Duration::from_secs(5));
    for (i, toast) in app.ui_store.toasts.iter().enumerate() {
        let (bg, text_color) = match toast.level {
            crate::ui::types::ToastLevel::Info => {
                (app.ui_store.theme.accent, app.ui_store.theme.text)
            }
            crate::ui::types::ToastLevel::Warn => {
                (app.ui_store.theme.status_busy, app.ui_store.theme.text)
            }
            crate::ui::types::ToastLevel::Error => {
                (app.ui_store.theme.danger, app.ui_store.theme.text)
            }
        };

        // Fade-in: 0 → 1 over duration_normal with cubic ease-out.
        let elapsed = now.duration_since(toast.created_at).as_secs_f32();
        let fade = (elapsed / app.ui_store.theme.duration_normal).min(1.0);
        let alpha = ease_out_cubic(fade);

        let screen = ctx.screen_rect();
        let x = screen.max.x - 320.0;
        let y = 20.0 + i as f32 * 56.0;
        egui::Area::new(egui::Id::new(("toast", i)))
            .fixed_pos(egui::pos2(x, y))
            .show(ctx, |ui| {
                egui::Frame::group(&ctx.style())
                    .fill(bg.linear_multiply(alpha))
                    .corner_radius(egui::CornerRadius::same(app.ui_store.theme.radius_sm as u8))
                    .inner_margin(egui::Margin::same(12))
                    .show(ui, |ui| {
                        ui.set_max_width(280.0);
                        ui.label(
                            egui::RichText::new(&toast.message)
                                .color(text_color.linear_multiply(alpha))
                                .size(app.ui_store.theme.text_base),
                        );
                    });
            });
    }
}
