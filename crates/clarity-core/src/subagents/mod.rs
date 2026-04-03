//! Subagent management system
//!
//! Provides registry and state storage for subagent instances.

pub mod builder;
mod store;
pub mod registry;

pub use builder::SubagentBuilder;
pub use store::{SubagentState, SubagentStatus, SubagentStore};
pub use registry::{AgentTypeDefinition, LaborMarket};
