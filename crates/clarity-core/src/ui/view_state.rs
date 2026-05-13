//! UI State Machine (S3 / Phase 1.5)
//!
//! Typed enums and structs that absorb the 50 boolean state flags audited in
//! P1.5.0. These types are the single source of truth for view state across
//! `clarity-egui` and `clarity-tui`.
//!
//! Established by ADR-011 (workspace architecture), ADR-012 (RenderLine enum),
//! and ADR-013 (keyboard shortcut focus-aware routing).
//!
//! ## Type Map
//!
//! - [`AppView`] — exclusive top-level view (Chat / Settings / Dashboard / ...)
//! - [`SidePanel`] — overlay panels alongside the main view
//! - [`ModalType`] — blocking modal dialogs with exclusive input focus
//! - [`TurnState`] — agent turn lifecycle (Idle / Loading / Compacting / ...)
//! - [`PanelExpansion`] — collapse/expand state of in-panel regions
//! - [`PanelKind`] — physical panel identifier (used by FocusScope)
//! - [`FocusScope`] — which scope currently owns keyboard input (ADR-013)
//! - [`ViewState`] — composed root that aggregates all of the above

use serde::{Deserialize, Serialize};

/// Main view enumeration — GUI and TUI share the same view semantics.
///
/// TUI shows one main view at a time due to screen constraints; GUI allows
/// side-panel overlays, but the main view itself is always mutually exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AppView {
    /// Primary chat view (default).
    #[default]
    Chat,
    /// Configuration panel (property inspector style).
    Settings,
    /// System dashboard.
    Dashboard,
    /// Gantt / timeline view.
    Gantt,
    /// Task board (full-screen).
    TaskBoard,
}

/// Side panel type — supports left/right overlays in GUI; TUI achieves the
/// same via modal switching.
///
/// Extended in P1.5.1 (2026-05-13) to cover the 9 panel-open booleans
/// found in the audit. Existing variants (`Sidebar`, `Workspace`, `Team`,
/// `Task`) are preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SidePanel {
    /// Left navigation / role list (left-anchored).
    Sidebar,
    /// File / workspace tab (right-anchored, primary tab in the D form factor).
    Workspace,
    /// Team collaboration panel.
    Team,
    /// Task details panel.
    Task,
    /// Skill management panel (P1.5.1 addition).
    Skill,
    /// MCP server configuration panel (P1.5.1 addition).
    Mcp,
    /// File preview drawer (P1.5.1 addition).
    PreviewDrawer,
    /// Sub-agent progress floating panel (P1.5.1 addition).
    SubAgentProgress,
}

/// Blocking modal type — top layer that receives exclusive input.
///
/// Extended in P1.5.1 (2026-05-13) to cover the 9 modal-open booleans
/// found in the audit. Existing variants are preserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModalType {
    /// Operation approval prompt.
    Approval,
    /// Snapshot restore.
    Snapshot,
    /// OAuth login.
    Login,
    /// Create new task.
    TaskCreate,
    /// View task detail (P1.5.1 addition).
    TaskView,
    /// Create new team.
    TeamCreate,
    /// Create new cron schedule (P1.5.1 addition).
    CronCreate,
    /// View sub-agent detail (P1.5.1 addition).
    SubAgentView,
    /// Add new LLM provider (P1.5.1 addition).
    AddProvider,
    /// Kimi Code login (P1.5.1 addition).
    KimiCodeLogin,
}

/// Agent turn lifecycle state — replaces the four legacy workflow booleans
/// (`is_loading`, `compacting`, `stopping`, `restoring`).
///
/// Added in P1.5.1 (2026-05-13). Mutually exclusive (a turn cannot be both
/// Loading and Compacting simultaneously).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TurnState {
    /// No active turn (default).
    #[default]
    Idle,
    /// LLM generation in progress.
    Loading,
    /// Context compaction running.
    Compacting,
    /// User-initiated stop in progress.
    Stopping,
    /// Session restore from snapshot in progress.
    Restoring,
}

