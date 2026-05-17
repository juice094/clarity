//! P1-2: Agent executor trait abstraction.
//!
//! The `AgentExecutor` trait has been uplifted to `clarity-contract::subagent`
//! (see ADR-005) so that `clarity-subagents` can depend on the trait without
//! pulling in the concrete `Agent` type.
//!
//! This module now only provides the `Agent` -> `AgentExecutor` bridge impl.
//! Future work: abstract `with_llm`, `with_approval_runtime`,
//! `with_approval_mode` into a separate `AgentBuilder` trait.

use crate::agent::Agent;
use crate::error::AgentError;
use clarity_contract::subagent::AgentExecutor;

#[async_trait::async_trait]
impl AgentExecutor for Agent {
    async fn run_turn(&self, query: &str) -> Result<String, AgentError> {
        self.run(query).await
    }

    fn last_turn_message_count(&self) -> usize {
        self.last_turn_message_count()
    }
}
