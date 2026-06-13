//! Keyboard shortcut registry — focus-aware routing per ADR-013.
//!
//! This module provides the skeleton for `ShortcutRegistry::resolve(key, focus)`
//! which maps physical keystrokes to command identifiers based on the current
//! focus scope.  The actual frontend key-event translation (egui `Key` →
//! `KeyEvent`, crossterm `KeyCode` → `KeyEvent`) lives in the respective
//! frontend crates.
//!
//! ## Design notes
//!
//! - `KeyEvent` is a frontend-neutral representation: modifiers + key string.
//! - `FocusScope` compatibility rules are defined in `view_state.rs`.
//! - Binding resolution picks the *most specific* compatible match.
//! - Full binding table (29 entries) ships in S8 (Phase 3B); this skeleton
//!   registers a representative subset to validate the routing mechanism.

use crate::ui::view_state::FocusScope;

/// Frontend-neutral key event.
///
/// Frontends translate their native key codes into this representation
/// before calling `ShortcutRegistry::resolve`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    /// Ctrl.
    pub ctrl: bool,
    /// Shift.
    pub shift: bool,
    /// Alt.
    pub alt: bool,
    /// Skill metadata.
    pub meta: bool,
    /// Argument key.
    pub key: String,
}

impl KeyEvent {
    /// Create a new `KeyEvent`.
    pub fn new(key: impl Into<String>) -> Self {
        Self {
            ctrl: false,
            shift: false,
            alt: false,
            meta: false,
            key: key.into(),
        }
    }

    /// Set the ctrl.
    pub fn with_ctrl(mut self) -> Self {
        self.ctrl = true;
        self
    }

    /// Set the shift.
    pub fn with_shift(mut self) -> Self {
        self.shift = true;
        self
    }

    /// Set the alt.
    pub fn with_alt(mut self) -> Self {
        self.alt = true;
        self
    }

    /// Set the meta.
    pub fn with_meta(mut self) -> Self {
        self.meta = true;
        self
    }
}

/// A single binding: when `key` is pressed while `scope` is active, emit `command_id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutBinding {
    /// Argument key.
    pub key: KeyEvent,
    /// Scope.
    pub scope: FocusScope,
    /// Command id.
    pub command_id: &'static str,
}

/// Focus-aware shortcut registry.
///
/// Resolution algorithm (ADR-013 §7):
/// 1. Filter bindings whose `key` exactly matches the input.
/// 2. Keep bindings whose `scope` is compatible with current focus.
/// 3. Select the binding with highest `specificity`.
/// 4. Return its `command_id`.
#[derive(Debug, Clone, Default)]
pub struct ShortcutRegistry {
    bindings: Vec<ShortcutBinding>,
}

impl ShortcutRegistry {
    /// Create a new `ShortcutRegistry`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a binding. Later registrations of the same key+scope overwrite
    /// earlier ones (LIFO semantics per ADR-013).
    pub fn register(&mut self, key: KeyEvent, scope: FocusScope, command_id: &'static str) {
        // Remove any existing binding with identical key+scope to enforce LIFO.
        self.bindings
            .retain(|b| !(b.key == key && b.scope == scope));
        self.bindings.push(ShortcutBinding {
            key,
            scope,
            command_id,
        });
    }

    /// Resolve a key event against the current focus scope.
    ///
    /// Returns `Some(command_id)` if a binding matches, `None` otherwise.
    pub fn resolve(&self, key: &KeyEvent, focus: &FocusScope) -> Option<&'static str> {
        let mut best: Option<&ShortcutBinding> = None;
        for binding in &self.bindings {
            if &binding.key != key {
                continue;
            }
            if !binding.scope.is_compatible_with(focus) {
                continue;
            }
            if best.is_none_or(|b| binding.scope.specificity() > b.scope.specificity()) {
                best = Some(binding);
            }
        }
        best.map(|b| b.command_id)
    }

    /// Return all bindings grouped by scope — used by the `?` help overlay.
    pub fn help_entries(&self) -> Vec<&ShortcutBinding> {
        self.bindings.iter().collect()
    }
}

