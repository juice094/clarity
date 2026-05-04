//! Message router for the Claw federation.
//!
//! Determines which node should handle a given federation message.
//! In Phase 1 this is a simple lookup; future phases may add
//! load balancing, fallback chains, and capability scoring.

use clarity_contract::{FederationMessage, FederationNode};
use std::sync::Arc;

use super::registry::CapabilityRegistry;

/// Router that selects a target node for a federation message.
#[derive(Default)]
pub struct FederationRouter;

impl FederationRouter {
    /// Create a new router.
    pub fn new() -> Self {
        Self
    }

    /// Route a message to the most appropriate node.
    ///
    /// Returns `Some(node)` if a suitable target is found, `None` otherwise.
    pub fn route(
        &self,
        registry: &CapabilityRegistry,
        msg: &FederationMessage,
    ) -> Option<Arc<dyn FederationNode>> {
        let target_id = match msg {
            FederationMessage::RegisterCapability { node_id, .. } => Some(node_id.as_str()),
            FederationMessage::DispatchTask { target_node, .. } => Some(target_node.as_str()),
            FederationMessage::Heartbeat { node_id, .. } => Some(node_id.as_str()),
            FederationMessage::MemoryQuery { sender, .. } => Some(sender.as_str()),
            FederationMessage::MemoryResponse { request_id: _, .. } => {
                // In a real implementation, track request_id → node mapping.
                // Phase 1: broadcast to all memory-capable nodes.
                return registry.nodes_with_capability("memory_storage").into_iter().next();
            }
            FederationMessage::ToolRequest { .. } => {
                return registry.nodes_with_capability("tool_execution").into_iter().next();
            }
            FederationMessage::ToolResponse { .. } => {
                // Responses are routed back to the original requester.
                // Phase 1: no-op; the caller handles responses directly.
                return None;
            }
            FederationMessage::LlmRequest { .. } => {
                return registry.nodes_with_capability("llm_inference").into_iter().next();
            }
            FederationMessage::LlmResponse { .. } => {
                // Responses are routed back to the original requester.
                return None;
            }
            FederationMessage::AgentTurn { .. } => Some("core"),
        };

        target_id.and_then(|id| registry.get(id))
    }
}
