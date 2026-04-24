//! Hook Registry — extensible interception points for the Agent lifecycle.
//!
//! Provides four lifecycle hooks that external code can register to
//! inspect, modify, or block agent behaviour at key decision points:
//!
//! - **PreDeliveryHook**: Intercept the final response before it reaches the user.
//! - **RoutingHook**: Influence how messages and tool calls are routed.
//! - **GoalAuditHook**: Audit sub-agent execution results against stated goals.
//! - **SessionTerminationHook**: Clean up or summarize when a session ends.
//!
//! # Example
//!
//! ```rust,no_run
//! use clarity_core::hooks::{HookRegistry, PreDeliveryHook, DeliveryTier};
//! use clarity_core::error::AgentError;
//! use async_trait::async_trait;
//!
//! struct LoggingHook;
//!
//! #[async_trait]
//! impl PreDeliveryHook for LoggingHook {
//!     async fn on_pre_delivery(&self, content: &str, tier: DeliveryTier) -> Result<String, AgentError> {
//!         println!("[{:?}] {}", tier, content);
//!         Ok(content.to_string())
//!     }
//! }
//! ```

use crate::error::AgentError;
use async_trait::async_trait;
use std::sync::Arc;

/// Delivery tier determines how aggressively a response should be reviewed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryTier {
    /// P0 — Direct delivery, no review (system confirmations, trivial acks).
    P0,
    /// P1 — Standard delivery, subject to PreDeliveryHook (most responses).
    P1,
    /// P2 — High-risk delivery, requires explicit confirmation before showing
    /// to the user (results of file edits, shell commands, etc.).
    P2,
    /// P3 — Internal-only, not delivered to the user (debug dumps, telemetry).
    P3,
}

/// Intercept the final assistant response before it is delivered to the user.
///
/// The hook receives the raw response content and its assigned delivery tier.
/// It may modify the content (e.g. append warnings, redact secrets) or reject
/// it entirely by returning an error.
#[async_trait]
pub trait PreDeliveryHook: Send + Sync {
    /// Called with the response content and its delivery tier.
    ///
    /// Returns the (possibly modified) content to deliver, or an error to
    /// block delivery.
    async fn on_pre_delivery(
        &self,
        content: &str,
        tier: DeliveryTier,
    ) -> Result<String, AgentError>;
}

/// Influence routing decisions for messages and tool calls.
///
/// Currently a placeholder for future expansion (e.g. routing certain requests
/// to specialised sub-agents or external services).
#[async_trait]
pub trait RoutingHook: Send + Sync {
    /// Called when the agent is about to process a model response that contains
    /// tool calls. The hook may inspect the message and tools and return a
    /// routing decision.
    async fn on_route(
        &self,
        message: &crate::llm::api::Message,
        tools: &[crate::types::ToolCall],
    ) -> Result<RoutingDecision, AgentError>;
}

/// Decision returned by a [`RoutingHook`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoutingDecision {
    /// Proceed with normal execution.
    Proceed,
    /// Re-route this turn to a different handler (not yet implemented).
    ReRoute,
    /// Block this turn entirely.
    Block,
}

/// Audit sub-agent execution results against their stated goals.
///
/// Used by Lazy Master (P1-9) and similar oversight mechanisms to verify
/// that a sub-agent actually accomplished what it was asked to do.
#[async_trait]
pub trait GoalAuditHook: Send + Sync {
    /// Called with the original goal and the execution result (often a
    /// transcript or summary). Returns whether the goal was achieved.
    async fn audit(&self, goal: &str, result: &str) -> Result<AuditResult, AgentError>;
}

/// Result of a goal audit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuditResult {
    /// The goal was fully achieved.
    Pass,
    /// The goal was partially achieved; retry or escalation may be needed.
    Partial,
    /// The goal was not achieved.
    Fail,
}

