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
//! shortcut layer and the `CommandPalette` (see [`crate::widgets::command_palette`])
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
    /// Toggle the layout debug overlay (green/blue/red/yellow diagnostic rects).
    ToggleLayoutDebug,
    /// Increase UI font scale (Ctrl + Plus / Equals).
    IncreaseFontScale,
    /// Decrease UI font scale (Ctrl + Minus).
    DecreaseFontScale,
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
    /// Toggle the Console right-rail panel (`Ctrl+Grave`).
    ToggleConsole,
    /// Toggle the Files right-rail panel (`Ctrl+Shift+F`).
    ToggleFiles,
    /// Toggle the Share right-rail panel (`Ctrl+Shift+S`).
    ToggleShare,
    /// Show the keyboard shortcuts reference.
    ShowShortcuts,
    /// Jump to the bottom of the chat stream.
    ScrollToBottom,
    /// Select the previous chat message (`ArrowUp`).
    NavigateMessageUp,
    /// Select the next chat message (`ArrowDown`).
    NavigateMessageDown,
    /// Copy the selected chat message (`Ctrl+C` with an active selection).
    CopySelectedMessage,
    /// Inline-edit the selected chat message (`E`, user messages only).
    EditSelectedMessage,
    /// Regenerate from the selected chat message (`R`, agent messages only).
    RegenerateSelectedMessage,
    /// Clear the keyboard message selection (`Escape`).
    ClearMessageSelection,
}

