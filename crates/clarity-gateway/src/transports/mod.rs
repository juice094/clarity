//! Server-side `ClawTransport` adapters for the native Gateway and OpenClaw
//! WebSocket endpoints.
//!
//! This module wraps the shared `clarity_contract::ClawTransport` trait around
//! `clarity-gateway`'s existing `AppState`, so `/ws` and `/openclaw/ws` can
//! share chat/history/sync logic without leaking protocol details into the
//! Axum handlers.

pub mod common;
pub mod gateway_ws;
pub mod openclaw;

pub use common::{
    ServerTransportContext, openclaw_message_from_contract, session_messages_to_history,
    transport_event_to_openclaw_frame, transport_event_to_ws_response,
    ws_request_to_message_context,
};
pub use gateway_ws::GatewayWebSocketTransport;
pub use openclaw::OpenClawServerTransport;
