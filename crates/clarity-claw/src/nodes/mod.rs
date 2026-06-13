//! Federal node implementations for the Claw runtime.

/// Skeleton CoreNode used by the tray binary until an `AgentExecutor` is wired in.
pub mod core;

/// Production CoreNode backed by `clarity_core::agent::AgentExecutor`.
pub mod core_node;

/// Re-export of the executor-backed CoreNode as the canonical node type.
pub use core_node::CoreNode;
