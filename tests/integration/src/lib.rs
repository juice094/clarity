//! Cross-crate integration tests for the Clarity workspace.

#![cfg_attr(
    test,
    allow(
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::collapsible_if,
        missing_docs,
        unsafe_code
    )
)]
#[cfg(test)]
pub mod acp_bridge;
#[cfg(test)]
pub mod adaptive_loop;
/// Shared mock consumers for integration tests.
pub mod mock_consumer;
#[cfg(test)]
pub mod session_v2_migration;
#[cfg(test)]
pub mod subagent_api;
#[cfg(test)]
pub mod subagent_ws;
#[cfg(test)]
pub mod telemetry_end_to_end;
#[cfg(test)]
pub mod thread_api;
