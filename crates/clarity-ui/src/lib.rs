//! Shared egui UI primitives and design tokens for Clarity.
//!
//! This crate holds everything that is agnostic to a specific application
//! surface: themes, typography, animation helpers, icon fonts, and reusable
//! widgets. It mirrors the role of `notedeck_ui` in the notedeck codebase.

#![allow(missing_docs)]
#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, unsafe_code)
)]

/// Re-export the egui crate so downstream crates can use the same version
/// without adding their own dependency.
pub use egui;

pub mod animation;
pub mod design_system;
pub mod i18n;
pub mod theme;
pub mod widgets;

/// Stateful top-level views that are larger than a single widget.
///
/// Similar to `notedeck_ui::View`, this trait is used for views that need to
/// keep internal state across frames and are awkward to express as a pure
/// `egui::Widget` implementation (which requires a mutable impl at the type
/// level and complicates preview generation).
pub trait View {
    /// Render the view into the given UI.
    fn ui(&mut self, ui: &mut egui::Ui);
}
