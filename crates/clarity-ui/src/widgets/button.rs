//! Theme-aware button component.
//!
//! Replaces raw `egui::Button::new` with a constrained set of variants and
//! sizes that follow the Clarity Design Protocol.

use crate::design_system::paint_focus_ring;
use crate::theme::Theme;

/// Button visual variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonVariant {
    /// Filled accent background, strong contrasting text. The default CTA.
    Primary,
    /// Bordered surface background, standard text. Secondary action.
    Secondary,
    /// Transparent background, text only. Toolbar/list actions.
    Ghost,
    /// Destructive action. Red fill with contrasting text.
    Danger,
    /// Destructive secondary action. Standard surface with red text.
    DangerGhost,
}

/// Button size.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ButtonSize {
    Small,
    Medium,
    Large,
}

/// Theme-aware Clarity button.
///
/// Use with `ui.add(Button::new("Save").primary())`.
pub struct Button<'a> {
    label: &'a str,
    variant: ButtonVariant,
    size: ButtonSize,
    enabled: bool,
    width: Option<f32>,
    fill: Option<egui::Color32>,
    stroke: Option<egui::Stroke>,
    text_color: Option<egui::Color32>,
    min_size: Option<egui::Vec2>,
}

impl<'a> Button<'a> {
    /// Create a new button with the given label.
    pub fn new(label: &'a str) -> Self {
        Self {
            label,
            variant: ButtonVariant::Secondary,
            size: ButtonSize::Medium,
            enabled: true,
            width: None,
            fill: None,
            stroke: None,
            text_color: None,
            min_size: None,
        }
    }

    /// Primary / CTA style.
    pub fn primary(mut self) -> Self {
        self.variant = ButtonVariant::Primary;
        self
    }

    /// Secondary / bordered style.
    pub fn secondary(mut self) -> Self {
        self.variant = ButtonVariant::Secondary;
        self
    }

    /// Ghost / transparent style.
    pub fn ghost(mut self) -> Self {
        self.variant = ButtonVariant::Ghost;
        self
    }

    /// Danger / destructive filled style.
    pub fn danger(mut self) -> Self {
        self.variant = ButtonVariant::Danger;
        self
    }

    /// Danger ghost / destructive subtle style.
    pub fn danger_ghost(mut self) -> Self {
        self.variant = ButtonVariant::DangerGhost;
        self
    }

    /// Set the visual variant directly (used by theme helpers).
    pub fn variant(mut self, variant: ButtonVariant) -> Self {
        self.variant = variant;
        self
    }

    /// Small size.
    pub fn small(mut self) -> Self {
        self.size = ButtonSize::Small;
        self
    }

    /// Medium size (default).
    pub fn medium(mut self) -> Self {
        self.size = ButtonSize::Medium;
        self
    }

    /// Large size.
    pub fn large(mut self) -> Self {
        self.size = ButtonSize::Large;
        self
    }

    /// Enable or disable the button.
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Fix the button width.
    pub fn width(mut self, width: f32) -> Self {
        self.width = Some(width);
        self
    }

    /// Override the button fill colour.
    ///
    /// Use sparingly — most buttons should use a semantic variant.
    pub fn fill(mut self, fill: egui::Color32) -> Self {
        self.fill = Some(fill);
        self
    }

    /// Override the button stroke.
    ///
    /// Use sparingly — most buttons should use a semantic variant.
    pub fn stroke(mut self, stroke: egui::Stroke) -> Self {
        self.stroke = Some(stroke);
        self
    }

    /// Override the label text colour.
    ///
    /// Use sparingly — most buttons should use a semantic variant.
    pub fn text_color(mut self, color: egui::Color32) -> Self {
        self.text_color = Some(color);
        self
    }

    /// Set the minimum size of the button.
    pub fn min_size(mut self, size: egui::Vec2) -> Self {
        self.min_size = Some(size);
        self
    }

    fn height(&self, t: &Theme) -> f32 {
        match self.size {
            ButtonSize::Small => t.button_height_sm,
            ButtonSize::Medium => t.button_height_md,
            ButtonSize::Large => t.button_height_lg,
        }
    }

    fn text_size(&self, t: &Theme) -> f32 {
        match self.size {
            ButtonSize::Small => t.text_xs,
            ButtonSize::Medium => t.text_sm,
            ButtonSize::Large => t.text_base,
        }
    }

    fn h_padding(&self, t: &Theme) -> f32 {
        match self.size {
            ButtonSize::Small => t.space_8,
            ButtonSize::Medium => t.space_12,
            ButtonSize::Large => t.space_16,
        }
    }

