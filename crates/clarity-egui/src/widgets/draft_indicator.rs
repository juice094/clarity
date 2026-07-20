//! Draft indicator widget
//!
//! Renders the transient model state ("thinking...", reasoning content, etc.)
//! during an agent turn. The visual design is intentionally minimal: the UI
//! team can replace this widget or wrap it without touching the event/handler
//! plumbing.
//!
//! Current responsibilities:
//! - Show a progress label while the model is preparing a response.
//! - Optionally show reasoning/thinking content (`DraftStatus::Content`).
//!
//! Non-responsibilities:
//! - Positioning inside the chat layout (call site decides).
//! - Animations / spinner graphics (call site can wrap this widget).
//! - Persisting draft content (state is cleared on `DraftClear` / turn end).

use crate::design_system::{self, TextStyle};
use crate::theme::Theme;
use crate::ui::types::DraftStatus;

/// Returns `true` if the given `DraftStatus` should produce any UI output.
///
/// This is split out so it can be unit-tested without constructing an egui `Ui`.
pub fn should_render(status: &DraftStatus) -> bool {
    match status {
        DraftStatus::None => false,
        DraftStatus::Progress { .. } => true,
        DraftStatus::Content { text } => !text.is_empty(),
    }
}

/// Render a draft indicator from the current `DraftStatus`.
///
/// Returns `true` if anything was rendered, `false` if the status is `None`.
/// Call sites can use the returned response to measure layout or attach
/// animations.
pub fn draft_indicator(ui: &mut egui::Ui, theme: &Theme, status: &DraftStatus) -> bool {
    if !should_render(status) {
        return false;
    }
    match status {
        DraftStatus::Progress { text } => render_progress(ui, theme, text),
        DraftStatus::Content { text } => render_content(ui, theme, text),
        DraftStatus::None => unreachable!("should_render prevents None"),
    }
    true
}

fn render_progress(ui: &mut egui::Ui, theme: &Theme, text: &str) {
    // Minimal placeholder using semantic tokens. Designers can replace this
    // with a spinner, icon, or styled row without changing the data contract.
    ui.horizontal(|ui| {
        ui.set_row_height(theme.text_base + theme.space_8);
        design_system::text(ui, text, TextStyle::Small);
    });
}

fn render_content(ui: &mut egui::Ui, _theme: &Theme, text: &str) {
    // Reasoning / thinking content. Currently rendered as muted text.
    // Future designs may collapse this, show it in a side panel, or style it
    // as a "<think>" block.
    design_system::surface_panel(ui, |ui| {
        design_system::text(ui, text, TextStyle::Body);
    });
}

// ============================================================================
// Tests
// ============================================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn none_status_should_not_render() {
        assert!(!should_render(&DraftStatus::None));
    }

    #[test]
    fn progress_status_should_render() {
        assert!(should_render(&DraftStatus::Progress {
            text: "thinking...".into()
        }));
    }

    #[test]
    fn empty_content_should_not_render() {
        assert!(!should_render(&DraftStatus::Content {
            text: String::new()
        }));
    }

    #[test]
    fn non_empty_content_should_render() {
        assert!(should_render(&DraftStatus::Content {
            text: "let me think...".into()
        }));
    }
}
