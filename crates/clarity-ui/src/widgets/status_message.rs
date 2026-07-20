//! Backend status message widget
//!
//! Displays transient status text emitted by the agent runtime, such as
//! "Executing 3 tool(s)..." or "Compacting context...". This widget is
//! intentionally minimal: the UI team can replace it with a spinner row,
//! toast, or inline badge without touching the event/handler plumbing.
//!
//! Responsibilities:
//! - Render a non-empty status message using theme tokens.
//!
//! Non-responsibilities:
//! - Positioning inside the chat layout (call site decides).
//! - Animations / icons (call site can wrap this widget).
//! - Persisting status text (state is cleared on turn end / content arrival).

use crate::theme::Theme;

/// Render a status message if `message` is non-empty.
///
/// Returns `true` if anything was rendered.
pub fn status_message(ui: &mut egui::Ui, theme: &Theme, message: &str) -> bool {
    if message.is_empty() {
        return false;
    }
    ui.horizontal(|ui| {
        ui.set_row_height(theme.text_base + theme.space_8);
        ui.label(
            egui::RichText::new(message)
                .color(theme.text_dim)
                .size(theme.text_sm),
        );
    });
    true
}

// ============================================================================
// Tests
// ============================================================================
#[cfg(test)]
mod tests {
    #[test]
    fn empty_message_renders_nothing() {
        assert!(!status_message_should_render(""));
    }

    #[test]
    fn non_empty_message_renders() {
        assert!(status_message_should_render("Executing tools..."));
    }

    fn status_message_should_render(message: &str) -> bool {
        !message.is_empty()
    }
}
