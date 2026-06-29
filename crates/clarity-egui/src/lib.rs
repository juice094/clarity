//! Shared library surface for the `clarity-egui` crate.
//!
//! # Architecture
//!
//! The binary target (`main.rs`) holds all rendering modules (handlers,
//! panels, stores, services, widgets). The library target exposes types
//! and persistence logic that don't depend on `eframe::App`, compiled
//! from the same source files via `#[path]` attributes.
//!
//! Module declarations shared with `main.rs` use `#[path]` to reference
//! the same source file — each crate root compiles an independent copy.
//!
//! # Documentation
//!
//! `pub` items in this library target reuse the source from the binary
//! target, which holds the canonical doc comments. To avoid duplicating
//! docs across both crate roots, `missing_docs` is allowed here.

#![allow(missing_docs)]
#![cfg_attr(
    test,
    allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, unsafe_code)
)]

// ── Domain types (no App dependency) ──
// Nested to match `crate::ui::types` path used by session.rs.
pub mod ui {
    #[path = "types.rs"]
    pub mod types;

    /// Stub markdown parser — `types.rs` references this for `Message::prepare()`,
    /// but that function is only called during binary rendering. The library
    /// target never invokes `prepare()`, so `parse_markdown` is never actually used.
    pub mod markdown {
        use crate::ui::types::RenderBlock;
        #[allow(dead_code)]
        pub fn parse_markdown(_content: &str) -> Vec<RenderBlock> {
            Vec::new()
        }
    }
}

// ── Session persistence ──
#[path = "session.rs"]
pub mod session;

// ── Theme system ──
#[path = "theme.rs"]
pub mod theme;

// ── Animation helpers ──
#[path = "animation.rs"]
pub mod animation;

// ── Internationalisation ──
#[path = "i18n.rs"]
pub mod i18n;

// ── Error types ──
#[path = "error.rs"]
pub mod error;

// ── Provider definitions ──
#[path = "provider.rs"]
pub mod provider;

// ── GUI settings persistence ──
#[path = "settings.rs"]
pub mod settings;

// ── LLM binding (Layer 3, synchronous, no App dependency) ──
#[path = "llm_binder.rs"]
pub mod llm_binder;

// ── Convenience re-exports ──
pub use ui::types;

// ── Test utilities ──
// Always compiled so integration tests (tests/*.rs) can import them.
// Integration tests link to the library as an external crate, where
// `#[cfg(test)]` is NOT automatically enabled.
pub mod test_util;
// test: verify pre-commit hook
