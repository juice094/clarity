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
/// Extended in P1.5.1 (2026-05-13) and revised in P1.5.4 per ADR-014:
/// - `Dashboard` added (was missed in P1.5.1).
/// - `Skill` / `Mcp` removed (relocated to `ModalType` — they are full-screen
///   scrim modals, not side panels).
///
/// All variants are mutually exclusive when assigned to `view_state.right`,
/// reflecting the single-tab consolidation decision from ADR-014.
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
    /// Dashboard aggregate view (P1.5.4 addition; was previously `dashboard_panel_open` boolean).
    Dashboard,
    /// File preview drawer (P1.5.1 addition).
    PreviewDrawer,
    /// Sub-agent progress floating panel (P1.5.1 addition).
    SubAgentProgress,
}

impl SidePanel {
    /// Derive the right-anchored business panel from the three legacy
    /// boolean flags (`dashboard_panel_open`, `team_panel_open`,
    /// `task_panel_open`).
    ///
    /// **Priority order** (most-important-when-multiple-true wins; mirrors
    /// the *inverse* of the responsive-collapse order in `main.rs:847-852`):
    ///
    /// 1. `Task` — user's active task detail; highest semantic priority.
    /// 2. `Team` — team management; secondary.
    /// 3. `Dashboard` — aggregate view; lowest (first to yield on narrow screens).
    /// 4. `None` — all three false.
    ///
    /// After ADR-014, multi-true input is a transitional state during
    /// migration only; once `view_state.right` becomes the authoritative
    /// writer (P1.5.2 bridge reversal), at most one boolean will ever be true.
    pub fn from_legacy_right_panel(dashboard: bool, team: bool, task: bool) -> Option<Self> {
        if task {
            Some(Self::Task)
        } else if team {
            Some(Self::Team)
        } else if dashboard {
            Some(Self::Dashboard)
        } else {
            None
        }
    }
}

/// Blocking modal type — top layer that receives exclusive input.
///
/// Extended in P1.5.1 (2026-05-13) and revised in P1.5.4 per ADR-014:
/// - `Skill` / `Mcp` added (relocated from `SidePanel` — these panels use a
///   full-screen scrim + outside-click-close + Esc-close, i.e., modal behavior).
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
    /// Skill management modal (P1.5.4 relocation from SidePanel per ADR-014).
    Skill,
    /// MCP server configuration modal (P1.5.4 relocation from SidePanel per ADR-014).
    Mcp,
}

impl ModalType {
    /// Derive `ModalType` from the two legacy boolean flags
    /// (`skill_panel_open`, `mcp_panel_open`) that controlled the
    /// modals previously misclassified as side panels (see ADR-014).
    ///
    /// **Priority**: `Skill` wins over `Mcp` if both are true (shouldn't
    /// happen in practice — the underlying scrim layers would stack).
    /// Returns `None` if both are false.
    ///
    /// This helper is the read-only side of the pre-bridge-reversal
    /// mirror; it does *not* compose with the other modal booleans
    /// (e.g. `settings_open`) because those are migrated separately in
    /// P1.5.3.
    pub fn from_legacy_skill_mcp(skill: bool, mcp: bool) -> Option<Self> {
        if skill {
            Some(Self::Skill)
        } else if mcp {
            Some(Self::Mcp)
        } else {
            None
        }
    }
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

impl TurnState {
    /// Derive `TurnState` from the four legacy boolean flags scattered across
    /// `ChatStore` and `SnapshotStore`.
    ///
    /// **Priority order** (most-urgent UI-display wins, highest priority first):
    ///
    /// 1. `Stopping` — user requested stop; communicate user intent.
    /// 2. `Compacting` — special phase, distinct UI cue desired.
    /// 3. `Loading` — regular generation.
    /// 4. `Restoring` — separate from agent turns; lowest non-idle priority.
    /// 5. `Idle` — all false.
    ///
    /// This priority matters when multiple booleans are set simultaneously
    /// (which is a transitional state during stop/restore handoffs).
    pub fn from_legacy(
        is_loading: bool,
        compacting: bool,
        stopping: bool,
        restoring: bool,
    ) -> Self {
        if stopping {
            Self::Stopping
        } else if compacting {
            Self::Compacting
        } else if is_loading {
            Self::Loading
        } else if restoring {
            Self::Restoring
        } else {
            Self::Idle
        }
    }
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

impl PanelExpansion {
    /// Construct `PanelExpansion` from the seven legacy boolean flags
    /// scattered across `UiStore` and `CronStore`.
    ///
    /// This is a direct 1-to-1 mapping; no priority resolution required
    /// since these flags are semantically independent.
    pub fn from_legacy_flags(
        cron: bool,
        web_tabs: bool,
        thinking_log: bool,
        tools: bool,
        subagents: bool,
        workspace_plan: bool,
        workspace_plan_manually_collapsed: bool,
    ) -> Self {
        Self {
            cron,
            web_tabs,
            thinking_log,
            tools,
            subagents,
            workspace_plan,
            workspace_plan_manually_collapsed,
        }
    }
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

impl FocusScope {
    /// Specificity rank for conflict resolution (higher = more specific).
    ///
    /// Hierarchy per ADR-013: Widget(5) > Panel(4) > Modal(3) > App(2) > Os(1).
    pub fn specificity(&self) -> u8 {
        match self {
            FocusScope::Widget => 5,
            FocusScope::Panel(_) => 4,
            FocusScope::Modal(_) => 3,
            FocusScope::App => 2,
            FocusScope::Os => 1,
        }
    }

