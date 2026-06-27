//! JSON-RPC server over stdio for exposing AgentController
//!
//! Reads JSON-RPC requests from stdin (one JSON object per line),
//! drives an Agent via AgentController, and writes responses/events
//! to stdout (one JSON object per line).
//!
//! # Protocol
//!
//! ## Request
//! ```json
//! {"jsonrpc":"2.0","id":1,"method":"agent/run","params":{"prompt":"Hello"}}
//! ```
//!
//! ## Event notification (streaming)
//! ```json
//! {"jsonrpc":"2.0","method":"event","params":{"type":"chunk","data":"..."}}
//! ```
//!
//! ## Response
//! ```json
//! {"jsonrpc":"2.0","id":1,"result":{"response":"...","status":"complete"}}
//! ```

use crate::agent::Agent;
use crate::agent::controller::{AgentController, ControllerEvent};
use crate::agent::ops::Op;
use crate::error::AgentError;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Write;
use thiserror::Error;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

/// Errors from the stdio server
#[derive(Debug, Error)]
pub enum StdioServerError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Agent error: {0}")]
    Agent(#[from] AgentError),
    #[error("Invalid request: {0}")]
    InvalidRequest(String),
    #[error("Method not found: {0}")]
    MethodNotFound(String),
    #[error("Channel closed")]
    ChannelClosed,
}

/// JSON-RPC 2.0 request
#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    jsonrpc: String,
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

/// JSON-RPC 2.0 response
#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error
#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<Value>,
}

/// JSON-RPC 2.0 notification (no id)
#[derive(Debug, Serialize)]
struct JsonRpcNotification {
    jsonrpc: String,
    method: String,
    params: Value,
}

/// Stdio-based JSON-RPC server for AgentController
pub struct StdioServer {
    agent: Agent,
}

impl StdioServer {
    /// Create a new stdio server wrapping the given agent
    pub fn new(agent: Agent) -> Self {
        Self { agent }
    }

