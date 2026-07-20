//! Protocol-compliant floating overlay panel.
//!
//! Overlays are non-blocking floating surfaces that sit above the main content
//! but below modals and toasts. They use an explicit `Area` with
//! `Order::Foreground` so their position and layer are deterministic.
//!
//! Unlike `Modal`, an overlay does not imply a scrim or block the rest of the
//! UI. Overlays that need a dimmer can call `overlay_scrim` before `Overlay::show`.

use crate::design_system::Elevation;

/// Where the overlay should be anchored relative to the viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum OverlayAnchor {
    /// Center of the screen.
    Center,
    /// Top-center, offset from the top by a fixed number of pixels.
    TopCenter { offset_y: f32 },
}

/// A non-blocking floating overlay panel.
///
/// ```rust,ignore
/// Overlay::new("command_palette")
///     .width(420.0)
///     .top_center(64.0)
///     .show(ctx, |ui| {
///         ui.label("Hello");
///     });
/// ```
pub struct Overlay {
    id: egui::Id,
    anchor: OverlayAnchor,
    width: f32,
    max_height: Option<f32>,
}

impl Overlay {
    /// Create an overlay with a stable id.
    pub fn new(id: impl std::hash::Hash + std::fmt::Debug) -> Self {
        Self {
            id: egui::Id::new(id),
            anchor: OverlayAnchor::Center,
            width: 420.0,
            max_height: None,
        }
    }

    /// Set the overlay width. Clamped to viewport minus margin.
    pub fn width(mut self, width: f32) -> Self {
        self.width = width;
        self
    }

    /// Limit the overlay height and make content scrollable.
    pub fn max_height(mut self, height: f32) -> Self {
        self.max_height = Some(height);
        self
    }

    /// Anchor the overlay at the top-center of the screen.
    pub fn top_center(mut self, offset_y: f32) -> Self {
        self.anchor = OverlayAnchor::TopCenter { offset_y };
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
            OverlayAnchor::Center => {
                // Center using the previous frame's *total* size (frame margins
                // included). Using `width` here would offset the overlay right by
                // the left frame margin because `width` is the content width.
                let fallback_h = 200.0;
                let w = last_size.map(|s| s.x).unwrap_or(width);
                let h = last_size.map(|s| s.y).unwrap_or(fallback_h);
                screen.center() - egui::vec2(w * 0.5, h * 0.5)
            }
            OverlayAnchor::TopCenter { offset_y } => {
                egui::pos2(screen.center().x - width * 0.5, screen.min.y + offset_y)
            }
        }
    }

    /// Show the overlay. Returns the inner result from `add_contents`.
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

                let result = Elevation::Overlay
                    .frame(&t)
                    .show(ui, |ui| {
                        if let Some(max_h) = self.max_height {
                            egui::ScrollArea::vertical()
                                .max_height(max_h)
                                .show(ui, add_contents)
                                .inner
                        } else {
                            add_contents(ui)
                        }
                    })
                    .inner;

                // Cache size for next frame's centering calculation.
                let size = ui.min_rect().size();
                ctx.memory_mut(|mem| mem.data.insert_temp(self.size_key(), size));

                result
            })
    }
}

/// Render a full-screen scrim behind an overlay.
///
/// Use this for overlays that need to dim the background (e.g. skill/mcp
/// panels). Call *before* `Overlay::show`. Returns the scrim response so
/// callers can detect outside clicks and dismiss the overlay.
pub fn overlay_scrim(ctx: &egui::Context) -> egui::Response {
    let t = crate::design_system::theme(ctx);
    let screen = ctx.input(|i| i.viewport_rect());
    egui::Area::new(egui::Id::new("clarity_overlay_scrim"))
        .fixed_pos(screen.min)
        .order(egui::Order::Foreground)
        .interactable(true)
        .show(ctx, |ui| {
            let (_, response) = ui.allocate_exact_size(screen.size(), egui::Sense::click());
            ui.painter()
                .rect_filled(screen, egui::CornerRadius::ZERO, t.overlay);
            response
        })
        .inner
}
