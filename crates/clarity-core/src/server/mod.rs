//! Server module — JSON-RPC over stdio for exposing AgentController
//!
//! See [`stdio::StdioServer`] for the primary interface.
//!
//! NOTE: This module is currently inactive and kept for future integration.
#![allow(dead_code, unused_imports)]

pub mod stdio;

pub use stdio::{StdioServer, StdioServerError};
