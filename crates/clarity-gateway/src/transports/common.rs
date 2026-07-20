//! Common conversion helpers for server-side ClawTransport adapters.

use std::sync::Arc;

use clarity_contract::{HistoryMessage, MessageRole, TransportEvent};

use crate::server::AppState;
use crate::session_store::SessionMessage;
use crate::ws::{ChatMessage, WsRequest, WsResponse};

/// Shared context passed to server-side transports.
pub struct ServerTransportContext {
    /// Shared Gateway application state.
    pub state: Arc<AppState>,
    /// Session / connection identifier for this transport.
    pub session_id: String,
}

impl ServerTransportContext {
    /// Create a new transport context.
    pub fn new(state: Arc<AppState>, session_id: impl Into<String>) -> Self {
        Self {
            state,
            session_id: session_id.into(),
        }
    }
}

/// Convert a native Gateway `WsRequest` into a transport-agnostic message
/// context, if the request carries chat semantics.
pub fn ws_request_to_message_context(req: WsRequest) -> Option<clarity_contract::MessageContext> {
    match req {
        WsRequest::Chat {
            message, context, ..
        } => {
            let mut ctx = clarity_contract::MessageContext {
                message,
                ..Default::default()
            };
            if let Some(context) = context {
                // ponytail: store the original request context as metadata so
                // the transport can forward it to streaming consumers if needed.
                if let Ok(json) = serde_json::to_string(&context) {
                    ctx.metadata.insert("context".into(), json);
                }
            }
            Some(ctx)
        }
        _ => None,
    }
}

/// Convert a transport event into a native Gateway `WsResponse`.
///
/// Returns `None` for events that do not have a direct `WsResponse` mapping
/// (e.g. `Done`, which should be interpreted by the caller as turn end).
pub fn transport_event_to_ws_response(ev: TransportEvent) -> Option<WsResponse> {
    match ev {
        TransportEvent::Connected {
            gateway_url,
            session_id,
        } => Some(WsResponse::Welcome {
            session_id: session_id.unwrap_or(gateway_url),
            message: "Connected to Clarity Gateway".into(),
        }),
        TransportEvent::ChatChunk { content } => Some(WsResponse::Chat {
            message: content,
            tool_calls: None,
        }),
        TransportEvent::History { messages } => Some(WsResponse::History {
            messages: messages
                .iter()
                .map(|m| ChatMessage {
                    role: role_to_string(&m.role),
                    content: m.content.clone(),
                    timestamp: chrono::Utc::now().to_rfc3339(),
                })
                .collect(),
        }),
        TransportEvent::RoleContextSynced {
            role_id: _role_id,
            events,
            next_cursor,
            online_devices,
        } => Some(WsResponse::RoleContextSynced {
            role_id: _role_id,
            events,
            next_cursor,
            online_devices,
        }),
        TransportEvent::Error { message } => Some(WsResponse::Error { error: message }),
        TransportEvent::WirePayload { payload } => Some(WsResponse::WireMessage { payload }),
        TransportEvent::Done => Some(WsResponse::Done),
        TransportEvent::ReasoningChunk { .. }
        | TransportEvent::Reconnecting { .. }
        | TransportEvent::Closed { .. }
        | TransportEvent::DevicePaired { .. }
        | TransportEvent::Unsupported { .. } => None,
    }
}

/// Convert stored session messages to contract `HistoryMessage`s.
pub fn session_messages_to_history(messages: &[SessionMessage]) -> Vec<HistoryMessage> {
    messages
        .iter()
        .map(|m| HistoryMessage {
            role: parse_role(&m.role),
            content: m.content.clone(),
            tool_calls: m
                .tool_calls
                .as_ref()
                .and_then(|t| serde_json::from_str::<Vec<clarity_contract::ToolCall>>(t).ok()),
            tool_call_id: m.tool_call_id.clone(),
        })
        .collect()
}

/// Convert stored session messages to contract `Message`s for the chat driver.
pub fn session_messages_to_contract_messages(
    messages: &[SessionMessage],
) -> Vec<clarity_contract::Message> {
    messages
        .iter()
        .map(|m| clarity_contract::Message {
            role: parse_role(&m.role),
            content: m.content.clone(),
            tool_calls: m
                .tool_calls
                .as_ref()
                .and_then(|t| serde_json::from_str::<Vec<clarity_contract::ToolCall>>(t).ok()),
            tool_call_id: m.tool_call_id.clone(),
        })
        .collect()
}

/// Build a `ChatMessage` from a contract message plus an explicit timestamp.
pub fn chat_message_from_contract(msg: &HistoryMessage, timestamp: &str) -> ChatMessage {
    ChatMessage {
        role: role_to_string(&msg.role),
        content: msg.content.clone(),
        timestamp: timestamp.into(),
    }
}

