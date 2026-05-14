//! Global keyboard shortcut system for clarity-egui.
//!
//! Provides a centralised, testable registry of shortcuts that maps
//! raw egui input events to semantic [`ShortcutAction`]s.  The caller
//! (`App::update`) is responsible for applying the actions so that
//! shortcut handling stays decoupled from business logic.
//!
//! Since P0.5.C.1, every [`ShortcutAction`] also carries a stable
//! [`command_id`](ShortcutAction::command_id) string from
//! `clarity_core::ui::ids`. This is the shared key between the keyboard
//! shortcut layer and the [`CommandPalette`](crate::widgets::CommandPalette)
//! — both route through `App::dispatch_command(&str)`.

use crate::App;
use clarity_core::ui::ids;

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
    /// Line-mode: move cursor down one line (`j`).
    #[allow(dead_code)] // only constructed when line-mode feature is enabled
    NavigateDown,
    /// Line-mode: move cursor up one line (`k`).
    #[allow(dead_code)]
    NavigateUp,
    /// Line-mode: jump to first line (`g`).
    #[allow(dead_code)]
    NavigateTop,
    /// Line-mode: jump to last line (`G`).
    #[allow(dead_code)]
    NavigateBottom,
    /// Line-mode: copy selected line text (`y` or `Ctrl+C`).
    CopyLine,
}

impl ShortcutAction {
    /// Stable kebab-case identifier shared with the CommandPalette.
    ///
    /// All values resolve to constants in [`clarity_core::ui::ids`].
    pub fn command_id(&self) -> &'static str {
        match self {
            ShortcutAction::NewSession => ids::NEW_SESSION,
            ShortcutAction::StopGeneration => ids::STOP_GENERATION,
            ShortcutAction::SendMessage => ids::SEND_MESSAGE,
            ShortcutAction::CloseModal => ids::CLOSE_MODAL,
            ShortcutAction::ToggleSkillPanel => ids::TOGGLE_SKILL_PANEL,
            ShortcutAction::ToggleTeamPanel => ids::TOGGLE_TEAM_PANEL,
            ShortcutAction::FocusInput => ids::FOCUS_INPUT,
            ShortcutAction::ToggleCommandPalette => ids::TOGGLE_COMMAND_PALETTE,
            ShortcutAction::ToggleDashboardPanel => ids::TOGGLE_DASHBOARD,
            ShortcutAction::NavigateDown => ids::NAVIGATE_DOWN,
            ShortcutAction::NavigateUp => ids::NAVIGATE_UP,
            ShortcutAction::NavigateTop => ids::NAVIGATE_TOP,
            ShortcutAction::NavigateBottom => ids::NAVIGATE_BOTTOM,
            ShortcutAction::CopyLine => ids::COPY_LINE,
        }
    }
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

    if app.chat_store.is_loading && ctx.input(|i| i.key_pressed(egui::Key::C) && i.modifiers.ctrl) {
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

    if ctx.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl)
        && !app.chat_store.input.trim().is_empty()
        && !app.chat_store.is_loading
    {
        actions.push(ShortcutAction::SendMessage);
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

    // ── Line-mode navigation (S7 Phase 2D) ──
    #[cfg(feature = "line-mode")]
    {
        let chat_focused = matches!(
            app.view_state.focus,
            clarity_core::ui::view_state::FocusScope::Panel(
                clarity_core::ui::view_state::PanelKind::ChatStream
            )
        );
        if chat_focused && !app.chat_store.is_loading {
            if ctx.input(|i| i.key_pressed(egui::Key::J)) {
                actions.push(ShortcutAction::NavigateDown);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::K)) {
                actions.push(ShortcutAction::NavigateUp);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::G) && !i.modifiers.shift) {
                actions.push(ShortcutAction::NavigateTop);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::G) && i.modifiers.shift) {
                actions.push(ShortcutAction::NavigateBottom);
            }
            if ctx.input(|i| i.key_pressed(egui::Key::Y)) {
                actions.push(ShortcutAction::CopyLine);
            }
        }
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
    use clarity_core::ui::ids;

    #[test]
    fn test_is_modal_open_approval_blocks() {
        // Placeholder: real test needs App construction helper.
        // This documents the expected behaviour.
        // TODO: implement once App test-harness is available.
    }

    /// Every [`ShortcutAction`] variant must resolve to a non-empty,
    /// kebab-case command id matching a constant in
    /// [`clarity_core::ui::ids`]. This guards against typo drift between
    /// the shortcut layer and the command palette.
    #[test]
    fn shortcut_action_command_id_matches_ids_module() {
        let all = [
            (ShortcutAction::NewSession, ids::NEW_SESSION),
            (ShortcutAction::StopGeneration, ids::STOP_GENERATION),
            (ShortcutAction::SendMessage, ids::SEND_MESSAGE),
            (ShortcutAction::CloseModal, ids::CLOSE_MODAL),
            (ShortcutAction::ToggleSkillPanel, ids::TOGGLE_SKILL_PANEL),
            (ShortcutAction::ToggleTeamPanel, ids::TOGGLE_TEAM_PANEL),
            (ShortcutAction::FocusInput, ids::FOCUS_INPUT),
            (
                ShortcutAction::ToggleCommandPalette,
                ids::TOGGLE_COMMAND_PALETTE,
            ),
            (ShortcutAction::ToggleDashboardPanel, ids::TOGGLE_DASHBOARD),
        ];
        for (action, expected) in all {
            assert_eq!(
                action.command_id(),
                expected,
                "ShortcutAction::{:?} should resolve to ids::{}",
                action,
                expected
            );
            assert!(
                !action.command_id().is_empty(),
                "command_id must be non-empty"
            );
            assert!(
                action
                    .command_id()
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-'),
                "command_id must be kebab-case: {}",
                action.command_id()
            );
        }
    }
}
