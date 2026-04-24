//! Server module — JSON-RPC over stdio for exposing AgentController
//!
//! See [`stdio::StdioServer`] for the primary interface.

pub mod stdio;

pub use stdio::{StdioServer, StdioServerError};