    fn colors(&self, t: &Theme, hovered: bool) -> (egui::Color32, egui::Color32) {
        match self.variant {
            ButtonVariant::Primary => {
                let fill = if hovered { t.accent_hover } else { t.accent };
                (fill, t.nav_cta_text)
            }
            ButtonVariant::Secondary => {
                let fill = if hovered { t.bg_hover } else { t.surface };
                (fill, t.text)
            }
            ButtonVariant::Ghost => {
                let fill = if hovered {
                    t.bg_hover
                } else {
                    egui::Color32::TRANSPARENT
                };
                (fill, t.text)
            }
            ButtonVariant::Danger => {
                let base = t.danger;
                let fill = if hovered { lighten(base, 0.12) } else { base };
                (fill, egui::Color32::WHITE)
            }
            ButtonVariant::DangerGhost => {
                let fill = if hovered {
                    alpha(t.danger, 0.10)
                } else {
                    egui::Color32::TRANSPARENT
                };
                (fill, t.danger)
            }
        }
    }
}

impl egui::Widget for Button<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let t = crate::design_system::theme(ui.ctx());
        let height = self.height(&t);
        let text_size = self.text_size(&t);
        let h_pad = self.h_padding(&t);
        let min_width = self.width.unwrap_or(0.0);

        // Measure the label so the button is wide enough to hold it on one line.
        let (_, text_color) = self.colors(&t, false);
        let measure_galley = ui.painter().layout(
            self.label.to_string(),
            t.font(text_size),
            text_color,
            f32::INFINITY,
        );
        let text_width = measure_galley.size().x;
        let desired_width = min_width.max(text_width + 2.0 * h_pad);
        let desired_size = egui::vec2(desired_width, height);
        let sense = if self.enabled {
            egui::Sense::click()
        } else {
            egui::Sense::hover()
        };

        let desired_size = if let Some(min_size) = self.min_size {
            desired_size.max(min_size)
        } else {
            desired_size
        };
        let (rect, response) = ui.allocate_at_least(desired_size, sense);
        let hovered = response.hovered() && self.enabled;
        let (base_fill, base_text_color) = self.colors(&t, hovered);
        let fill = self.fill.unwrap_or(base_fill);
        let text_color = self.text_color.unwrap_or(base_text_color);
        let stroke = self.stroke.unwrap_or_else(|| match self.variant {
            ButtonVariant::Secondary => egui::Stroke::new(1.0, t.border),
            _ => egui::Stroke::NONE,
        });
        let radius = egui::CornerRadius::same(t.radius_sm as u8);

        if ui.is_rect_visible(rect) {
            ui.painter().rect_filled(rect, radius, fill);
            if stroke.width > 0.0 {
                ui.painter()
                    .rect_stroke(rect, radius, stroke, egui::StrokeKind::Inside);
            }

            let galley = ui.painter().layout(
                self.label.to_string(),
                t.font(text_size),
                text_color,
                rect.width() - 2.0 * h_pad,
            );
            let text_pos = egui::pos2(
                rect.center().x - galley.size().x * 0.5,
                rect.center().y - galley.size().y * 0.5,
            );
            ui.painter().galley(text_pos, galley, text_color);
        }

        if response.has_focus() {
            paint_focus_ring(ui, rect, radius);
        }

        response
    }
}

/// Render a CTA button with the primary style.
pub fn primary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(Button::new(label).primary())
}

/// Render a secondary button.
pub fn secondary_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(Button::new(label).secondary())
}

/// Render a ghost button.
pub fn ghost_button(ui: &mut egui::Ui, label: &str) -> egui::Response {
    ui.add(Button::new(label).ghost())
}

fn lighten(c: egui::Color32, amount: f32) -> egui::Color32 {
    egui::Color32::from_rgb(
        ((c.r() as f32) + (255.0 - c.r() as f32) * amount).min(255.0) as u8,
        ((c.g() as f32) + (255.0 - c.g() as f32) * amount).min(255.0) as u8,
        ((c.b() as f32) + (255.0 - c.b() as f32) * amount).min(255.0) as u8,
    )
}

fn alpha(c: egui::Color32, a: f32) -> egui::Color32 {
    egui::Color32::from_rgba_premultiplied(c.r(), c.g(), c.b(), (c.a() as f32 * a) as u8)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn button_variants_construct() {
        let _ = Button::new("Save").primary();
        let _ = Button::new("Cancel").ghost();
        let _ = Button::new("Delete").danger();
        let _ = Button::new("Delete").danger_ghost();
    }

    #[test]
    fn button_allocates_space() {
        let resp = run_in_frame(|ui| ui.add(Button::new("Click me").primary()));
        assert!(resp.rect.width() > 0.0);
        assert!(resp.rect.height() > 0.0);
    }

    fn run_in_frame<R>(f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ctx = egui::Context::default();
        crate::theme::setup_fonts(&ctx);
        let mut f_opt = Some(f);
        let mut output = None;
        let input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(400.0, 800.0),
            )),
            ..Default::default()
        };
        let _ = ctx.run_ui(input, |ui| {
            egui::CentralPanel::default().show(ui, |ui| {
                if let Some(f) = f_opt.take() {
                    output = Some(f(ui));
                }
            });
        });
        output.expect("CentralPanel should always run its closure")
    }
}