/// Pre-registered binding set covering the ADR-013 MVP + representative samples.
///
/// This is the *skeleton* subset; S8 (Phase 3B) expands to the full 29-binding table.
impl ShortcutRegistry {
    /// Build a registry with the P1.5.9 validation subset.
    pub fn with_defaults() -> Self {
        use crate::ui::view_state::{ModalType, PanelKind};
        use FocusScope::*;

        let mut reg = Self::new();

        // App-level global shortcuts (ADR-013 §2 direct adoption)
        reg.register(
            KeyEvent::new("p").with_ctrl().with_shift(),
            App,
            "toggle-command-palette",
        );
        reg.register(KeyEvent::new("n").with_ctrl(), App, "new-session");
        reg.register(KeyEvent::new("c").with_ctrl(), App, "cancel-input");
        reg.register(KeyEvent::new("q").with_ctrl(), Os, "quit");

        // Focus-scoped critical bindings (ADR-013 §4)
        reg.register(
            KeyEvent::new("s").with_ctrl(),
            Panel(PanelKind::RightWorkspace),
            "save-previewed-file",
        );
        reg.register(
            KeyEvent::new("j"),
            Panel(PanelKind::ChatStream),
            "navigate-down",
        );
        reg.register(
            KeyEvent::new("k"),
            Panel(PanelKind::ChatStream),
            "navigate-up",
        );

        // Modal-level bindings (ADR-013 §2)
        reg.register(
            KeyEvent::new("esc"),
            Modal(ModalType::Approval),
            "close-modal",
        );
        reg.register(
            KeyEvent::new("1"),
            Modal(ModalType::Approval),
            "approve-yes",
        );
        reg.register(KeyEvent::new("2"), Modal(ModalType::Approval), "approve-no");

        // Widget-level binding (ADR-013 §2)
        reg.register(KeyEvent::new("tab"), Widget, "cycle-approval-options");

        reg
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::view_state::{ModalType, PanelKind};
    use FocusScope::*;

    #[test]
    fn registry_resolve_app_binding_at_app_focus() {
        let reg = ShortcutRegistry::with_defaults();
        let key = KeyEvent::new("p").with_ctrl().with_shift();
        assert_eq!(reg.resolve(&key, &App), Some("toggle-command-palette"));
    }

    #[test]
    fn registry_resolve_app_binding_at_panel_focus() {
        let reg = ShortcutRegistry::with_defaults();
        let key = KeyEvent::new("p").with_ctrl().with_shift();
        let focus = Panel(PanelKind::ChatStream);
        assert_eq!(reg.resolve(&key, &focus), Some("toggle-command-palette"));
    }

    #[test]
    fn registry_resolve_panel_binding_at_matching_panel() {
        let reg = ShortcutRegistry::with_defaults();
        let key = KeyEvent::new("j");
        let focus = Panel(PanelKind::ChatStream);
        assert_eq!(reg.resolve(&key, &focus), Some("navigate-down"));
    }

    #[test]
    fn registry_resolve_panel_binding_at_wrong_panel_is_none() {
        let reg = ShortcutRegistry::with_defaults();
        let key = KeyEvent::new("j");
        let focus = Panel(PanelKind::RightWorkspace);
        assert_eq!(reg.resolve(&key, &focus), None);
    }

    #[test]
    fn registry_resolve_panel_binding_at_widget_focus() {
        let reg = ShortcutRegistry::with_defaults();
        let key = KeyEvent::new("j");
        assert_eq!(reg.resolve(&key, &Widget), Some("navigate-down"));
    }

    #[test]
    fn registry_resolve_workspace_ctrl_s_only_in_workspace() {
        let reg = ShortcutRegistry::with_defaults();
        let key = KeyEvent::new("s").with_ctrl();

        assert_eq!(
            reg.resolve(&key, &Panel(PanelKind::RightWorkspace)),
            Some("save-previewed-file")
        );
        assert_eq!(reg.resolve(&key, &Panel(PanelKind::ChatStream)), None);
        assert_eq!(reg.resolve(&key, &App), None);
    }

    #[test]
    fn registry_resolve_modal_esc_at_modal_focus() {
        let reg = ShortcutRegistry::with_defaults();
        let key = KeyEvent::new("esc");
        let focus = Modal(ModalType::Approval);
        assert_eq!(reg.resolve(&key, &focus), Some("close-modal"));
    }

    #[test]
    fn registry_resolve_modal_esc_not_at_app_focus() {
        let reg = ShortcutRegistry::with_defaults();
        let key = KeyEvent::new("esc");
        assert_eq!(reg.resolve(&key, &App), None);
    }

    #[test]
    fn specificity_wins_on_key_collision() {
        let mut reg = ShortcutRegistry::new();
        reg.register(KeyEvent::new("x").with_ctrl(), App, "global-x");
        reg.register(
            KeyEvent::new("x").with_ctrl(),
            Panel(PanelKind::ChatStream),
            "panel-x",
        );

        let key = KeyEvent::new("x").with_ctrl();
        assert_eq!(reg.resolve(&key, &App), Some("global-x"));
        assert_eq!(
            reg.resolve(&key, &Panel(PanelKind::ChatStream)),
            Some("panel-x")
        );
        assert_eq!(
            reg.resolve(&key, &Panel(PanelKind::RightWorkspace)),
            Some("global-x")
        );
    }

    #[test]
    fn lifo_overwrite_same_key_and_scope() {
        let mut reg = ShortcutRegistry::new();
        reg.register(KeyEvent::new("a").with_ctrl(), App, "first");
        reg.register(KeyEvent::new("a").with_ctrl(), App, "second");
        let key = KeyEvent::new("a").with_ctrl();
        assert_eq!(reg.resolve(&key, &App), Some("second"));
    }

    #[test]
    fn unregistered_key_returns_none() {
        let reg = ShortcutRegistry::with_defaults();
        let key = KeyEvent::new("z").with_ctrl().with_shift().with_alt();
        assert_eq!(reg.resolve(&key, &App), None);
    }

    #[test]
    fn help_entries_returns_all_bindings() {
        let reg = ShortcutRegistry::with_defaults();
        let entries = reg.help_entries();
        assert!(!entries.is_empty());
        for e in &entries {
            assert!(!e.command_id.is_empty());
        }
    }
}
