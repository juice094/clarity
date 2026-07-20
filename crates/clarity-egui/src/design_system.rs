//! Re-export of the semantic design-system layer previously located here.
//!
//! The canonical implementation now lives in `clarity-ui::design_system`. This
//! module preserves the old import path during the notedeck-style refactor.
//!
//! The `Panel` trait remains here because its contract is tied to the
//! clarity-egui `App` type. It will be replaced by `clarity-shell::ClarityApp`
//! in Phase 2.

pub use clarity_ui::design_system::*;

/// A renderable UI panel.
///
/// Panels are the primary unit of UI organisation in clarity-egui.
/// Implementing this trait provides a consistent signature and enables
/// future panel-registry features (discovery, ordering, keyboard
/// shortcut auto-binding).
///
/// # Implementation notes
///
/// Most panels are stateless and render from `App` state directly.
/// Use a unit struct for stateless panels, or hold transient UI state
/// (e.g. scroll position, text buffer) in the struct for stateful ones.
///
/// ```ignore
/// struct SettingsPanel;
/// impl Panel for SettingsPanel {
///     fn title(&self, _app: &App) -> &str { "Settings" }
///     fn render(&mut self, app: &mut App, ui: &mut egui::Ui) {
///         // render using app state
///     }
/// }
/// ```
pub trait Panel {
    /// Human-readable panel name (used for header title, shortcuts reference, debug).
    fn title(&self, app: &crate::App) -> &str;
    /// Render the panel into the given ui.
    fn render(&mut self, app: &mut crate::App, ui: &mut egui::Ui);
}
