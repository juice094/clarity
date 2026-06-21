//! Claw Mesh: distributed role-context synchronization.
//!
//! This module unifies two transport strategies for sharing a `RoleContext`
//! across Claw devices:
//!
//! - **Gateway sync**: online, small-event synchronization via
//!   `clarity-gateway` (REST or WebSocket).
//! - **syncthing-rust**: offline-capable, file-based P2P synchronization using
//!   the Syncthing BEP protocol.
//!
//! Both transports feed events into the same CRDT [`merger`] to converge on a
//! single `RoleContext` per role.

pub mod crypto;
pub mod gateway_transport;
pub mod merger;
pub mod syncthing_transport;
pub mod transport;

pub use gateway_transport::GatewaySyncTransport;
pub use merger::{merge_events, merge_into};
pub use syncthing_transport::SyncthingTransport;
pub use transport::{
    MeshTransportError, NullTransport, Result, RoleContextTransport, TransportHandle,
};
