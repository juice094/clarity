//! CoreNode — bridges the Claw Coordinator to clarity-core's AgentExecutor.
//!
//! Implements `FederationNode` so that federation messages can trigger
//! full ReAct agent turns via `AgentExecutor::run_turn`.

use std::sync::Arc;
use clarity_contract::{AgentError, Capability, FederationMessage, FederationNode, FederationResponse};
use clarity_core::agent::AgentExecutor;

/// Federal node wrapping an `AgentExecutor`.
pub struct CoreNode {
    agent: Arc<dyn AgentExecutor>,
}

impl CoreNode {
    /// Create a new CoreNode backed by the given agent executor.
    pub fn new(agent: Arc<dyn AgentExecutor>) -> Self {
        Self { agent }
    }
}

#[async_trait::async_trait]
impl FederationNode for CoreNode {
    fn node_id(&self) -> &str {
        "core"
    }

    fn capabilities(&self) -> Vec<Capability> {
        vec![Capability::Channel {
            name: "agent_executor".into(),
        }]
    }

    async fn handle(
        &self,
        msg: FederationMessage,
    ) -> Result<FederationResponse, AgentError> {
        match msg {
            FederationMessage::AgentTurn { query, .. } => {
                match self.agent.run_turn(&query).await {
                    Ok(content) => Ok(FederationResponse::Text(content)),
                    Err(e) => Ok(FederationResponse::Error(e)),
                }
            }
            FederationMessage::Heartbeat { .. } => Ok(FederationResponse::Ack),
            _ => Ok(FederationResponse::Error(AgentError::registry(
                format!("CoreNode does not handle {:?}", msg),
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clarity_contract::NodeStatus;

    struct MockAgent;

    #[async_trait::async_trait]
    impl AgentExecutor for MockAgent {
        async fn run_turn(&self, query: &str) -> Result<String, AgentError> {
            Ok(format!("mock: {}", query))
        }
    }

    #[tokio::test]
    async fn test_core_node_agent_turn_returns_text() {
        let agent = Arc::new(MockAgent);
        let node = CoreNode::new(agent);

        let msg = FederationMessage::AgentTurn {
            query: "hello".into(),
            sender: "test".into(),
        };
        let resp = node.handle(msg).await.unwrap();

        assert!(matches!(resp, FederationResponse::Text(ref t) if t == "mock: hello"));
    }

    #[tokio::test]
    async fn test_core_node_heartbeat_returns_ack() {
        let agent = Arc::new(MockAgent);
        let node = CoreNode::new(agent);

        let msg = FederationMessage::Heartbeat {
            node_id: "core".into(),
            status: NodeStatus::Healthy,
        };
        let resp = node.handle(msg).await.unwrap();

        assert!(matches!(resp, FederationResponse::Ack));
    }

    #[test]
    fn test_core_node_capabilities_include_agent_executor() {
        let agent = Arc::new(MockAgent);
        let node = CoreNode::new(agent);

        let caps = node.capabilities();
        assert!(caps.iter().any(|c| matches!(c, Capability::Channel { name } if name == "agent_executor")));
    }
}
