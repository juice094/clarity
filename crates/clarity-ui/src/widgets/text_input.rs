//! Theme-aware text input component.
//!
//! Replaces raw `egui::TextEdit::singleline/multiline` with a constrained,
//! protocol-compliant input that always uses the theme's input background,
//! border, radius, and typography tokens.

use crate::theme::Theme;

/// Theme-aware single-line or multi-line text input.
///
/// ```rust,ignore
/// ui.add(TextInput::singleline(&mut name).hint_text("Task name"));
/// ui.add_sized(
///     egui::vec2(ui.available_width(), 80.0),
///     TextInput::multiline(&mut prompt).hint_text("Agent prompt..."),
/// );
/// ```
pub struct TextInput<'a> {
    text: &'a mut String,
    multiline: bool,
    hint: Option<String>,
    desired_width: Option<f32>,
    min_height: Option<f32>,
    id_salt: Option<String>,
    id: Option<egui::Id>,
    password: bool,
    desired_rows: Option<usize>,
    transparent: bool,
    font: Option<egui::FontId>,
}

impl<'a> TextInput<'a> {
    /// Single-line input.
    pub fn singleline(text: &'a mut String) -> Self {
        Self {
            text,
            multiline: false,
            hint: None,
            desired_width: None,
            min_height: None,
            id_salt: None,
            id: None,
            password: false,
            desired_rows: None,
            transparent: false,
            font: None,
        }
    }

    /// Multi-line input.
    pub fn multiline(text: &'a mut String) -> Self {
        Self {
            text,
            multiline: true,
            hint: None,
            desired_width: None,
            min_height: None,
            id_salt: None,
            id: None,
            password: false,
            desired_rows: None,
            transparent: false,
            font: None,
        }
    }

    /// Placeholder shown when the field is empty.
    pub fn hint_text(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }

    /// Desired width for the input.
    pub fn width(mut self, width: f32) -> Self {
        self.desired_width = Some(width);
        self
    }

    /// Minimum height for multi-line inputs.
    pub fn min_height(mut self, height: f32) -> Self {
        self.min_height = Some(height);
        self
    }

    /// Provide a stable id salt to disambiguate multiple inputs in the same
    /// parent scope.
    pub fn id_salt(mut self, salt: impl Into<String>) -> Self {
        self.id_salt = Some(salt.into());
        self
    }

    /// Render as a password field.
    pub fn password(mut self, password: bool) -> Self {
        self.password = password;
        self
    }

    /// Set the number of desired rows for a multi-line input.
    pub fn desired_rows(mut self, rows: usize) -> Self {
        self.desired_rows = Some(rows);
        self
    }

    /// Set a stable egui id for the input.
    pub fn id(mut self, id: egui::Id) -> Self {
        self.id = Some(id);
        self
    }

    /// Render without the theme input frame, for embedding inside a custom
    /// card or container.
    pub fn transparent(mut self) -> Self {
        self.transparent = true;
        self
    }

    /// Override the font used for the input text and placeholder.
    ///
    /// Use this for code-like inputs that need a monospace font.
    pub fn font(mut self, font: egui::FontId) -> Self {
        self.font = Some(font);
        self
    }

    fn build(self, t: &Theme) -> egui::TextEdit<'a> {
        let frame = if self.transparent {
            egui::Frame::NONE
        } else {
            egui::Frame::new()
                .fill(t.input_bg)
                .stroke(egui::Stroke::new(1.0, t.border))
                .corner_radius(egui::CornerRadius::same(t.radius_sm as u8))
                .inner_margin(egui::Margin::symmetric(t.space_12 as i8, t.space_8 as i8))
        };

        let font_id = self.font.unwrap_or_else(|| t.font(t.text_base));
        let mut edit = if self.multiline {
            let mut edit = egui::TextEdit::multiline(self.text)
                .font(font_id)
                .frame(frame);
            if let Some(h) = self.min_height {
                edit = edit.min_size(egui::vec2(0.0, h));
            }
            if let Some(rows) = self.desired_rows {
                edit = edit.desired_rows(rows);
            }
            edit
        } else {
            egui::TextEdit::singleline(self.text)
                .font(font_id)
                .frame(frame)
        };

        if let Some(hint) = self.hint {
            edit = edit.hint_text(hint);
        }
        if let Some(w) = self.desired_width {
            edit = edit.desired_width(w);
        }
        if let Some(salt) = self.id_salt {
            edit = edit.id_salt(salt);
        }
        if let Some(id) = self.id {
            edit = edit.id(id);
        }
        if self.password {
            edit = edit.password(true);
        }

        edit
    }
}

impl egui::Widget for TextInput<'_> {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let t = crate::design_system::theme(ui.ctx());
        ui.add(self.build(&t))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_input_builds_without_panic() {
        let mut text = String::from("hello");
        let _ = TextInput::singleline(&mut text)
            .hint_text("Hint")
            .width(200.0);
        let _ = TextInput::multiline(&mut text).min_height(80.0);
    }

    #[test]
    fn text_input_allocates_space() {
        let mut text = String::new();
        let resp = run_in_frame(|ui| ui.add(TextInput::singleline(&mut text).width(200.0)));
        assert!(resp.rect.width() >= 200.0);
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