/// Collapse/expand state for in-panel regions — replaces the seven legacy
/// expansion booleans found in the audit.
///
/// Added in P1.5.1 (2026-05-13). Each field tracks whether a specific
/// sub-region is expanded; `false` = collapsed (default state).
///
/// Note: `workspace_plan_manually_collapsed` is a sticky override —
/// when set, the auto-expansion heuristic does not re-open the plan
/// panel even if a new plan starts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PanelExpansion {
    /// Cron schedule list expanded.
    #[serde(default)]
    pub cron: bool,
    /// Web tabs (tabbit precursor) expanded.
    #[serde(default)]
    pub web_tabs: bool,
    /// Agent thinking log expanded.
    #[serde(default)]
    pub thinking_log: bool,
    /// MCP tool list expanded.
    #[serde(default)]
    pub tools: bool,
    /// Sub-agent list expanded.
    #[serde(default)]
    pub subagents: bool,
    /// Workspace plan tree expanded.
    #[serde(default)]
    pub workspace_plan: bool,
    /// Sticky flag preventing auto-expansion of workspace plan.
    #[serde(default)]
    pub workspace_plan_manually_collapsed: bool,
}

/// Physical panel identifier — used by [`FocusScope::Panel`] to discriminate
/// which panel currently owns keyboard input.
///
/// Added in P1.5.1 (2026-05-13) per ADR-013 §1 focus-aware routing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PanelKind {
    /// Left sidebar (sessions / pinned / notes).
    LeftSidebar,
    /// Center chat stream (RenderLine flow).
    ChatStream,
    /// Right panel — SSH tab.
    RightSsh,
    /// Right panel — Workspace tab (3-tier file view).
    RightWorkspace,
    /// Right panel — Settings tab.
    RightSettings,
    /// Bottom input panel.
    Input,
    /// Bottom status bar with equipment floating panels.
    StatusBar,
}

/// Keyboard focus scope — determines which set of shortcuts is active.
///
/// Resolution order (most specific wins):
/// `Widget` > `Modal` > `Panel` > `App` > `Os`.
///
/// Added in P1.5.1 (2026-05-13) per ADR-013 §1.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "value")]
pub enum FocusScope {
    /// A specific widget has focus (e.g., approval prompt with 1/2/3 active).
    Widget,
    /// A panel has focus (e.g., chat stream → j/k navigation; Workspace tab → Ctrl+S).
    Panel(PanelKind),
    /// A modal is open and consumes input.
    Modal(ModalType),
    /// No specific panel focus (app-level shortcuts only — Ctrl+Shift+P, etc.).
    #[default]
    App,
    /// OS window-level (Ctrl+Q, F11, etc.).
    Os,
}

/// Unified view state — single source of truth shared by GUI and TUI.
///
/// ## Composition rules
///
/// - `main` switching preserves `left` / `right` panel state (unless responsive
///   guard triggers collapse).
/// - When `modal` is set, `main` / `left` / `right` render but do not receive
///   input events; `focus` is overridden to `FocusScope::Modal(...)`.
/// - `turn` is independent of view state; an agent turn can run in any view.
/// - `expansions` is independent per panel; collapsing one does not affect others.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ViewState {
    /// Current main view.
    #[serde(default)]
    pub main: AppView,
    /// Left-side panel overlay (None = hidden).
    #[serde(default)]
    pub left: Option<SidePanel>,
    /// Right-side panel overlay (None = hidden).
    #[serde(default)]
    pub right: Option<SidePanel>,
    /// Currently open modal (None = no modal).
    #[serde(default)]
    pub modal: Option<ModalType>,
    /// Agent turn lifecycle state.
    #[serde(default)]
    pub turn: TurnState,
    /// In-panel collapse/expand state.
    #[serde(default)]
    pub expansions: PanelExpansion,
    /// Current keyboard focus scope.
    #[serde(default)]
    pub focus: FocusScope,
}

