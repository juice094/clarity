//! Agent Loop orchestration for the Claw runtime.
//!
//! Phase 1: Skeleton only. The actual agent loop lives in `clarity-core`.
//! This module will eventually wrap it with federal capabilities
//! (multi-agent dispatch, SOP templates, cron triggers).

use std::sync::Arc;

use clarity_contract::AgentError;
use clarity_core::agent::AgentExecutor;

/// A federal agent session that delegates turns to an underlying [`AgentExecutor`].
///
/// In Phase 1 this was a no-op struct. Phase 2 now holds the
/// `clarity_core::Agent` reference (via `Arc<dyn AgentExecutor>`) and
/// coordinates turn execution.
pub struct FederalAgentSession {
    session_id: String,
    agent: Arc<dyn AgentExecutor>,
}

impl FederalAgentSession {
    /// Create a new federal session wrapping the given agent executor.
    pub fn new(session_id: impl Into<String>, agent: Arc<dyn AgentExecutor>) -> Self {
        Self {
            session_id: session_id.into(),
            agent,
        }
    }

    /// Run a single turn, delegating to the underlying agent executor.
    pub async fn run_turn(&self, input: &str) -> Result<String, AgentError> {
        tracing::debug!(
            session_id = %self.session_id,
            input_len = input.len(),
            "FederalAgentSession::run_turn delegating to AgentExecutor"
        );
        self.agent.run_turn(input).await
    }

    /// Session ID.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Access the underlying agent executor.
    pub fn agent(&self) -> &Arc<dyn AgentExecutor> {
        &self.agent
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use parking_lot::Mutex;

    struct MockExecutor {
        result: Mutex<Result<String, AgentError>>,
        last_query: Mutex<Option<String>>,
    }

    impl MockExecutor {
        fn new(result: Result<String, AgentError>) -> Self {
            Self {
                result: Mutex::new(result),
                last_query: Mutex::new(None),
            }
        }
    }

    #[async_trait]
    impl AgentExecutor for MockExecutor {
        async fn run_turn(&self, query: &str) -> Result<String, AgentError> {
            let mut last = self.last_query.lock();
            *last = Some(query.to_string());
            self.result.lock().clone()
        }

        fn last_turn_message_count(&self) -> usize {
            0
        }
    }

    #[tokio::test]
    async fn run_turn_returns_agent_output() {
        let mock = Arc::new(MockExecutor::new(Ok("hello from mock".to_string())));
        let session = FederalAgentSession::new("test-1", mock.clone());

        let result = session.run_turn("user input").await;

        assert_eq!(result.unwrap(), "hello from mock");
        assert_eq!(mock.last_query.lock().as_ref().unwrap(), "user input");
    }

    #[tokio::test]
    async fn run_turn_propagates_error() {
        let err = AgentError::registry("mock failure");
        let mock = Arc::new(MockExecutor::new(Err(err.clone())));
        let session = FederalAgentSession::new("test-2", mock);

        let result = session.run_turn("user input").await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().to_string(), err.to_string());
    }

    #[tokio::test]
    async fn accessors_work() {
        let mock = Arc::new(MockExecutor::new(Ok("x".to_string())));
        let session = FederalAgentSession::new("sess-42", mock.clone());

        assert_eq!(session.session_id(), "sess-42");
        // Delegation through agent() accessor should reach the same mock.
        let via_accessor = session.agent().run_turn("q").await.unwrap();
        assert_eq!(via_accessor, "x");
    }
}