    /// Run the server loop
    ///
    /// Reads requests from stdin, drives the agent, writes events/responses
    /// to stdout until EOF or `agent/shutdown` is received.
    pub async fn run(self) -> Result<(), StdioServerError> {
        let (event_tx, mut event_rx) = mpsc::unbounded_channel::<ControllerEvent>();
        let (controller, op_tx) = AgentController::new_with_events(self.agent, event_tx, None);

        // Spawn the controller event loop
        let controller_handle = tokio::spawn(controller.run());

        // Spawn event forwarder: ControllerEvent -> stdout notification
        let event_forwarder = tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                let notification = Self::event_to_notification(event);
                if let Err(e) = Self::write_stdout(&notification) {
                    warn!("Failed to write event to stdout: {}", e);
                    break;
                }
            }
        });

        // Read requests from stdin
        let stdin = tokio::io::stdin();
        let reader = BufReader::new(stdin);
        let mut lines = reader.lines();

        info!("StdioServer ready. Reading JSON-RPC requests from stdin...");

        while let Some(line) = lines.next_line().await? {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            debug!("Received line: {}", line);

            let request: JsonRpcRequest = match serde_json::from_str(line) {
                Ok(req) => req,
                Err(e) => {
                    let response = JsonRpcResponse {
                        jsonrpc: "2.0".to_string(),
                        id: None,
                        result: None,
                        error: Some(JsonRpcError {
                            code: -32700,
                            message: format!("Parse error: {}", e),
                            data: None,
                        }),
                    };
                    Self::write_stdout(&response)?;
                    continue;
                }
            };

            let id = request.id.clone();

            match Self::handle_request(request, &op_tx).await {
                Ok(result) => {
                    if let Some(id) = id {
                        let response = JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: Some(id),
                            result: Some(result),
                            error: None,
                        };
                        Self::write_stdout(&response)?;
                    }
                }
                Err(e) => {
                    if let Some(id) = id {
                        let response = JsonRpcResponse {
                            jsonrpc: "2.0".to_string(),
                            id: Some(id),
                            result: None,
                            error: Some(JsonRpcError {
                                code: -32603,
                                message: e.to_string(),
                                data: None,
                            }),
                        };
                        Self::write_stdout(&response)?;
                    }
                }
            }
        }

        // EOF reached — send shutdown
        let _ = op_tx.send(Op::Shutdown);

        // Wait for controller to finish
        let _ = controller_handle.await;
        let _ = event_forwarder.await;

        info!("StdioServer shutting down");
        Ok(())
    }

    async fn handle_request(
        request: JsonRpcRequest,
        op_tx: &mpsc::UnboundedSender<Op>,
    ) -> Result<Value, StdioServerError> {
        if request.jsonrpc != "2.0" {
            return Err(StdioServerError::InvalidRequest(
                "Invalid JSON-RPC version".to_string(),
            ));
        }

        match request.method.as_str() {
            "agent/run" => {
                let params = request.params.unwrap_or(Value::Null);
                let prompt = params
                    .get("prompt")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        StdioServerError::InvalidRequest("Missing 'prompt' parameter".to_string())
                    })?;

                op_tx
                    .send(Op::user_turn(prompt.to_string()))
                    .map_err(|_| StdioServerError::ChannelClosed)?;

                Ok(json!({"status": "started"}))
            }

            "agent/interrupt" => {
                op_tx
                    .send(Op::Interrupt)
                    .map_err(|_| StdioServerError::ChannelClosed)?;
                Ok(json!({"status": "interrupted"}))
            }

            "agent/compact" => {
                op_tx
                    .send(Op::Compact)
                    .map_err(|_| StdioServerError::ChannelClosed)?;
                Ok(json!({"status": "compaction_triggered"}))
            }

            "agent/shutdown" => {
                op_tx
                    .send(Op::Shutdown)
                    .map_err(|_| StdioServerError::ChannelClosed)?;
                Ok(json!({"status": "shutdown_initiated"}))
            }

            _ => Err(StdioServerError::MethodNotFound(request.method)),
        }
    }

    fn event_to_notification(event: ControllerEvent) -> JsonRpcNotification {
        let (event_type, data) = match event {
            ControllerEvent::Chunk(text) => ("chunk", json!({"text": text})),
            ControllerEvent::Complete(response) => ("complete", json!({"response": response})),
            ControllerEvent::Error(error) => ("error", json!({"error": error})),
            ControllerEvent::ToolCallStart {
                id,
                name,
                arguments,
            } => (
                "tool_call_start",
                json!({"id": id, "name": name, "arguments": arguments}),
            ),
            ControllerEvent::ToolResult { id, result } => {
                ("tool_result", json!({"id": id, "result": result}))
            }
            ControllerEvent::StepBegin { tool_name } => {
                ("step_begin", json!({"tool_name": tool_name}))
            }
        };

        JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "event".to_string(),
            params: json!({"type": event_type, "data": data}),
        }
    }

    fn write_stdout<T: Serialize>(value: &T) -> Result<(), StdioServerError> {
        let json = serde_json::to_string(value)?;
        let mut stdout = std::io::stdout();
        writeln!(stdout, "{}", json)?;
        stdout.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_json_rpc_request_deserialization() {
        let json = r#"{"jsonrpc":"2.0","id":1,"method":"agent/run","params":{"prompt":"Hello"}}"#;
        let req: JsonRpcRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.jsonrpc, "2.0");
        assert_eq!(req.method, "agent/run");
        assert!(req.params.is_some());
    }

    #[test]
    fn test_json_rpc_response_serialization() {
        use serde_json::json;
        let resp = JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id: Some(json!(1)),
            result: Some(json!({"status": "ok"})),
            error: None,
        };
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"id\":1"));
    }

    #[test]
    fn test_event_to_notification() {
        let event = ControllerEvent::Chunk("Hello".to_string());
        let notif = StdioServer::event_to_notification(event);
        assert_eq!(notif.method, "event");

        let params = notif.params;
        assert_eq!(params["type"].as_str().unwrap(), "chunk");
        assert_eq!(params["data"]["text"].as_str().unwrap(), "Hello");
    }
}
