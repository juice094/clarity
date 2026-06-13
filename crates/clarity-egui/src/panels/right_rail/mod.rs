//! Right rail cards for the Pretext three-column layout.
//!
//! Each card is a self-contained content unit rendered inside the right utility
//! rail. They are intentionally decoupled from panel chrome so the rail shell
//! can switch between them without touching content logic.

pub mod memory_card;
pub mod status_card;
pub mod subagent_card;
pub mod tools_card;

pub use memory_card::render as render_memory_card;
pub use status_card::render as render_status_card;
pub use subagent_card::render as render_subagent_card;
pub use tools_card::render as render_tools_card;
