//! Capability registry for the Claw federation.
//!
//! Maintains the mapping between node IDs, their capabilities,
//! and their runtime references.

use clarity_contract::{Capability, FederationNode};
use std::collections::HashMap;
use std::sync::Arc;

/// Registry of all federal nodes and their advertised capabilities.
#[derive(Default)]
pub struct CapabilityRegistry {
    /// node_id → node reference
    nodes: HashMap<String, Arc<dyn FederationNode>>,
    /// node_id → capabilities
    capabilities: HashMap<String, Vec<Capability>>,
}

impl CapabilityRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a node and snapshot its capabilities.
    pub fn register(&mut self, node: Arc<dyn FederationNode>) {
        let id = node.node_id().to_string();
        let caps = node.capabilities();
        tracing::info!(node_id = %id, cap_count = caps.len(), "Registering federal node");
        self.nodes.insert(id.clone(), node);
        self.capabilities.insert(id, caps);
    }

    /// Get a node by its ID.
    pub fn get(&self, node_id: &str) -> Option<Arc<dyn FederationNode>> {
        self.nodes.get(node_id).cloned()
    }

    /// Get capabilities advertised by a specific node.
    pub fn capabilities_of(&self, node_id: &str) -> Vec<Capability> {
        self.capabilities.get(node_id).cloned().unwrap_or_default()
    }

    /// Find all nodes that advertise a given capability type.
    pub fn nodes_with_capability(&self, cap_type: &str) -> Vec<Arc<dyn FederationNode>> {
        self.capabilities
            .iter()
            .filter(|(_, caps)| caps.iter().any(|c| capability_type_name(c) == cap_type))
            .filter_map(|(id, _)| self.nodes.get(id).cloned())
            .collect()
    }

    /// List all registered node IDs.
    pub fn node_ids(&self) -> Vec<String> {
        self.nodes.keys().cloned().collect()
    }

    /// Total number of registered nodes.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the registry is empty.
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// Return a stable type name for a capability variant.
fn capability_type_name(cap: &Capability) -> &'static str {
    match cap {
        Capability::LlmInference { .. } => "llm_inference",
        Capability::ToolExecution { .. } => "tool_execution",
        Capability::MemoryStorage { .. } => "memory_storage",
        Capability::VectorSearch { .. } => "vector_search",
        Capability::McpClient { .. } => "mcp_client",
        Capability::Channel { .. } => "channel",
        Capability::OAuth { .. } => "oauth",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::{AgentError, FederationMessage, FederationResponse};

    struct DummyNode {
        id: String,
    }

    #[async_trait::async_trait]
    impl FederationNode for DummyNode {
        fn node_id(&self) -> &str {
            &self.id
        }
        fn capabilities(&self) -> Vec<Capability> {
            vec![Capability::LlmInference {
                models: vec!["test".into()],
            }]
        }
        async fn handle(&self, _msg: FederationMessage) -> Result<FederationResponse, AgentError> {
            Ok(FederationResponse::Ack)
        }
    }

    #[test]
    fn test_register_and_get() {
        let mut reg = CapabilityRegistry::new();
        let node = Arc::new(DummyNode {
            id: "test-node".into(),
        });
        reg.register(node);

        assert_eq!(reg.len(), 1);
        assert!(reg.get("test-node").is_some());
        assert!(reg.get("missing").is_none());
    }

    #[test]
    fn test_capabilities_of() {
        let mut reg = CapabilityRegistry::new();
        let node = Arc::new(DummyNode { id: "core".into() });
        reg.register(node);

        let caps = reg.capabilities_of("core");
        assert_eq!(caps.len(), 1);
    }

    #[test]
    fn test_nodes_with_capability() {
        let mut reg = CapabilityRegistry::new();
        let node = Arc::new(DummyNode { id: "core".into() });
        reg.register(node);

        let llm_nodes = reg.nodes_with_capability("llm_inference");
        assert_eq!(llm_nodes.len(), 1);

        let empty = reg.nodes_with_capability("tool_execution");
        assert!(empty.is_empty());
    }
}