    /// Returns true if a binding defined for `self` may fire when the current
    /// keyboard focus is `focus`.
    ///
    /// Rules per ADR-013 §7 Conflict Resolution:
    /// - Exact match always works.
    /// - Os bindings fire only at Os focus.
    /// - App bindings fire at App focus and any more-specific focus.
    /// - Modal bindings fire at Modal focus and Widget focus inside a modal.
    /// - Panel bindings fire at Panel focus and Widget focus inside a panel.
    /// - Widget bindings fire only at Widget focus.
    pub fn is_compatible_with(&self, focus: &FocusScope) -> bool {
        use FocusScope::*;
        match (self, focus) {
            // Exact match (includes Panel/Modal variant equality).
            (a, b) if a == b => true,
            // Os-level bindings are exclusive to Os focus.
            (Os, Os) => true,
            // App-level global shortcuts available everywhere except Os-only.
            (App, App | Modal(_) | Panel(_) | Widget) => true,
            // Modal shortcuts available inside the *same* modal type or its child widgets.
            (Modal(mt1), Modal(mt2)) => mt1 == mt2,
            (Modal(_), Widget) => true, // skeleton: assume widget is inside the modal
            // Panel shortcuts available inside the *same* panel kind or its child widgets.
            (Panel(pk1), Panel(pk2)) => pk1 == pk2,
            (Panel(_), Widget) => true, // skeleton: assume widget is inside the panel
            // Widget shortcuts are widget-exclusive.
            (Widget, Widget) => true,
            _ => false,
        }
    }
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
        self.left = if self.left == Some(panel) {
            None
        } else {
            Some(panel)
        };
    }