/// Called when a session (or turn) terminates.
///
/// Allows external code to perform cleanup, emit telemetry, or persist
/// session summaries.
#[async_trait]
pub trait SessionTerminationHook: Send + Sync {
    /// Called with a JSON-serialised summary of the session.
    async fn on_terminate(&self, session_summary: &str) -> Result<(), AgentError>;
}

/// Registry holding zero or more implementations of each hook type.
///
/// Hooks are executed in registration order. For [`PreDeliveryHook`] the
/// output of each hook is fed into the next, forming a pipeline.
#[derive(Clone, Default)]
pub struct HookRegistry {
    pre_delivery: Vec<Arc<dyn PreDeliveryHook>>,
    routing: Vec<Arc<dyn RoutingHook>>,
    goal_audit: Vec<Arc<dyn GoalAuditHook>>,
    session_termination: Vec<Arc<dyn SessionTerminationHook>>,
}

impl HookRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    // ------------------------------------------------------------------
    // Registration
    // ------------------------------------------------------------------

    /// Register a [`PreDeliveryHook`].
    pub fn register_pre_delivery<H: PreDeliveryHook + 'static>(mut self, hook: H) -> Self {
        self.pre_delivery.push(Arc::new(hook));
        self
    }

    /// Register a [`RoutingHook`].
    pub fn register_routing<H: RoutingHook + 'static>(mut self, hook: H) -> Self {
        self.routing.push(Arc::new(hook));
        self
    }

    /// Register a [`GoalAuditHook`].
    pub fn register_goal_audit<H: GoalAuditHook + 'static>(mut self, hook: H) -> Self {
        self.goal_audit.push(Arc::new(hook));
        self
    }

    /// Register a [`SessionTerminationHook`].
    pub fn register_session_termination<H: SessionTerminationHook + 'static>(
        mut self,
        hook: H,
    ) -> Self {
        self.session_termination.push(Arc::new(hook));
        self
    }

    // ------------------------------------------------------------------
    // Execution
    // ------------------------------------------------------------------

    /// Run all registered [`PreDeliveryHook`]s as a pipeline.
    ///
    /// The content is passed through each hook in order; the output of hook
    /// *i* becomes the input of hook *i+1*. If any hook returns an error,
    /// the pipeline short-circuits and the error is returned.
    pub async fn run_pre_delivery(
        &self,
        content: &str,
        tier: DeliveryTier,
    ) -> Result<String, AgentError> {
        let mut output = content.to_string();
        for hook in &self.pre_delivery {
            output = hook.on_pre_delivery(&output, tier).await?;
        }
        Ok(output)
    }

    /// Run all registered [`RoutingHook`]s.
    ///
    /// Returns the most restrictive decision (`Block` > `ReRoute` > `Proceed`).
    /// If no hooks are registered, returns [`RoutingDecision::Proceed`].
    pub async fn run_routing(
        &self,
        message: &crate::llm::api::Message,
        tools: &[crate::types::ToolCall],
    ) -> Result<RoutingDecision, AgentError> {
        let mut decision = RoutingDecision::Proceed;
        for hook in &self.routing {
            let d = hook.on_route(message, tools).await?;
            if d == RoutingDecision::Block {
                return Ok(RoutingDecision::Block);
            }
            if d == RoutingDecision::ReRoute {
                decision = RoutingDecision::ReRoute;
            }
        }
        Ok(decision)
    }

    /// Run all registered [`GoalAuditHook`]s.
    ///
    /// Returns the most restrictive result (`Fail` > `Partial` > `Pass`).
    /// If no hooks are registered, returns [`AuditResult::Pass`].
    pub async fn run_goal_audit(&self, goal: &str, result: &str) -> Result<AuditResult, AgentError> {
        let mut audit = AuditResult::Pass;
        for hook in &self.goal_audit {
            let r = hook.audit(goal, result).await?;
            if r == AuditResult::Fail {
                return Ok(AuditResult::Fail);
            }
            if r == AuditResult::Partial {
                audit = AuditResult::Partial;
            }
        }
        Ok(audit)
    }

    /// Run all registered [`SessionTerminationHook`]s.
    ///
    /// Errors from individual hooks are logged but do not short-circuit.
    pub async fn run_session_termination(&self, session_summary: &str) {
        for hook in &self.session_termination {
            if let Err(e) = hook.on_terminate(session_summary).await {
                tracing::warn!("SessionTerminationHook failed: {}", e);
            }
        }
    }

    /// Returns true if no hooks of any type are registered.
    pub fn is_empty(&self) -> bool {
        self.pre_delivery.is_empty()
            && self.routing.is_empty()
            && self.goal_audit.is_empty()
            && self.session_termination.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct AppendTierHook;

    #[async_trait]
    impl PreDeliveryHook for AppendTierHook {
        async fn on_pre_delivery(
            &self,
            content: &str,
            tier: DeliveryTier,
        ) -> Result<String, AgentError> {
            Ok(format!("{} [{:?}]", content, tier))
        }
    }

    struct BlockP2Hook;

    #[async_trait]
    impl PreDeliveryHook for BlockP2Hook {
        async fn on_pre_delivery(
            &self,
            _content: &str,
            tier: DeliveryTier,
        ) -> Result<String, AgentError> {
            if tier == DeliveryTier::P2 {
                Err(AgentError::ToolExecutionFailed(
                    "pre_delivery".to_string(),
                    "P2 content blocked".to_string(),
                ))
            } else {
                Ok("ok".to_string())
            }
        }
    }

    struct AlwaysBlockRouting;

    #[async_trait]
    impl RoutingHook for AlwaysBlockRouting {
        async fn on_route(
            &self,
            _message: &crate::llm::api::Message,
            _tools: &[crate::types::ToolCall],
        ) -> Result<RoutingDecision, AgentError> {
            Ok(RoutingDecision::Block)
        }
    }

    struct PassAudit;

    #[async_trait]
    impl GoalAuditHook for PassAudit {
        async fn audit(&self, _goal: &str, _result: &str) -> Result<AuditResult, AgentError> {
            Ok(AuditResult::Pass)
        }
    }

    struct FailAudit;

    #[async_trait]
    impl GoalAuditHook for FailAudit {
        async fn audit(&self, _goal: &str, _result: &str) -> Result<AuditResult, AgentError> {
            Ok(AuditResult::Fail)
        }
    }

    #[tokio::test]
    async fn test_pre_delivery_pipeline() {
        let registry = HookRegistry::new().register_pre_delivery(AppendTierHook);
        let result = registry
            .run_pre_delivery("hello", DeliveryTier::P1)
            .await
            .unwrap();
        assert_eq!(result, "hello [P1]");
    }

    #[tokio::test]
    async fn test_pre_delivery_block() {
        let registry = HookRegistry::new().register_pre_delivery(BlockP2Hook);
        assert!(registry.run_pre_delivery("x", DeliveryTier::P2).await.is_err());
        assert!(registry.run_pre_delivery("x", DeliveryTier::P1).await.is_ok());
    }

    #[tokio::test]
    async fn test_routing_block() {
        let registry = HookRegistry::new().register_routing(AlwaysBlockRouting);
        let msg = crate::llm::api::Message::user("test");
        let decision = registry.run_routing(&msg, &[]).await.unwrap();
        assert_eq!(decision, RoutingDecision::Block);
    }

    #[tokio::test]
    async fn test_goal_audit_fail_wins() {
        let registry = HookRegistry::new()
            .register_goal_audit(PassAudit)
            .register_goal_audit(FailAudit);
        let result = registry.run_goal_audit("g", "r").await.unwrap();
        assert_eq!(result, AuditResult::Fail);
    }

    #[tokio::test]
    async fn test_empty_registry() {
        let registry = HookRegistry::new();
        assert!(registry.is_empty());
        let result = registry
            .run_pre_delivery("hello", DeliveryTier::P0)
            .await
            .unwrap();
        assert_eq!(result, "hello");
    }
}