impl ShortcutAction {
    /// Human-readable keybinding string (e.g. "Ctrl+N").
    pub fn keybinding(&self) -> &'static str {
        match self {
            ShortcutAction::NewSession => "Ctrl+N",
            ShortcutAction::StopGeneration => "Ctrl+C",
            ShortcutAction::SendMessage => "Ctrl+Enter",
            ShortcutAction::CloseModal => "Escape",
            ShortcutAction::ToggleSkillPanel => "Ctrl+.",
            ShortcutAction::ToggleTeamPanel => "Ctrl+Shift+T",
            ShortcutAction::FocusInput => "Ctrl+K",
            ShortcutAction::ToggleCommandPalette => "Ctrl+Shift+P",
            ShortcutAction::ToggleDashboardPanel => "Ctrl+Shift+D",
            ShortcutAction::ToggleLayoutDebug => "Ctrl+Shift+L",
            ShortcutAction::IncreaseFontScale => "Ctrl+=",
            ShortcutAction::DecreaseFontScale => "Ctrl+-",
            ShortcutAction::NavigateDown => "j",
            ShortcutAction::NavigateUp => "k",
            ShortcutAction::NavigateTop => "g",
            ShortcutAction::NavigateBottom => "G",
            ShortcutAction::CopyLine => "y",
            ShortcutAction::ToggleConsole => "Ctrl+`",
            ShortcutAction::ToggleFiles => "Ctrl+Shift+F",
            ShortcutAction::ToggleShare => "Ctrl+Shift+S",
            ShortcutAction::ShowShortcuts => "Ctrl+/",
            ShortcutAction::ScrollToBottom => "End",
            ShortcutAction::NavigateMessageUp => "↑",
            ShortcutAction::NavigateMessageDown => "↓",
            ShortcutAction::CopySelectedMessage => "Ctrl+C",
            ShortcutAction::EditSelectedMessage => "E",
            ShortcutAction::RegenerateSelectedMessage => "R",
            ShortcutAction::ClearMessageSelection => "Esc",
        }
    }

    /// One-line description of what this shortcut does.
    pub fn description(&self) -> &'static str {
        match self {
            ShortcutAction::NewSession => "New chat session",
            ShortcutAction::StopGeneration => "Stop agent generation",
            ShortcutAction::SendMessage => "Send message",
            ShortcutAction::CloseModal => "Close current modal / panel",
            ShortcutAction::ToggleSkillPanel => "Toggle skill panel",
            ShortcutAction::ToggleTeamPanel => "Toggle team panel",
            ShortcutAction::FocusInput => "Focus chat input",
            ShortcutAction::ToggleCommandPalette => "Open command palette",
            ShortcutAction::ToggleDashboardPanel => "Toggle dashboard",
            ShortcutAction::ToggleLayoutDebug => "Toggle layout debug overlay",
            ShortcutAction::IncreaseFontScale => "Increase font size",
            ShortcutAction::DecreaseFontScale => "Decrease font size",
            ShortcutAction::NavigateDown => "Line down (line-mode)",
            ShortcutAction::NavigateUp => "Line up (line-mode)",
            ShortcutAction::NavigateTop => "Jump to top (line-mode)",
            ShortcutAction::NavigateBottom => "Jump to bottom (line-mode)",
            ShortcutAction::CopyLine => "Copy selected line",
            ShortcutAction::ToggleConsole => "Toggle console panel",
            ShortcutAction::ToggleFiles => "Toggle files panel",
            ShortcutAction::ToggleShare => "Toggle share panel",
            ShortcutAction::ShowShortcuts => "Show this reference",
            ShortcutAction::ScrollToBottom => "Scroll to bottom of chat",
            ShortcutAction::NavigateMessageUp => "Select previous message",
            ShortcutAction::NavigateMessageDown => "Select next message",
            ShortcutAction::CopySelectedMessage => "Copy selected message",
            ShortcutAction::EditSelectedMessage => "Edit selected message",
            ShortcutAction::RegenerateSelectedMessage => "Regenerate selected message",
            ShortcutAction::ClearMessageSelection => "Clear message selection",
        }
    }
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
            ShortcutAction::ToggleLayoutDebug => ids::TOGGLE_LAYOUT_DEBUG,
            ShortcutAction::IncreaseFontScale => ids::INCREASE_FONT_SCALE,
            ShortcutAction::DecreaseFontScale => ids::DECREASE_FONT_SCALE,
            ShortcutAction::NavigateDown => ids::NAVIGATE_DOWN,
            ShortcutAction::NavigateUp => ids::NAVIGATE_UP,
            ShortcutAction::NavigateTop => ids::NAVIGATE_TOP,
            ShortcutAction::NavigateBottom => ids::NAVIGATE_BOTTOM,
            ShortcutAction::CopyLine => ids::COPY_LINE,
            ShortcutAction::ToggleConsole => ids::TOGGLE_CONSOLE,
            ShortcutAction::ToggleFiles => ids::TOGGLE_FILES,
            ShortcutAction::ToggleShare => ids::TOGGLE_SHARE,
            ShortcutAction::ShowShortcuts => ids::SHOW_SHORTCUTS,
            ShortcutAction::ScrollToBottom => ids::SCROLL_TO_BOTTOM,
            ShortcutAction::NavigateMessageUp => ids::NAVIGATE_MESSAGE_UP,
            ShortcutAction::NavigateMessageDown => ids::NAVIGATE_MESSAGE_DOWN,
            ShortcutAction::CopySelectedMessage => ids::COPY_SELECTED_MESSAGE,
            ShortcutAction::EditSelectedMessage => ids::EDIT_SELECTED_MESSAGE,
            ShortcutAction::RegenerateSelectedMessage => ids::REGENERATE_SELECTED_MESSAGE,
            ShortcutAction::ClearMessageSelection => ids::CLEAR_MESSAGE_SELECTION,
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
        // A2: with an active keyboard message selection, Escape clears the
        // selection first instead of navigating back (same layering as the
        // find bar, which closes before CloseModal).
        if !modal_open
            && app.current_main() == &clarity_core::ui::AppView::Chat
            && app.context.ui_store.selected_message_idx.is_some()
        {
            actions.push(ShortcutAction::ClearMessageSelection);
        } else {
            actions.push(ShortcutAction::CloseModal);
        }
    }

    if app.view_state.turn == clarity_core::ui::TurnState::Loading
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

    if ctx.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl)
        && !app.chat_store().input.trim().is_empty()
        && app.view_state.turn != clarity_core::ui::TurnState::Loading
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

    if ctx.input(|i| i.key_pressed(egui::Key::L) && i.modifiers.ctrl && i.modifiers.shift) {
        actions.push(ShortcutAction::ToggleLayoutDebug);
    }

    // ── Right-rail panel shortcuts ──
    if ctx.input(|i| i.key_pressed(egui::Key::Backtick) && i.modifiers.ctrl) {
        actions.push(ShortcutAction::ToggleConsole);
    }
    if ctx.input(|i| i.key_pressed(egui::Key::F) && i.modifiers.ctrl && i.modifiers.shift) {
        actions.push(ShortcutAction::ToggleFiles);
    }
    if ctx.input(|i| i.key_pressed(egui::Key::S) && i.modifiers.ctrl && i.modifiers.shift) {
        actions.push(ShortcutAction::ToggleShare);
    }

    // Use Equals (not Plus) for zoom-in: on most layouts the `=`/`+` key is the
    // same physical key, and egui may report both symbols for one event, causing
    // a single Ctrl++ press to trigger twice when both Plus and Equals are bound.
    if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Equals)) {
        actions.push(ShortcutAction::IncreaseFontScale);
    }

    if ctx.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::Minus)) {
        actions.push(ShortcutAction::DecreaseFontScale);
    }

    // Keyboard shortcuts reference
    if ctx.input(|i| i.key_pressed(egui::Key::Slash) && i.modifiers.ctrl) {
        actions.push(ShortcutAction::ShowShortcuts);
    }

    // Jump to the latest chat message when the chat view is active.
    if app.current_main() == &clarity_core::ui::AppView::Chat
        && ctx.input(|i| i.key_pressed(egui::Key::End))
    {
        actions.push(ShortcutAction::ScrollToBottom);
    }

    // ── Message-level keyboard navigation (A2) ──
    // Focus guard: never hijack keys while any widget owns keyboard input
    // (composer, inline-edit bubble, palette / picker search fields). The
    // command palette and find bar also handle arrows themselves, so they
    // block message navigation while open.
    let keyboard_free = !ctx.egui_wants_keyboard_input()
        && !app.command_palette.open
        && !app.chat_store().find_open;
    if app.current_main() == &clarity_core::ui::AppView::Chat && keyboard_free {
        let no_modifiers = ctx.input(|i| i.modifiers.is_none());
        if no_modifiers && ctx.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
            actions.push(ShortcutAction::NavigateMessageUp);
        }
        if no_modifiers && ctx.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
            actions.push(ShortcutAction::NavigateMessageDown);
        }
        let has_selection = app.context.ui_store.selected_message_idx.is_some();
        if has_selection && no_modifiers && ctx.input(|i| i.key_pressed(egui::Key::E)) {
            actions.push(ShortcutAction::EditSelectedMessage);
        }
        if has_selection
            && no_modifiers
            && ctx.input(|i| i.key_pressed(egui::Key::R))
            && app.view_state.turn != clarity_core::ui::TurnState::Loading
        {
            actions.push(ShortcutAction::RegenerateSelectedMessage);
        }
        // Ctrl+C copies the selection only when idle — while a turn is
        // loading Ctrl+C stays bound to StopGeneration (handled above).
        // ponytail: mouse text-selection inside bubbles is not detectable
        // here; keyboard selection wins when both exist.
        if has_selection
            && app.view_state.turn != clarity_core::ui::TurnState::Loading
            && ctx.input(|i| i.key_pressed(egui::Key::C) && i.modifiers.ctrl)
        {
            actions.push(ShortcutAction::CopySelectedMessage);
        }
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
        if chat_focused && app.view_state.turn != clarity_core::ui::TurnState::Loading {
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
pub fn is_modal_open(app: &App) -> bool {
    !app.context.ui_store.pending_approvals.is_empty()
        || app.current_main() == &clarity_core::ui::AppView::Settings
        || app.current_modal() == Some(&clarity_core::ui::ModalType::TeamCreate)
        || app.current_modal() == Some(&clarity_core::ui::ModalType::CronCreate)
        || app.current_modal() == Some(&clarity_core::ui::ModalType::Snapshot)
        || app.current_modal() == Some(&clarity_core::ui::ModalType::TaskCreate)
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
        // Tracked: github.com/juice094/clarity/issues — implement once App test-harness is available.
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
            (ShortcutAction::ToggleLayoutDebug, ids::TOGGLE_LAYOUT_DEBUG),
            (ShortcutAction::ToggleConsole, ids::TOGGLE_CONSOLE),
            (ShortcutAction::ToggleFiles, ids::TOGGLE_FILES),
            (ShortcutAction::ToggleShare, ids::TOGGLE_SHARE),
            (ShortcutAction::IncreaseFontScale, ids::INCREASE_FONT_SCALE),
            (ShortcutAction::DecreaseFontScale, ids::DECREASE_FONT_SCALE),
            (ShortcutAction::NavigateDown, ids::NAVIGATE_DOWN),
            (ShortcutAction::NavigateUp, ids::NAVIGATE_UP),
            (ShortcutAction::NavigateTop, ids::NAVIGATE_TOP),
            (ShortcutAction::NavigateBottom, ids::NAVIGATE_BOTTOM),
            (ShortcutAction::CopyLine, ids::COPY_LINE),
            (ShortcutAction::ScrollToBottom, ids::SCROLL_TO_BOTTOM),
            (ShortcutAction::NavigateMessageUp, ids::NAVIGATE_MESSAGE_UP),
            (
                ShortcutAction::NavigateMessageDown,
                ids::NAVIGATE_MESSAGE_DOWN,
            ),
            (
                ShortcutAction::CopySelectedMessage,
                ids::COPY_SELECTED_MESSAGE,
            ),
            (
                ShortcutAction::EditSelectedMessage,
                ids::EDIT_SELECTED_MESSAGE,
            ),
            (
                ShortcutAction::RegenerateSelectedMessage,
                ids::REGENERATE_SELECTED_MESSAGE,
            ),
            (
                ShortcutAction::ClearMessageSelection,
                ids::CLEAR_MESSAGE_SELECTION,
            ),
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

    #[test]
    fn show_shortcuts_uses_local_command_id() {
        // ShowShortcuts only opens the in-app help modal and is not a shared
        // core command, so it intentionally stays a local kebab-case id.
        assert_eq!(ShortcutAction::ShowShortcuts.command_id(), "show-shortcuts");
    }
}
