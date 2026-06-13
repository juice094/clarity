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
    LlmInference {
        /// Supported model identifiers.
        models: Vec<String>,
    },
    /// Tool execution with a list of available tools.
    ToolExecution {
        /// Available tool specifications.
        tools: Vec<ToolSpec>,
    },
    /// Memory storage with a list of backends.
    MemoryStorage {
        /// Supported storage backend names.
        backends: Vec<String>,
    },
    /// Vector search with dimensionality.
    VectorSearch {
        /// Vector dimension size.
        dims: usize,
    },
    /// MCP client connected to external servers.
    McpClient {
        /// Connected MCP server names.
        servers: Vec<String>,
    },
    /// Communication channel (e.g., chat, notification).
    Channel {
        /// Channel identifier.
        name: String,
    },
    /// OAuth token management.
    OAuth {
        /// OAuth provider identifier.
        provider: String,
    },
}

/// Specification of a tool for capability advertisement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolSpec {
    /// Tool name.
    pub name: String,
    /// Human-readable description.
    pub description: String,
    /// JSON Schema for tool parameters.
    pub parameters: Value,
    /// Whether the tool requires explicit user approval.
    pub requires_approval: bool,
}

/// A lightweight fact for federation communication.
///
/// This is a simplified version of `clarity_memory::Fact` that avoids
/// the `chrono` dependency in the contract layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fact {
    /// Fact identifier.
    pub id: i64,
    /// Fact content.
    pub fact: String,
    /// Tags for categorization.
    pub tags: Vec<String>,
    /// Optional ISO-8601 timestamp.
    pub time: Option<String>,
    /// Optional session identifier.
    pub session_id: Option<String>,
}

/// Status of a federal node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeStatus {
    /// Node is fully operational.
    Healthy,
    /// Node is operational with reduced capacity.
    Degraded,
    /// Node is unreachable.
    Offline,
}

/// Specification of a task for dispatch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSpec {
    /// Unique task identifier.
    pub task_id: String,
    /// Human-readable task name.
    pub name: String,
    /// Prompt or instructions for the task.
    pub prompt: String,
    /// Maximum allowed iterations.
    pub max_iterations: usize,
    /// Optional target capability for routing.
    pub target_capability: Option<String>,
}

/// Messages sent between federal nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FederationMessage {
    /// Register capabilities with the Coordinator.
    RegisterCapability {
        /// Node identifier.
        node_id: String,
        /// Capabilities advertised by this node.
        capabilities: Vec<Capability>,
    },
    /// Dispatch a task to a specific node.
    DispatchTask {
        /// Task to execute.
        task: TaskSpec,
        /// Identifier of the target node.
        target_node: String,
    },
    /// Heartbeat from a node.
    Heartbeat {
        /// Node identifier.
        node_id: String,
        /// Current node status.
        status: NodeStatus,
    },
    /// Query the memory system.
    MemoryQuery {
        /// Natural-language query string.
        query: String,
        /// Sender node identifier.
        sender: String,
        /// Maximum number of results to return.
        limit: usize,
    },
    /// Response from a memory query.
    MemoryResponse {
        /// Matching facts.
        results: Vec<Fact>,
        /// Identifier matching the original query.
        request_id: String,
    },
    /// Request tool execution.
    ToolRequest {
        /// Name of the tool to execute.
        tool_name: String,
        /// JSON-encoded arguments.
        arguments: Value,
        /// Sender node identifier.
        sender: String,
    },
    /// Response from tool execution.
    ToolResponse {
        /// Tool execution result.
        result: Value,
        /// Identifier matching the original request.
        request_id: String,
    },
    /// LLM inference request.
    LlmRequest {
        /// Message history for the LLM.
        messages: Vec<crate::Message>,
        /// Available tools as a JSON value.
        tools: Value,
        /// Sender node identifier.
        sender: String,
    },
    /// LLM inference response.
    LlmResponse {
        /// Text content of the response.
        content: String,
        /// Tool calls requested by the model.
        tool_calls: Vec<crate::ToolCall>,
        /// Identifier matching the original request.
        request_id: String,
    },
    /// Execute a full agent turn with a natural-language query.
    AgentTurn {
        /// Natural-language query.
        query: String,
        /// Sender node identifier.
        sender: String,
    },
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
