//! Protocol-compliant modal dialog container.
//!
//! Replaces `egui::Window::new` for dialogs. Uses an explicit `Area` with
//! `Order::Foreground` so the position and layer are deterministic, and
//! applies the `Elevation::Modal` surface treatment.
//!
//! # Positioning
//!
//! The modal is centered on screen. Because egui immediate mode does not know
//! content height before layout, the previous frame's size is cached in egui
//! memory and used to center the current frame. This converges to a true
//! center after one frame.

use crate::design_system::Elevation;

/// Where the modal should be anchored relative to the viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModalAnchor {
    /// Center of the screen (default).
    Center,
    /// Top-center, offset from the top by a fixed number of pixels.
    TopCenter { offset_y: f32 },
}

/// A blocking modal dialog.
///
/// ```rust,ignore
/// Modal::new("create_task")
///     .width(420.0)
///     .show(ctx, |ui| {
///         ui.heading("New Task");
///     });
/// ```
pub struct Modal {
    id: egui::Id,
    anchor: ModalAnchor,
    width: f32,
    max_height: Option<f32>,
}

impl Modal {
    /// Create a modal with a stable id.
    pub fn new(id: impl std::hash::Hash + std::fmt::Debug) -> Self {
        Self {
            id: egui::Id::new(id),
            anchor: ModalAnchor::Center,
            width: 420.0,
            max_height: None,
        }
    }

    /// Set the modal width. Clamped to viewport minus margin.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Limit the modal height and make content scrollable.
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Anchor the modal at the top-center of the screen.
    pub fn top_center(mut self, offset_y: f32) -> Self {
        self.anchor = ModalAnchor::TopCenter { offset_y };
        self
    }

    fn size_key(&self) -> egui::Id {
        self.id.with("__last_size")
    }

    fn desired_width(&self, screen: egui::Rect, t: &crate::theme::Theme) -> f32 {
        let margin = t.space_32;
        self.width.min(screen.width() - 2.0 * margin).max(240.0)
    }

    fn compute_pos(
        &self,
        screen: egui::Rect,
        width: f32,
        last_size: Option<egui::Vec2>,
    ) -> egui::Pos2 {
        match self.anchor {
            ModalAnchor::Center => {
                // Center using the previous frame's *total* size (frame margins
                // included). Using `width` here would offset the modal right by
                // the left frame margin because `width` is the content width.
                let fallback_h = 200.0;
                let w = last_size.map(|s| s.x).unwrap_or(width);
                let h = last_size.map(|s| s.y).unwrap_or(fallback_h);
                screen.center() - egui::vec2(w * 0.5, h * 0.5)
            }
            ModalAnchor::TopCenter { offset_y } => {
                egui::pos2(screen.center().x - width * 0.5, screen.min.y + offset_y)
            }
        }
    }

    /// Show the modal. Returns the inner result from `add_contents`.
    pub fn show<R>(
        self,
        ctx: &egui::Context,
        add_contents: impl FnOnce(&mut egui::Ui) -> R,
    ) -> egui::InnerResponse<R> {
        let t = crate::design_system::theme(ctx);
        let screen = ctx.input(|i| i.viewport_rect());
        let width = self.desired_width(screen, &t);
        let last_size: Option<egui::Vec2> = ctx.memory(|mem| mem.data.get_temp(self.size_key()));
        let pos = self.compute_pos(screen, width, last_size);

        egui::Area::new(self.id)
            .fixed_pos(pos)
            .order(egui::Order::Foreground)
            .interactable(true)
            .show(ctx, |ui| {
                ui.set_min_width(width);
                ui.set_max_width(width);

                let result = Elevation::Modal.frame(&t).show(ui, |ui| {
                    if let Some(max_h) = self.max_height {
                        egui::ScrollArea::vertical()
                            .max_height(max_h)
                            .show(ui, add_contents)
                            .inner
                    } else {
                        add_contents(ui)
                    }
                });

                // Cache size for next frame's centering calculation.
                let size = ui.min_rect().size();
                ctx.memory_mut(|mem| mem.data.insert_temp(self.size_key(), size));

                result.inner
            })
    }
}

/// Render a full-screen scrim behind a modal.
///
/// Call *before* `Modal::show` so the scrim sits on the layer just below the
/// modal. Clicks and Tab/Shift+Tab on the scrim are absorbed so focus stays
/// inside the modal.
pub fn modal_scrim(ctx: &egui::Context) -> egui::Response {
    let t = crate::design_system::theme(ctx);
    let screen = ctx.input(|i| i.viewport_rect());
    egui::Area::new(egui::Id::new("clarity_modal_scrim"))
        .fixed_pos(screen.min)
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            // Absorb Tab so keyboard navigation cannot cycle back into the
            // background panels.
            if ui.input(|i| i.key_pressed(egui::Key::Tab)) {
                ui.ctx().memory_mut(|mem| mem.stop_text_input());
            }

            let (_, response) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            ui.painter()
                .rect_filled(screen, egui::CornerRadius::ZERO, t.overlay);
            response
        })
        .inner
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn modal_constructs_with_defaults() {
        let _ = Modal::new("test_modal");
        let _ = Modal::new("test_modal").width(360.0).top_center(64.0);
    }

    #[test]
    fn modal_shows_without_panic() {
        let ctx = egui::Context::default();
        crate::theme::setup_fonts(&ctx);
        let mut result = None;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(800.0, 600.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run_ui(input, |ui| {
            let response = Modal::new("test_modal").width(300.0).show(ui.ctx(), |ui| {
                ui.label("Hello");
                42
            });
            result = Some(response.inner);
        });
        assert_eq!(result, Some(42));
    }

    #[test]
    fn modal_scrim_renders_without_panic() {
        let ctx = egui::Context::default();
        crate::theme::setup_fonts(&ctx);
        modal_scrim(&ctx);
    }
}
