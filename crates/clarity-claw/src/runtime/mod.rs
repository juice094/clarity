//! Claw Runtime — orchestration layer for the federal agent system.
//!
//! The Runtime holds a reference to the Coordinator and exposes
//! high-level operations like "run a task" or "chat with the agent".
//! In Phase 1 this is a skeleton; future phases will add SOP,
//! cron scheduling, and multi-agent orchestration.

use crate::coordinator::Coordinator;
use clarity_core::agent::AgentExecutor;
use std::sync::Arc;
use tokio::sync::RwLock;

pub mod agent_loop;

/// Claw Runtime — the top-level orchestrator.
///
/// Owns the Coordinator and provides safe concurrent access.
/// Designed to be shared across async tasks (tray loop, HTTP gateway,
/// background scheduler, etc.).
pub struct Runtime {
    coordinator: Arc<RwLock<Coordinator>>,
}

impl Runtime {
    /// Create a new runtime with the given coordinator.
    pub fn new(coordinator: Coordinator) -> Self {
        tracing::info!("Claw Runtime initializing...");
        Self {
            coordinator: Arc::new(RwLock::new(coordinator)),
        }
    }

    /// Access the coordinator (read lock).
    pub async fn coordinator(&self) -> tokio::sync::RwLockReadGuard<'_, Coordinator> {
        self.coordinator.read().await
    }

    /// Access the coordinator (write lock).
    pub async fn coordinator_mut(&self) -> tokio::sync::RwLockWriteGuard<'_, Coordinator> {
        self.coordinator.write().await
    }

    /// Get a clone of the coordinator Arc.
    pub fn coordinator_arc(&self) -> Arc<RwLock<Coordinator>> {
        self.coordinator.clone()
    }

    /// Register an agent executor as a federal CoreNode.
    pub async fn register_agent(&self, agent: Arc<dyn AgentExecutor>) {
        let node = Arc::new(crate::nodes::core_node::CoreNode::new(agent));
        let mut coordinator = self.coordinator.write().await;
        coordinator.register_node(node);
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new(Coordinator::new())
    }
}
