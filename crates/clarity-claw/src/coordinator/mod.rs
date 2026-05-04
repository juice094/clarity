//! Federation Coordinator for the Claw runtime.
//!
//! The Coordinator is the central hub of the Clarity federal system.
//! It maintains a registry of all federal nodes and routes messages
//! between them. Each node implements `FederationNode` and advertises
//! its capabilities at registration time.

use clarity_contract::{AgentError, FederationMessage, FederationNode, FederationResponse};
use std::sync::Arc;

pub mod registry;
pub mod router;

use registry::CapabilityRegistry;
use router::FederationRouter;

/// Central coordinator for the Claw federation.
///
/// Holds the capability registry and message router.
/// Downstream code (tray, CLI, daemon) creates a Coordinator,
/// registers nodes, and then dispatches messages.
#[derive(Default)]
pub struct Coordinator {
    registry: CapabilityRegistry,
    router: FederationRouter,
}

impl Coordinator {
    /// Create a new, empty coordinator.
    pub fn new() -> Self {
        tracing::info!("Claw Coordinator initializing...");
        Self::default()
    }

    /// Register a federal node.
    ///
    /// The node's capabilities are queried once and cached.
    pub fn register_node(&mut self, node: Arc<dyn FederationNode>) {
        self.registry.register(node);
    }

    /// Dispatch a federation message to the appropriate node.
    ///
    /// Uses the router to select a target, then calls the node's
    /// `handle()` method. If no suitable node is found, returns
    /// an error.
    pub async fn dispatch(&self, msg: FederationMessage) -> Result<FederationResponse, AgentError> {
        let target = self
            .router
            .route(&self.registry, &msg)
            .ok_or_else(|| AgentError::registry("No node available to handle message"))?;

        target.handle(msg).await
    }

    /// Send a message directly to a specific node by ID.
    pub async fn send_to(
        &self,
        node_id: &str,
        msg: FederationMessage,
    ) -> Result<FederationResponse, AgentError> {
        let target = self
            .registry
            .get(node_id)
            .ok_or_else(|| AgentError::registry(format!("Node '{}' not found", node_id)))?;

        target.handle(msg).await
    }

    /// Find all nodes that advertise a given capability type.
    pub fn nodes_with_capability(&self, cap_type: &str) -> Vec<Arc<dyn FederationNode>> {
        self.registry.nodes_with_capability(cap_type)
    }

    /// Get the number of registered nodes.
    pub fn node_count(&self) -> usize {
        self.registry.len()
    }

    /// List all registered node IDs.
    pub fn node_ids(&self) -> Vec<String> {
        self.registry.node_ids()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::Capability;

    struct EchoNode {
        id: String,
    }

    #[async_trait::async_trait]
    impl FederationNode for EchoNode {
        fn node_id(&self) -> &str {
            &self.id
        }
        fn capabilities(&self) -> Vec<Capability> {
            vec![]
        }
        async fn handle(&self, msg: FederationMessage) -> Result<FederationResponse, AgentError> {
            match msg {
                FederationMessage::Heartbeat { .. } => Ok(FederationResponse::Ack),
                _ => Ok(FederationResponse::Text(format!("echo from {}", self.id))),
            }
        }
    }

    #[tokio::test]
    async fn test_coordinator_register_and_dispatch() {
        let mut coord = Coordinator::new();
        let node = Arc::new(EchoNode { id: "echo".into() });
        coord.register_node(node);

        assert_eq!(coord.node_count(), 1);

        // Heartbeat to self → Ack
        let resp = coord
            .send_to(
                "echo",
                FederationMessage::Heartbeat {
                    node_id: "echo".into(),
                    status: clarity_contract::NodeStatus::Healthy,
                },
            )
            .await
            .unwrap();

        assert!(matches!(resp, FederationResponse::Ack));
    }

    #[tokio::test]
    async fn test_dispatch_to_missing_node() {
        let coord = Coordinator::new();
        let result = coord
            .send_to(
                "missing",
                FederationMessage::Heartbeat {
                    node_id: "missing".into(),
                    status: clarity_contract::NodeStatus::Healthy,
                },
            )
            .await;

        assert!(result.is_err());
    }
}
