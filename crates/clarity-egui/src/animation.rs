//! Re-export of animation helpers previously located here.
//!
//! The canonical implementation now lives in `clarity-ui::animation`. This
//! module preserves the old import path during the notedeck-style refactor.

pub use clarity_ui::animation::*;

/// Animation state used by the right-rail panel.
#[derive(Clone, Default)]
pub struct PanelAnimationState {
    /// Previous panel variant for detecting switches.
    pub prev_panel: Option<clarity_core::ui::RightRailPanel>,
}

/// Active main-stage view transition.
#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct MainStageTransition {
    /// View being replaced.
    pub from: clarity_core::ui::AppView,
    /// When the transition started.
    pub started: std::time::Instant,
    /// Total transition duration.
    pub duration: std::time::Duration,
    /// +1.0 slides the new view in from the right; -1.0 from the left.
    pub direction: f32,
}
