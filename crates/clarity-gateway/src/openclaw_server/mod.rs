//! OpenClaw-compatible JSON-RPC WebSocket endpoint.
//!
//! This module lets `clarity-gateway` speak the same protocol as Kimi Desktop's
//! local OpenClaw Gateway (`127.0.0.1:18679`). When Kimi Desktop is removed,
//! Clarity clients (`clarity-claw`, `clarity-headless acp-bridge`) can connect
//! to `ws://127.0.0.1:18790/openclaw/ws` instead.

pub mod auth;
pub mod handler;
pub mod protocol;
pub mod state;

pub use handler::ws_handler;
pub use state::OpenClawServerState;
