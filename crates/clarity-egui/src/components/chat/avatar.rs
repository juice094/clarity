use crate::theme::Theme;

/// Circular avatar with a single-letter label.
///
/// Delegates to the idiomatic `widgets::avatar` implementation; kept as a
/// compatibility re-export for existing call sites during the module reorganization.
pub fn avatar(ui: &mut egui::Ui, label: &str, theme: &Theme) -> egui::Response {
    crate::widgets::avatar::avatar(ui, label, theme, None, None)
}