/// Convert a transport event into an OpenClaw JSON-RPC response frame.
pub fn transport_event_to_openclaw_frame(
    ev: TransportEvent,
    req_id: &str,
) -> clarity_contract::openclaw_protocol::OpenClawFrame {
    use clarity_contract::openclaw_protocol::{OpenClawErrorShape, OpenClawFrame};

    match ev {
        TransportEvent::Connected { .. } => OpenClawFrame::Event {
            event: "hello-ok".into(),
            payload: None,
            seq: None,
        },
        TransportEvent::ChatChunk { content } => {
            let payload = serde_json::json!({
                "role": "assistant",
                "content": content,
            });
            OpenClawFrame::Event {
                event: "chat".into(),
                payload: Some(payload),
                seq: None,
            }
        }
        TransportEvent::Done => OpenClawFrame::Event {
            event: "done".into(),
            payload: None,
            seq: None,
        },
        TransportEvent::History { messages } => {
            let payload = serde_json::json!({
                "messages": messages.iter().map(openclaw_message_from_contract).collect::<Vec<_>>(),
            });
            OpenClawFrame::Res {
                id: req_id.into(),
                ok: true,
                payload: Some(payload),
                error: None,
            }
        }
        TransportEvent::Error { message } => OpenClawFrame::Res {
            id: req_id.into(),
            ok: false,
            payload: None,
            error: Some(OpenClawErrorShape {
                code: "TRANSPORT_ERROR".into(),
                message,
                details: None,
                retryable: Some(false),
                retry_after_ms: None,
            }),
        },
        TransportEvent::RoleContextSynced {
            role_id: _role_id,
            events,
            next_cursor,
            online_devices,
        } => {
            let payload = serde_json::to_value(clarity_contract::SyncResponse {
                events,
                next_cursor,
                online_devices,
            })
            .unwrap_or_default();
            OpenClawFrame::Event {
                event: "role_context.synced".into(),
                payload: Some(payload),
                seq: None,
            }
        }
        TransportEvent::DevicePaired {
            device_id,
            approved,
            token,
            scopes,
        } => {
            let payload = serde_json::json!({
                "deviceId": device_id,
                "approved": approved,
                "token": token,
                "scopes": scopes,
            });
            OpenClawFrame::Event {
                event: "device.paired".into(),
                payload: Some(payload),
                seq: None,
            }
        }
        _ => OpenClawFrame::Res {
            id: req_id.into(),
            ok: false,
            payload: None,
            error: Some(OpenClawErrorShape {
                code: "UNSUPPORTED".into(),
                message: "event not supported over OpenClaw".into(),
                details: None,
                retryable: Some(false),
                retry_after_ms: None,
            }),
        },
    }
}

/// Convert a contract `HistoryMessage` to an `OpenClawMessage` payload.
pub fn openclaw_message_from_contract(
    msg: &HistoryMessage,
) -> crate::openclaw_server::protocol::OpenClawMessage {
    crate::openclaw_server::protocol::OpenClawMessage {
        role: role_to_string(&msg.role),
        content: Some(msg.content.clone()),
        blocks: None,
        timestamp_ms: None,
        id: msg.tool_call_id.clone(),
    }
}

fn parse_role(role: &str) -> MessageRole {
    match role.to_ascii_lowercase().as_str() {
        "system" => MessageRole::System,
        "user" => MessageRole::User,
        "assistant" => MessageRole::Assistant,
        "tool" => MessageRole::Tool,
        _ => MessageRole::User,
    }
}

fn role_to_string(role: &MessageRole) -> String {
    match role {
        MessageRole::System => "system".into(),
        MessageRole::User => "user".into(),
        MessageRole::Assistant => "assistant".into(),
        MessageRole::Tool => "tool".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ws_chat_request_to_context() {
        let req = WsRequest::Chat {
            message: "hi".into(),
            context: None,
            use_wire: false,
        };
        let ctx = ws_request_to_message_context(req).unwrap();
        assert_eq!(ctx.message, "hi");
    }

    #[test]
    fn transport_event_chat_chunk_to_ws_response() {
        let ev = TransportEvent::ChatChunk {
            content: "hello".into(),
        };
        let resp = transport_event_to_ws_response(ev).unwrap();
        assert!(matches!(resp, WsResponse::Chat { message, .. } if message == "hello"));
    }

    #[test]
    fn transport_event_done_maps_to_done_response() {
        let resp = transport_event_to_ws_response(TransportEvent::Done);
        assert!(matches!(resp, Some(WsResponse::Done)));
    }

    #[test]
    fn session_messages_to_history_parses_roles() {
        let msgs = vec![
            SessionMessage::new("user", "hi"),
            SessionMessage::new("assistant", "hello"),
        ];
        let history = session_messages_to_history(&msgs);
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].role, MessageRole::User);
        assert_eq!(history[1].role, MessageRole::Assistant);
    }
}
