//! `ClawTransport` adapter implementations.
//!
//! This module contains concrete adapters that turn the legacy channel-based
//! `GatewayClient` and `ClawClient` into `clarity_contract::ClawTransport`
//! implementations, plus a `TransportManager` that exposes a synchronous poll
//! interface on top of any transport.

pub mod gateway_ws;
pub mod manager;
pub mod openclaw;

pub use gateway_ws::GatewayWebSocketTransport;
pub use manager::TransportManager;
pub use openclaw::OpenClawTransport;