impl ViewState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Switch main view, preserving side panels.
    pub fn switch_main(&mut self, view: AppView) {
        self.main = view;
    }

    /// Toggle left panel (mutually exclusive: only one left panel at a time).
    pub fn toggle_left(&mut self, panel: SidePanel) {
        self.left = if self.left == Some(panel) { None } else { Some(panel) };
    }

    /// Toggle right panel (mutually exclusive: only one right panel at a time).
    pub fn toggle_right(&mut self, panel: SidePanel) {
        self.right = if self.right == Some(panel) { None } else { Some(panel) };
    }

    /// Open a modal. The modal takes input focus until closed.
    pub fn open_modal(&mut self, modal: ModalType) {
        self.modal = Some(modal);
        self.focus = FocusScope::Modal(modal);
    }

    /// Close the current modal and restore focus to the previously-focused panel.
    pub fn close_modal(&mut self) {
        self.modal = None;
        // Focus defaults to App; caller should restore specific panel focus if needed.
        self.focus = FocusScope::App;
    }

    /// Returns true if any side panel is open.
    pub fn has_panels(&self) -> bool {
        self.left.is_some() || self.right.is_some()
    }

    /// Returns true if an agent turn is in progress.
    pub fn is_turn_active(&self) -> bool {
        !matches!(self.turn, TurnState::Idle)
    }

    /// Set focus to a specific panel.
    pub fn focus_panel(&mut self, panel: PanelKind) {
        // Modal focus takes precedence — refuse to override.
        if !matches!(self.focus, FocusScope::Modal(_)) {
            self.focus = FocusScope::Panel(panel);
        }
    }

    /// Returns true if a specific panel kind currently holds focus.
    pub fn is_focused(&self, panel: PanelKind) -> bool {
        matches!(self.focus, FocusScope::Panel(p) if p == panel)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_state_default_is_chat_idle() {
        let vs = ViewState::new();
        assert_eq!(vs.main, AppView::Chat);
        assert_eq!(vs.turn, TurnState::Idle);
        assert_eq!(vs.focus, FocusScope::App);
        assert!(vs.left.is_none());
        assert!(vs.right.is_none());
        assert!(vs.modal.is_none());
        assert!(!vs.is_turn_active());
        assert!(!vs.has_panels());
    }

    #[test]
    fn switch_main_preserves_side_panels() {
        let mut vs = ViewState::new();
        vs.toggle_left(SidePanel::Sidebar);
        vs.toggle_right(SidePanel::Workspace);
        vs.switch_main(AppView::Dashboard);
        assert_eq!(vs.main, AppView::Dashboard);
        assert_eq!(vs.left, Some(SidePanel::Sidebar));
        assert_eq!(vs.right, Some(SidePanel::Workspace));
    }

    #[test]
    fn toggle_left_is_mutually_exclusive() {
        let mut vs = ViewState::new();
        vs.toggle_left(SidePanel::Sidebar);
        assert_eq!(vs.left, Some(SidePanel::Sidebar));
        // Toggling the same panel closes it.
        vs.toggle_left(SidePanel::Sidebar);
        assert_eq!(vs.left, None);
        // Toggling a different panel replaces.
        vs.toggle_left(SidePanel::Sidebar);
        vs.toggle_left(SidePanel::Skill);
        assert_eq!(vs.left, Some(SidePanel::Skill));
    }

    #[test]
    fn open_modal_sets_focus_to_modal() {
        let mut vs = ViewState::new();
        vs.focus = FocusScope::Panel(PanelKind::ChatStream);
        vs.open_modal(ModalType::Approval);
        assert_eq!(vs.modal, Some(ModalType::Approval));
        assert_eq!(vs.focus, FocusScope::Modal(ModalType::Approval));
    }

    #[test]
    fn close_modal_clears_focus_to_app() {
        let mut vs = ViewState::new();
        vs.open_modal(ModalType::Approval);
        vs.close_modal();
        assert!(vs.modal.is_none());
        assert_eq!(vs.focus, FocusScope::App);
    }

    #[test]
    fn modal_focus_overrides_panel_focus_attempts() {
        let mut vs = ViewState::new();
        vs.open_modal(ModalType::TaskCreate);
        // Attempting to focus a panel while modal is open is a no-op.
        vs.focus_panel(PanelKind::ChatStream);
        assert_eq!(vs.focus, FocusScope::Modal(ModalType::TaskCreate));
    }

    #[test]
    fn focus_panel_routes_correctly() {
        let mut vs = ViewState::new();
        vs.focus_panel(PanelKind::ChatStream);
        assert_eq!(vs.focus, FocusScope::Panel(PanelKind::ChatStream));
        assert!(vs.is_focused(PanelKind::ChatStream));
        assert!(!vs.is_focused(PanelKind::RightWorkspace));
    }

    #[test]
    fn turn_state_lifecycle() {
        let mut vs = ViewState::new();
        assert!(!vs.is_turn_active());
        vs.turn = TurnState::Loading;
        assert!(vs.is_turn_active());
        vs.turn = TurnState::Compacting;
        assert!(vs.is_turn_active());
        vs.turn = TurnState::Idle;
        assert!(!vs.is_turn_active());
    }

    #[test]
    fn panel_expansion_default_is_all_collapsed() {
        let exp = PanelExpansion::default();
        assert!(!exp.cron);
        assert!(!exp.web_tabs);
        assert!(!exp.thinking_log);
        assert!(!exp.tools);
        assert!(!exp.subagents);
        assert!(!exp.workspace_plan);
        assert!(!exp.workspace_plan_manually_collapsed);
    }

    #[test]
    fn focus_scope_default_is_app() {
        let fs = FocusScope::default();
        assert_eq!(fs, FocusScope::App);
    }

    #[test]
    fn all_side_panel_variants_serialize() {
        // Sanity check: every SidePanel variant round-trips through JSON.
        for panel in [
            SidePanel::Sidebar,
            SidePanel::Workspace,
            SidePanel::Team,
            SidePanel::Task,
            SidePanel::Skill,
            SidePanel::Mcp,
            SidePanel::PreviewDrawer,
            SidePanel::SubAgentProgress,
        ] {
            let s = serde_json::to_string(&panel).unwrap();
            let deserialized: SidePanel = serde_json::from_str(&s).unwrap();
            assert_eq!(panel, deserialized);
        }
    }

    #[test]
    fn all_modal_type_variants_serialize() {
        for modal in [
            ModalType::Approval,
            ModalType::Snapshot,
            ModalType::Login,
            ModalType::TaskCreate,
            ModalType::TaskView,
            ModalType::TeamCreate,
            ModalType::CronCreate,
            ModalType::SubAgentView,
            ModalType::AddProvider,
            ModalType::KimiCodeLogin,
        ] {
            let s = serde_json::to_string(&modal).unwrap();
            let deserialized: ModalType = serde_json::from_str(&s).unwrap();
            assert_eq!(modal, deserialized);
        }
    }

    #[test]
    fn view_state_roundtrips_through_json() {
        let mut vs = ViewState::new();
        vs.main = AppView::Settings;
        vs.left = Some(SidePanel::Sidebar);
        vs.right = Some(SidePanel::Workspace);
        vs.turn = TurnState::Compacting;
        vs.expansions.tools = true;
        vs.expansions.workspace_plan = true;
        vs.focus = FocusScope::Panel(PanelKind::ChatStream);

        let json = serde_json::to_string(&vs).unwrap();
        let restored: ViewState = serde_json::from_str(&json).unwrap();
        assert_eq!(vs, restored);
    }

    #[test]
    fn focus_scope_with_panel_kind_roundtrips() {
        let fs = FocusScope::Panel(PanelKind::RightWorkspace);
        let json = serde_json::to_string(&fs).unwrap();
        let restored: FocusScope = serde_json::from_str(&json).unwrap();
        assert_eq!(fs, restored);
    }

    #[test]
    fn focus_scope_with_modal_roundtrips() {
        let fs = FocusScope::Modal(ModalType::Approval);
        let json = serde_json::to_string(&fs).unwrap();
        let restored: FocusScope = serde_json::from_str(&json).unwrap();
        assert_eq!(fs, restored);
    }
}
