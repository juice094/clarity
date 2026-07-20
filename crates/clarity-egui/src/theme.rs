//! Re-export of the shared design system previously located here.
//!
//! The canonical implementation now lives in `clarity-ui::theme`. This module
//! preserves the old import path (`crate::theme::*`) so existing call sites
//! keep working during the notedeck-style architecture refactor.

pub use clarity_ui::theme::*;
