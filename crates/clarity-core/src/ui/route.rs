//! Typed routes for the Pretext UI.
//!
//! Routes are layer-aware navigation targets. They wrap the existing
//! `AppView`, `ModalType`, and `RightRailPanel` enums so each layer can keep
//! its own `Router<T>` while the chrome has a single typed way to receive
//! navigation requests from sub-applications.

use serde::{Deserialize, Serialize};

/// A navigation target in one of the three UI layers.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Route {
    /// Exclusive main view (Chat / Settings / Dashboard).
    Main(super::AppView),
    /// Blocking modal dialog.
    Modal(super::ModalType),
    /// IDE-style right rail panel.
    RightRail(super::RightRailPanel),
}

impl Route {
    /// Return the layer this route belongs to.
    pub fn layer(&self) -> RouteLayer {
        match self {
            Route::Main(_) => RouteLayer::Main,
            Route::Modal(_) => RouteLayer::Modal,
            Route::RightRail(_) => RouteLayer::RightRail,
        }
    }
}

/// Navigation layer. Used by the chrome to dispatch a route to the correct
/// router and to resolve global "go back" priority.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RouteLayer {
    /// Exclusive main view layer.
    Main,
    /// Blocking modal layer.
    Modal,
    /// IDE-style right rail panel layer.
    RightRail,
}

/// Priority order for global back navigation: modal is closed first, then
/// right rail, then main view history.
pub const BACK_PRIORITY: [RouteLayer; 3] =
    [RouteLayer::Modal, RouteLayer::RightRail, RouteLayer::Main];

impl From<super::AppView> for Route {
    fn from(view: super::AppView) -> Self {
        Route::Main(view)
    }
}

impl From<super::ModalType> for Route {
    fn from(modal: super::ModalType) -> Self {
        Route::Modal(modal)
    }
}

impl From<super::RightRailPanel> for Route {
    fn from(panel: super::RightRailPanel) -> Self {
        Route::RightRail(panel)
    }
}
