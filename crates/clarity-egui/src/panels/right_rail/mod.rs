//! Right rail cards for the Pretext three-column layout.
//!
//! S6 Phase C: the right rail is now a drawer with a vertical stack of cards.
//! The shell (`main.rs::render_right_rail`) renders the drawer chrome and
//! iterates over the active card order; each card is responsible for its own
//! content.

pub mod context_card;
pub mod progress_card;

pub(crate) mod memory_card;
pub(crate) mod status_card;
pub(crate) mod subagent_card;
pub(crate) mod tools_card;

pub use context_card::render as render_context_card;
pub use progress_card::render as render_progress_card;
