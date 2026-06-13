//! Panels awaiting redesign or integration into the new single-page layout.
//!
//! These modules are preserved to avoid deleting functionality during the
//! module reorganization sprint. They will be either migrated into the
//! three-column layout or removed in a future cleanup phase.
//!
//! S6 Phase B update: `task` and `team` panels have been migrated into the
//! right rail (`panels::right_rail::subagent_card` and `memory_card`).

pub mod gantt;
pub mod mcp;
pub mod skill;
pub mod task_board;
