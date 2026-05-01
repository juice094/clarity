//! P1-2: Agent executor trait abstraction.
//!
//! Extracts the minimal surface that `subagents::runner` needs from `Agent`,
//! breaking the `subagents↔agent` circular dependency at the type level.
//!
//! PoC scope (Week 4):
//! - Only `run()` is abstracted; builder methods (`with_llm`, etc.) remain on
//!   the concrete `Agent` type.
//! - `subagents::runner::execute_agent` now takes `&dyn AgentExecutor` instead
//!   of `&Agent`, proving the abstraction works end-to-end.
//!
//! Risk: The trait surface is intentionally minimal.  If `subagents` starts
//! calling other `Agent` methods (e.g. `run_streaming`), the trait must grow.
//! Future work:
//! - Abstract `with_llm`, `with_approval_runtime`, `with_approval_mode` into
//!   a separate `AgentBuilder` trait if `subagents` ever needs to construct
//!   agents generically.

use crate::error::AgentError;
use crate::agent::Agent;

/// Minimal trait for anything that can execute an agent turn.
#[async_trait::async_trait]
pub trait AgentExecutor: Send + Sync {
    /// Run a single turn with the given user query.
    async fn run_turn(&self, query: &str) -> Result<String, AgentError>;
}

#[async_trait::async_trait]
impl AgentExecutor for Agent {
    async fn run_turn(&self, query: &str) -> Result<String, AgentError> {
        self.run(query).await
    }
}