    /// Toggle right panel (mutually exclusive: only one right panel at a time).
    pub fn toggle_right(&mut self, panel: SidePanel) {
        self.right = if self.right == Some(panel) {
            None
        } else {
            Some(panel)
        };
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
        vs.toggle_left(SidePanel::Dashboard);
        assert_eq!(vs.left, Some(SidePanel::Dashboard));
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
            SidePanel::Dashboard,
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
            ModalType::Skill,
            ModalType::Mcp,
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

    // ────────────────────────────────────────────────────────────────────────
    // P1.5.5 — TurnState::from_legacy priority tests
    // ────────────────────────────────────────────────────────────────────────

    #[test]
    fn turn_state_from_legacy_all_false_is_idle() {
        assert_eq!(
            TurnState::from_legacy(false, false, false, false),
            TurnState::Idle
        );
    }

    #[test]
    fn turn_state_from_legacy_loading_only() {
        assert_eq!(
            TurnState::from_legacy(true, false, false, false),
            TurnState::Loading
        );
    }

    #[test]
    fn turn_state_from_legacy_compacting_only() {
        assert_eq!(
            TurnState::from_legacy(false, true, false, false),
            TurnState::Compacting
        );
    }

    #[test]
    fn turn_state_from_legacy_stopping_only() {
        assert_eq!(
            TurnState::from_legacy(false, false, true, false),
            TurnState::Stopping
        );
    }

    #[test]
    fn turn_state_from_legacy_restoring_only() {
        assert_eq!(
            TurnState::from_legacy(false, false, false, true),
            TurnState::Restoring
        );
    }

    #[test]
    fn turn_state_priority_stopping_over_loading() {
        // During user-initiated stop, is_loading may still be true until
        // async cleanup completes. Stopping should win the UI display.
        assert_eq!(
            TurnState::from_legacy(true, false, true, false),
            TurnState::Stopping
        );
    }

    #[test]
    fn turn_state_priority_stopping_over_compacting() {
        assert_eq!(
            TurnState::from_legacy(false, true, true, false),
            TurnState::Stopping
        );
    }

    #[test]
    fn turn_state_priority_compacting_over_loading() {
        // If both is_loading and compacting are true, compacting wins (special phase).
        assert_eq!(
            TurnState::from_legacy(true, true, false, false),
            TurnState::Compacting
        );
    }

    #[test]
    fn turn_state_priority_loading_over_restoring() {
        // Loading + restoring is an unusual state; loading wins.
        assert_eq!(
            TurnState::from_legacy(true, false, false, true),
            TurnState::Loading
        );
    }

    #[test]
    fn turn_state_priority_all_true_yields_stopping() {
        // Worst-case state: all four booleans true. Stopping (highest priority) wins.
        assert_eq!(
            TurnState::from_legacy(true, true, true, true),
            TurnState::Stopping
        );
    }

    // ────────────────────────────────────────────────────────────────────────
    // P1.5.6 — PanelExpansion::from_legacy_flags
    // ────────────────────────────────────────────────────────────────────────

    #[test]
    fn panel_expansion_from_legacy_all_false() {
        let exp =
            PanelExpansion::from_legacy_flags(false, false, false, false, false, false, false);
        assert_eq!(exp, PanelExpansion::default());
    }

    #[test]
    fn panel_expansion_from_legacy_one_to_one_mapping() {
        let exp = PanelExpansion::from_legacy_flags(
            true,  // cron
            false, // web_tabs
            true,  // thinking_log
            false, // tools
            true,  // subagents
            false, // workspace_plan
            true,  // workspace_plan_manually_collapsed
        );
        assert!(exp.cron);
        assert!(!exp.web_tabs);
        assert!(exp.thinking_log);
        assert!(!exp.tools);
        assert!(exp.subagents);
        assert!(!exp.workspace_plan);
        assert!(exp.workspace_plan_manually_collapsed);
    }

    #[test]
    fn panel_expansion_from_legacy_all_true() {
        let exp = PanelExpansion::from_legacy_flags(true, true, true, true, true, true, true);
        assert!(exp.cron);
        assert!(exp.web_tabs);
        assert!(exp.thinking_log);
        assert!(exp.tools);
        assert!(exp.subagents);
        assert!(exp.workspace_plan);
        assert!(exp.workspace_plan_manually_collapsed);
    }

    // ────────────────────────────────────────────────────────────────────────
    // P1.5.4c — SidePanel::from_legacy_right_panel priority tests (ADR-014)
    // ────────────────────────────────────────────────────────────────────────

    #[test]
    fn side_panel_from_legacy_all_false_is_none() {
        assert_eq!(
            SidePanel::from_legacy_right_panel(false, false, false),
            None
        );
    }

    #[test]
    fn side_panel_from_legacy_dashboard_only() {
        assert_eq!(
            SidePanel::from_legacy_right_panel(true, false, false),
            Some(SidePanel::Dashboard)
        );
    }

    #[test]
    fn side_panel_from_legacy_team_only() {
        assert_eq!(
            SidePanel::from_legacy_right_panel(false, true, false),
            Some(SidePanel::Team)
        );
    }

    #[test]
    fn side_panel_from_legacy_task_only() {
        assert_eq!(
            SidePanel::from_legacy_right_panel(false, false, true),
            Some(SidePanel::Task)
        );
    }

    #[test]
    fn side_panel_priority_task_over_team() {
        // Multi-true transitional state: task wins (highest semantic priority).
        assert_eq!(
            SidePanel::from_legacy_right_panel(false, true, true),
            Some(SidePanel::Task)
        );
    }

    #[test]
    fn side_panel_priority_team_over_dashboard() {
        assert_eq!(
            SidePanel::from_legacy_right_panel(true, true, false),
            Some(SidePanel::Team)
        );
    }

    #[test]
    fn side_panel_priority_task_over_all() {
        assert_eq!(
            SidePanel::from_legacy_right_panel(true, true, true),
            Some(SidePanel::Task)
        );
    }

    // ────────────────────────────────────────────────────────────────────────
    // P1.5.4c — ModalType::from_legacy_skill_mcp tests (ADR-014)
    // ────────────────────────────────────────────────────────────────────────

    #[test]
    fn modal_type_from_legacy_skill_mcp_all_false_is_none() {
        assert_eq!(ModalType::from_legacy_skill_mcp(false, false), None);
    }

    #[test]
    fn modal_type_from_legacy_skill_only() {
        assert_eq!(
            ModalType::from_legacy_skill_mcp(true, false),
            Some(ModalType::Skill)
        );
    }

    #[test]
    fn modal_type_from_legacy_mcp_only() {
        assert_eq!(
            ModalType::from_legacy_skill_mcp(false, true),
            Some(ModalType::Mcp)
        );
    }

    #[test]
    fn modal_type_from_legacy_skill_priority_over_mcp() {
        assert_eq!(
            ModalType::from_legacy_skill_mcp(true, true),
            Some(ModalType::Skill)
        );
    }

    // ────────────────────────────────────────────────────────────────────────
    // P1.5.7 — Illegal-state reachability tests
    // ────────────────────────────────────────────────────────────────────────

    /// Exhaustive coverage of all 16 input combinations to `TurnState::from_legacy`.
    /// Every tuple maps deterministically to a single variant via the priority ladder.
    #[test]
    fn turn_state_from_legacy_exhaustive() {
        let cases: [((bool, bool, bool, bool), TurnState); 16] = [
            // 0 true → Idle
            ((false, false, false, false), TurnState::Idle),
            // 1 true → direct mapping
            ((true, false, false, false), TurnState::Loading),
            ((false, true, false, false), TurnState::Compacting),
            ((false, false, true, false), TurnState::Stopping),
            ((false, false, false, true), TurnState::Restoring),
            // 2 true → priority resolution
            ((true, true, false, false), TurnState::Compacting), // compacting > loading
            ((true, false, true, false), TurnState::Stopping),   // stopping > loading
            ((true, false, false, true), TurnState::Loading),    // loading > restoring
            ((false, true, true, false), TurnState::Stopping),   // stopping > compacting
            ((false, true, false, true), TurnState::Compacting), // compacting > restoring
            ((false, false, true, true), TurnState::Stopping),   // stopping > restoring
            // 3 true → top priority wins
            ((true, true, true, false), TurnState::Stopping), // stopping > compacting > loading
            ((true, true, false, true), TurnState::Compacting), // compacting > loading > restoring
            ((true, false, true, true), TurnState::Stopping), // stopping > loading > restoring
            ((false, true, true, true), TurnState::Stopping), // stopping > compacting > restoring
            // 4 true → stopping highest
            ((true, true, true, true), TurnState::Stopping),
        ];

        for ((loading, compacting, stopping, restoring), expected) in cases {
            let actual = TurnState::from_legacy(loading, compacting, stopping, restoring);
            assert_eq!(
                actual, expected,
                "from_legacy({loading}, {compacting}, {stopping}, {restoring}) expected {expected:?} but got {actual:?}"
            );
        }
    }

    /// TurnState is an enum — by Rust type-system construction it is impossible
    /// to hold two variants simultaneously. This test documents that invariant.
    #[test]
    fn turn_state_is_mutually_exclusive_by_type() {
        // If TurnState were a struct with boolean flags, this would be a real risk.
        // Because it is an enum, the compiler rejects:
        //   let t = TurnState::Loading;
        //   t = TurnState::Compacting; // overwrite, not coexist
        // This test exists as executable documentation of the type-level guarantee.
        let states = [
            TurnState::Idle,
            TurnState::Loading,
            TurnState::Compacting,
            TurnState::Stopping,
            TurnState::Restoring,
        ];
        // All 5 variants are pairwise distinct.
        for (i, a) in states.iter().enumerate() {
            for (j, b) in states.iter().enumerate() {
                if i == j {
                    assert_eq!(a, b);
                } else {
                    assert_ne!(a, b, "TurnState variants must be mutually exclusive");
                }
            }
        }
    }

    /// ViewState operations must maintain structural invariants:
    /// - at most one modal
    /// - at most one left panel
    /// - at most one right panel
    /// - turn is always exactly one variant
    #[test]
    fn view_state_structural_invariants() {
        let mut vs = ViewState::new();

        // Invariant: modal is Option — open overwrites, never stacks.
        vs.open_modal(ModalType::Approval);
        assert_eq!(vs.modal, Some(ModalType::Approval));
        vs.open_modal(ModalType::Skill);
        assert_eq!(
            vs.modal,
            Some(ModalType::Skill),
            "Modal must be single-valued; opening a second overwrites"
        );

        // Invariant: right panel is Option — toggle replaces.
        vs.toggle_right(SidePanel::Task);
        assert_eq!(vs.right, Some(SidePanel::Task));
        vs.toggle_right(SidePanel::Team);
        assert_eq!(
            vs.right,
            Some(SidePanel::Team),
            "Right panel must be single-valued"
        );

        // Invariant: left panel is Option — toggle replaces.
        vs.toggle_left(SidePanel::Sidebar);
        assert_eq!(vs.left, Some(SidePanel::Sidebar));
        vs.toggle_left(SidePanel::Workspace);
        assert_eq!(
            vs.left,
            Some(SidePanel::Workspace),
            "Left panel must be single-valued"
        );

        // Invariant: turn is never ambiguous.
        vs.turn = TurnState::Loading;
        assert!(vs.is_turn_active());
        vs.turn = TurnState::Idle;
        assert!(!vs.is_turn_active());
        // It is impossible for vs.turn to simultaneously be Loading and Compacting
        // because TurnState is an enum.
    }

    /// Illegal focus transitions: when a modal is open, focus_panel is a no-op.
    /// When modal closes, focus returns to App (caller must restore specific panel).
    #[test]
    fn focus_modal_blocks_panel_focus_invariant() {
        let mut vs = ViewState::new();

        vs.focus_panel(PanelKind::ChatStream);
        assert_eq!(vs.focus, FocusScope::Panel(PanelKind::ChatStream));

        vs.open_modal(ModalType::Approval);
        assert!(matches!(vs.focus, FocusScope::Modal(_)));

        // Attempting to focus a panel while modal is open must not change focus.
        vs.focus_panel(PanelKind::RightWorkspace);
        assert_eq!(vs.focus, FocusScope::Modal(ModalType::Approval));

        vs.close_modal();
        assert_eq!(vs.focus, FocusScope::App);
    }

    /// Illegal side-panel placement: SidePanel variants are not typed by left/right
    /// at the enum level, but the ViewState API (toggle_left / toggle_right) enforces
    /// that at most one panel occupies each physical side.
    #[test]
    fn side_panel_physical_exclusivity() {
        let mut vs = ViewState::new();

        // Left side can hold any SidePanel variant, but only one at a time.
        vs.toggle_left(SidePanel::Sidebar);
        vs.toggle_left(SidePanel::Workspace);
        assert_eq!(vs.left, Some(SidePanel::Workspace));
        assert!(vs.right.is_none());

        // Right side is independent.
        vs.toggle_right(SidePanel::Task);
        assert_eq!(vs.right, Some(SidePanel::Task));

        // Clearing left does not affect right.
        vs.toggle_left(SidePanel::Workspace); // toggles off
        assert!(vs.left.is_none());
        assert_eq!(vs.right, Some(SidePanel::Task));
    }
}
