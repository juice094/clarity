//! Federation types for the Claw runtime.
//!
//! These types enable cross-crate communication between federal nodes
//! (core, memory, egui, gateway, devbase, syncthing-rust) without
//! creating circular dependencies.

use crate::{AgentError, ContractError};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A node participating in the Claw federation.
///
/// Each crate that provides a federal capability implements this trait
/// and registers itself with the Coordinator at startup.
#[async_trait]
pub trait FederationNode: Send + Sync {
    /// Unique identifier for this node (e.g., "core", "memory", "egui").
    fn node_id(&self) -> &str;

    /// Capabilities advertised by this node.
    fn capabilities(&self) -> Vec<Capability>;

    /// Handle a federation message.
    ///
    /// The node should only process messages it understands;
    /// unknown messages can be ignored or return an error.
    async fn handle(&self, msg: FederationMessage) -> Result<FederationResponse, AgentError>;
}

/// A capability advertised by a federal node.
///
/// The Coordinator uses this to route requests to the appropriate node.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Capability {
    /// LLM inference with a list of supported models.
    LlmInference { models: Vec<String> },
    /// Tool execution with a list of available tools.
    ToolExecution { tools: Vec<ToolSpec> },
    /// Memory storage with a list of backends.
    MemoryStorage { backends: Vec<String> },
    /// Vector search with dimensionality.
    VectorSearch { dims: usize },
    /// MCP client connected to external servers.
    McpClient { servers: Vec<String> },
    /// Communication channel (e.g., chat, notification).
    Channel { name: String },
    /// OAuth token management.
    OAuth { provider: String },
}

/// Specification of a tool for capability advertisement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Value,
    pub requires_approval: bool,
}

/// A lightweight fact for federation communication.
///
/// This is a simplified version of `clarity_memory::Fact` that avoids
/// the `chrono` dependency in the contract layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    pub id: i64,
    pub fact: String,
    pub tags: Vec<String>,
    pub time: Option<String>,
    pub session_id: Option<String>,
}

/// Status of a federal node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    Healthy,
    Degraded,
    Offline,
}

/// Specification of a task for dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    pub task_id: String,
    pub name: String,
    pub prompt: String,
    pub max_iterations: usize,
    pub target_capability: Option<String>,
}

/// Messages sent between federal nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FederationMessage {
    /// Register capabilities with the Coordinator.
    RegisterCapability {
        node_id: String,
        capabilities: Vec<Capability>,
    },
    /// Dispatch a task to a specific node.
    DispatchTask { task: TaskSpec, target_node: String },
    /// Heartbeat from a node.
    Heartbeat { node_id: String, status: NodeStatus },
    /// Query the memory system.
    MemoryQuery {
        query: String,
        sender: String,
        limit: usize,
    },
    /// Response from a memory query.
    MemoryResponse {
        results: Vec<Fact>,
        request_id: String,
    },
    /// Request tool execution.
    ToolRequest {
        tool_name: String,
        arguments: Value,
        sender: String,
    },
    /// Response from tool execution.
    ToolResponse { result: Value, request_id: String },
    /// LLM inference request.
    LlmRequest {
        messages: Vec<crate::Message>,
        tools: Value,
        sender: String,
    },
    /// LLM inference response.
    LlmResponse {
        content: String,
        tool_calls: Vec<crate::ToolCall>,
        request_id: String,
    },
    /// Execute a full agent turn with a natural-language query.
    AgentTurn { query: String, sender: String },
}

/// Responses from federal node handlers.
#[derive(Debug, Clone)]
pub enum FederationResponse {
    /// Request acknowledged, no data to return.
    Ack,
    /// JSON payload.
    Json(Value),
    /// String payload.
    Text(String),
    /// Error occurred.
    Error(AgentError),
}

impl FederationResponse {
    /// Unwrap as JSON, or return an error.
    pub fn into_json(self) -> Result<Value, ContractError> {
        match self {
            Self::Json(v) => Ok(v),
            Self::Text(s) => {
                serde_json::from_str(&s).map_err(|e| AgentError::InvalidResponse(e.to_string()))
            }
            Self::Error(e) => Err(e),
            Self::Ack => Ok(Value::Null),
        }
    }

    /// Unwrap as text, or return an error.
    pub fn into_text(self) -> Result<String, ContractError> {
        match self {
            Self::Text(s) => Ok(s),
            Self::Json(v) => Ok(v.to_string()),
            Self::Error(e) => Err(e),
            Self::Ack => Ok(String::new()),
        }
    }
}
