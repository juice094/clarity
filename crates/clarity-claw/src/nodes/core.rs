//! CoreNode — federal wrapper for `clarity-core` capabilities.
//!
//! Implements `FederationNode` to expose the core runtime's
//! LLM, tool, and agent capabilities to the Claw Coordinator.
//!
//! Phase 1: skeleton only. `handle()` returns `Ack` for all messages;
//! actual LLM/Tool dispatch is planned for Phase 2.

use clarity_contract::{
    AgentError, Capability, FederationMessage, FederationNode, FederationResponse,
};

/// Federal node wrapping `clarity-core` capabilities.
pub struct CoreNode;

impl CoreNode {
    /// Create a new CoreNode.
    pub fn new() -> Self {
        tracing::info!("CoreNode initializing");
        Self
    }
}

impl Default for CoreNode {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl FederationNode for CoreNode {
    fn node_id(&self) -> &str {
        "core"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability::LlmInference {
                models: vec!["default".into()],
            },
            Capability::ToolExecution { tools: vec![] },
            Capability::MemoryStorage {
                backends: vec!["sqlite".into(), "file".into()],
            },
        ]
    }

    async fn handle(&self, msg: FederationMessage) -> Result<FederationResponse, AgentError> {
        match msg {
            FederationMessage::Heartbeat { node_id, status } => {
                tracing::debug!(%node_id, ?status, "CoreNode received heartbeat");
                Ok(FederationResponse::Ack)
            }
            FederationMessage::RegisterCapability { node_id, .. } => {
                tracing::debug!(%node_id, "CoreNode received register capability");
                Ok(FederationResponse::Ack)
            }
            _ => {
                tracing::debug!("CoreNode received unhandled message — Phase 1 skeleton");
                Ok(FederationResponse::Ack)
            }
        }
    }
}
