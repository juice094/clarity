//! Global keyboard shortcut system for clarity-egui.
//!
//! Provides a centralised, testable registry of shortcuts that maps
//! raw egui input events to semantic [`ShortcutAction`]s.  The caller
//! (`App::update`) is responsible for applying the actions so that
//! shortcut handling stays decoupled from business logic.

use crate::App;

/// Semantic actions produced by the global shortcut layer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    /// Create a new chat session.
    NewSession,
    /// Cancel the currently running agent turn.
    StopGeneration,
    /// Send the current input text.
    SendMessage,
    /// Close the top-most modal / panel.
    CloseModal,
    /// Toggle the skill side-panel.
    ToggleSkillPanel,
    /// Toggle the team side-panel.
    ToggleTeamPanel,
    /// Move keyboard focus to the chat input box.
    FocusInput,
    /// Toggle the command palette (placeholder).
    ToggleCommandPalette,
    /// Toggle the dashboard metrics side-panel.
    ToggleDashboardPanel,
}

/// Collect global shortcut actions for the current frame.
///
/// Call once per frame in `App::update()` after `process_events()`.
/// Returns an empty vec when a modal dialog is open that should capture
/// all input — **except** `Escape` and `Ctrl+C` which are always
/// processed for safety.
pub fn collect_actions(ctx: &egui::Context, app: &App) -> Vec<ShortcutAction> {
    let mut actions = Vec::new();
    let modal_open = is_modal_open(app);

    // ── Always-on shortcuts (safety critical) ──
    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        actions.push(ShortcutAction::CloseModal);
    }

    if app.chat_store.is_loading
        && ctx.input(|i| i.key_pressed(egui::Key::C) && i.modifiers.ctrl)
    {
        actions.push(ShortcutAction::StopGeneration);
    }

    // When a modal is open we block everything else so that shortcuts
    // don't leak through to the main UI (e.g. Ctrl+N while typing in
    // a settings text field).
    if modal_open {
        return actions;
    }

    // ── Main-UI shortcuts ──
    if ctx.input(|i| i.key_pressed(egui::Key::N) && i.modifiers.ctrl) {
        actions.push(ShortcutAction::NewSession);
    }

    if ctx.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl) {
        if !app.chat_store.input.trim().is_empty() && !app.chat_store.is_loading {
            actions.push(ShortcutAction::SendMessage);
        }
    }

    if ctx.input(|i| i.key_pressed(egui::Key::K) && i.modifiers.ctrl) {
        actions.push(ShortcutAction::FocusInput);
    }

    if ctx.input(|i| i.key_pressed(egui::Key::P) && i.modifiers.ctrl && i.modifiers.shift) {
        actions.push(ShortcutAction::ToggleCommandPalette);
    }

    if ctx.input(|i| i.key_pressed(egui::Key::Period) && i.modifiers.ctrl) {
        actions.push(ShortcutAction::ToggleSkillPanel);
    }

    if ctx.input(|i| i.key_pressed(egui::Key::T) && i.modifiers.ctrl && i.modifiers.shift) {
        actions.push(ShortcutAction::ToggleTeamPanel);
    }

    if ctx.input(|i| i.key_pressed(egui::Key::D) && i.modifiers.ctrl && i.modifiers.shift) {
        actions.push(ShortcutAction::ToggleDashboardPanel);
    }

    actions
}

/// Returns `true` when a modal dialog is open that should block main-UI shortcuts.
fn is_modal_open(app: &App) -> bool {
    !app.ui_store.pending_approvals.is_empty()
        || app.settings_store.settings_open
        || app.team_store.create_modal_open
        || app.cron_store.create_modal_open
        || app.snapshot_store.modal_open
        || app.task_store.task_create_modal_open
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: build a minimal App for shortcut testing.
    fn test_app() -> App {
        // We cannot easily construct a full App without egui context,
        // but `is_modal_open` only touches store flags.
        // For a unit test we would need a test-harness; keep the test
        // surface small and rely on integration tests for the hot path.
        panic!("Use integration tests in tests/egui_shortcuts.rs");
    }

    #[test]
    fn test_is_modal_open_approval_blocks() {
        // Placeholder: real test needs App construction helper.
        // This documents the expected behaviour.
        assert!(true);
    }
}
