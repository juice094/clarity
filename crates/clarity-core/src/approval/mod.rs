//! Approval runtime for tool call authorization
//!
//! This module provides an asynchronous approval system for tool calls,
//! supporting different approval modes and both foreground and background contexts.

use crate::error::AgentError;
use crate::types::ToolCall;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;
use tokio::sync::oneshot;
use uuid::Uuid;

/// The source of an approval request
#[derive(Debug, Clone)]
pub enum ApprovalSource {
    /// Foreground turn context
    ForegroundTurn { turn_id: String },
    /// Background agent context
    BackgroundAgent { task_id: String, agent_id: String },
}

impl ApprovalSource {
    /// Returns a stable session key for this approval source
    fn session_key(&self) -> String {
        match self {
            ApprovalSource::ForegroundTurn { turn_id } => format!("turn:{}", turn_id),
            ApprovalSource::BackgroundAgent { task_id, agent_id } => {
                format!("agent:{}:{}", agent_id, task_id)
            }
        }
    }
}

/// Response to an approval request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalResponse {
    /// Approve this request
    Approve,
    /// Approve and remember for this session
    ApproveForSession,
    /// Reject this request
    Reject,
}

/// Status of an approval request
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalStatus {
    /// Waiting for approval
    Pending,
    /// Has been resolved
    Resolved,
    /// Was cancelled
    Cancelled,
}

/// An approval request record
#[derive(Debug)]
pub struct ApprovalRequest {
    /// Unique request ID
    pub id: String,
    /// The tool call being approved
    pub tool_call: ToolCall,
    /// Source of the request
    pub source: ApprovalSource,
    /// Current status
    pub status: ApprovalStatus,
    /// When the request was created
    pub created_at: Instant,
    /// Optional description highlighting sensitive nature or other concerns
    pub description: Option<String>,
}

impl ApprovalRequest {
    /// Create a new approval request
    fn new(tool_call: ToolCall, source: ApprovalSource, description: Option<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            tool_call,
            source,
            status: ApprovalStatus::Pending,
            created_at: Instant::now(),
            description,
        }
    }
}

/// Core trait for approval runtimes
#[async_trait]
pub trait ApprovalRuntime: Send + Sync {
    /// Create an approval request and return its ID
    ///
    /// # Arguments
    ///
    /// * `tool_call` - The tool call to be approved
    /// * `source` - The source context of the request
    ///
    /// # Returns
    ///
    /// The unique request ID for tracking
    async fn create_request(
        &self,
        tool_call: &ToolCall,
        source: ApprovalSource,
        description: Option<String>,
    ) -> Result<String, AgentError>;

    /// Wait for a response to an approval request
    ///
    /// # Arguments
    ///
    /// * `request_id` - The ID of the request to wait for
    ///
    /// # Returns
    ///
    /// The approval response when available
    async fn wait_for_response(&self, request_id: &str) -> Result<ApprovalResponse, AgentError>;

    /// Resolve an approval request (typically called by UI)
    ///
    /// # Arguments
    ///
    /// * `request_id` - The ID of the request to resolve
    /// * `response` - The response to set
    async fn resolve(&self, request_id: &str, response: ApprovalResponse)
        -> Result<(), AgentError>;
}

/// Approval mode determining automatic vs manual approval behavior
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ApprovalMode {
    /// Interactive mode - wait for user confirmation
    #[default]
    Interactive,
    /// YOLO mode - automatically approve everything
    Yolo,
    /// Plan mode - special handling for planning context
    Plan,
}

/// Wrapper that adds mode-aware behavior to any approval runtime
pub struct ModeAwareApprovalRuntime<R: ApprovalRuntime> {
    inner: R,
    mode: ApprovalMode,
    session_approvals: Mutex<HashMap<String, ()>>,
    request_sessions: Mutex<HashMap<String, String>>,
}

impl<R: ApprovalRuntime> ModeAwareApprovalRuntime<R> {
    /// Create a new mode-aware wrapper
    pub fn new(inner: R, mode: ApprovalMode) -> Self {
        Self {
            inner,
            mode,
            session_approvals: Mutex::new(HashMap::new()),
            request_sessions: Mutex::new(HashMap::new()),
        }
    }

    /// Get the current approval mode
    pub fn mode(&self) -> ApprovalMode {
        self.mode
    }

    /// Set the approval mode
    pub fn set_mode(&mut self, mode: ApprovalMode) {
        self.mode = mode;
    }

    /// Get a reference to the inner runtime
    pub fn inner(&self) -> &R {
        &self.inner
    }

    /// Get a mutable reference to the inner runtime
    pub fn inner_mut(&mut self) -> &mut R {
        &mut self.inner
    }

    /// Unwrap to get the inner runtime
    pub fn into_inner(self) -> R {
        self.inner
    }

    /// Approve all requests for a given session key
    pub fn approve_session(&self, session_key: &str) {
        if let Ok(mut approvals) = self.session_approvals.lock() {
            approvals.insert(session_key.to_string(), ());
        }
    }
}

#[async_trait]
impl<R: ApprovalRuntime> ApprovalRuntime for ModeAwareApprovalRuntime<R> {
    async fn create_request(
        &self,
        tool_call: &ToolCall,
        source: ApprovalSource,
        description: Option<String>,
    ) -> Result<String, AgentError> {
        let session_key = source.session_key();
        let request_id = self
            .inner
            .create_request(tool_call, source.clone(), description)
            .await?;
        if let Ok(mut sessions) = self.request_sessions.lock() {
            sessions.insert(request_id.clone(), session_key.clone());
        }
        // Auto-approve if session is already approved
        let should_auto_approve = if let Ok(approvals) = self.session_approvals.lock() {
            approvals.contains_key(&session_key)
        } else {
            false
        };
        if should_auto_approve {
            let _ = self
                .inner
                .resolve(&request_id, ApprovalResponse::Approve)
                .await;
        }
        Ok(request_id)
    }

    async fn wait_for_response(&self, request_id: &str) -> Result<ApprovalResponse, AgentError> {
        match self.mode {
            ApprovalMode::Yolo => Ok(ApprovalResponse::Approve),
            ApprovalMode::Plan | ApprovalMode::Interactive => {
                // Check if session already approved
                if let Ok(sessions) = self.request_sessions.lock() {
                    if let Some(session_key) = sessions.get(request_id) {
                        if let Ok(approvals) = self.session_approvals.lock() {
                            if approvals.contains_key(session_key) {
                                return Ok(ApprovalResponse::Approve);
                            }
                        }
                    }
                }

                self.inner.wait_for_response(request_id).await
            }
        }
    }

    async fn resolve(
        &self,
        request_id: &str,
        response: ApprovalResponse,
    ) -> Result<(), AgentError> {
        if response == ApprovalResponse::ApproveForSession {
            if let Ok(sessions) = self.request_sessions.lock() {
                if let Some(session_key) = sessions.get(request_id) {
                    self.approve_session(session_key);
                }
            }
            // Also resolve the current request as Approve to wake up the waiter.
            // Without this, wait_for_response() would block forever since
            // session_approvals is only checked on entry, not while waiting.
            return self.inner.resolve(request_id, ApprovalResponse::Approve).await;
        }
        self.inner.resolve(request_id, response).await
    }
}

/// In-memory implementation of the approval runtime
///
/// This is suitable for testing and single-process applications.
pub struct InMemoryApprovalRuntime {
    requests: Mutex<HashMap<String, ApprovalRequest>>,
    waiters: Mutex<HashMap<String, oneshot::Sender<ApprovalResponse>>>,
}

impl Default for InMemoryApprovalRuntime {
    fn default() -> Self {
        Self::new()
    }
}

impl InMemoryApprovalRuntime {
    /// Create a new in-memory approval runtime
    pub fn new() -> Self {
        Self {
            requests: Mutex::new(HashMap::new()),
            waiters: Mutex::new(HashMap::new()),
        }
    }

    /// Get a copy of a request by ID (for inspection)
    pub fn get_request(&self, request_id: &str) -> Option<ApprovalRequest> {
        self.requests
            .lock()
            .ok()
            .and_then(|requests| requests.get(request_id).cloned())
    }

    /// List all pending requests
    pub fn list_pending(&self) -> Vec<ApprovalRequest> {
        self.requests
            .lock()
            .map(|requests| {
                requests
                    .values()
                    .filter(|r| r.status == ApprovalStatus::Pending)
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Cancel a pending request
    pub fn cancel(&self, request_id: &str) -> Result<(), AgentError> {
        let mut requests = self.requests.lock().map_err(|_| {
            AgentError::ToolExecutionFailed(
                "cancel".to_string(),
                "Failed to lock requests".to_string(),
            )
        })?;

        if let Some(request) = requests.get_mut(request_id) {
            if request.status == ApprovalStatus::Pending {
                request.status = ApprovalStatus::Cancelled;
                drop(requests); // Drop early to avoid deadlock

                // Notify any waiters
                let mut waiters = self.waiters.lock().map_err(|_| {
                    AgentError::ToolExecutionFailed(
                        "cancel".to_string(),
                        "Failed to lock waiters".to_string(),
                    )
                })?;
                if let Some(sender) = waiters.remove(request_id) {
                    let _ = sender.send(ApprovalResponse::Reject);
                }
                Ok(())
            } else {
                Err(AgentError::ToolExecutionFailed(
                    "cancel".to_string(),
                    format!("Request {} is not pending", request_id),
                ))
            }
        } else {
            Err(AgentError::ToolExecutionFailed(
                "cancel".to_string(),
                format!("Request {} not found", request_id),
            ))
        }
    }
}

#[async_trait]
impl ApprovalRuntime for InMemoryApprovalRuntime {
    async fn create_request(
        &self,
        tool_call: &ToolCall,
        source: ApprovalSource,
        description: Option<String>,
    ) -> Result<String, AgentError> {
        let request = ApprovalRequest::new(tool_call.clone(), source, description);
        let request_id = request.id.clone();

        let mut requests = self.requests.lock().map_err(|_| {
            AgentError::ToolExecutionFailed(
                "create_request".to_string(),
                "Failed to lock requests".to_string(),
            )
        })?;
        requests.insert(request_id.clone(), request);

        Ok(request_id)
    }

    async fn wait_for_response(&self, request_id: &str) -> Result<ApprovalResponse, AgentError> {
        // Check if already resolved
        {
            let requests = self.requests.lock().map_err(|_| {
                AgentError::ToolExecutionFailed(
                    "wait_for_response".to_string(),
                    "Failed to lock requests".to_string(),
                )
            })?;

            if let Some(request) = requests.get(request_id) {
                match request.status {
                    ApprovalStatus::Resolved => {
                        // This shouldn't happen normally as we resolve via the channel
                        return Ok(ApprovalResponse::Approve);
                    }
                    ApprovalStatus::Cancelled => {
                        return Err(AgentError::ToolExecutionFailed(
                            "wait_for_response".to_string(),
                            format!("Request {} was cancelled", request_id),
                        ));
                    }
                    ApprovalStatus::Pending => {
                        // Continue to wait
                    }
                }
            } else {
                return Err(AgentError::ToolExecutionFailed(
                    "wait_for_response".to_string(),
                    format!("Request {} not found", request_id),
                ));
            }
        }

        // Create a oneshot channel for the response
        let (tx, rx) = oneshot::channel();

        {
            let mut waiters = self.waiters.lock().map_err(|_| {
                AgentError::ToolExecutionFailed(
                    "wait_for_response".to_string(),
                    "Failed to lock waiters".to_string(),
                )
            })?;
            waiters.insert(request_id.to_string(), tx);
        }

        // Wait for the response
        match rx.await {
            Ok(response) => Ok(response),
            Err(_) => Err(AgentError::ToolExecutionFailed(
                "wait_for_response".to_string(),
                "Response channel closed".to_string(),
            )),
        }
    }

    async fn resolve(
        &self,
        request_id: &str,
        response: ApprovalResponse,
    ) -> Result<(), AgentError> {
        // Update the request status
        {
            let mut requests = self.requests.lock().map_err(|_| {
                AgentError::ToolExecutionFailed(
                    "resolve".to_string(),
                    "Failed to lock requests".to_string(),
                )
            })?;

            if let Some(request) = requests.get_mut(request_id) {
                if request.status != ApprovalStatus::Pending {
                    return Err(AgentError::ToolExecutionFailed(
                        "resolve".to_string(),
                        format!("Request {} is not pending", request_id),
                    ));
                }
                request.status = ApprovalStatus::Resolved;
            } else {
                return Err(AgentError::ToolExecutionFailed(
                    "resolve".to_string(),
                    format!("Request {} not found", request_id),
                ));
            }
        }

        // Notify any waiters
        let mut waiters = self.waiters.lock().map_err(|_| {
            AgentError::ToolExecutionFailed(
                "resolve".to_string(),
                "Failed to lock waiters".to_string(),
            )
        })?;

        if let Some(sender) = waiters.remove(request_id) {
            let _ = sender.send(response);
        }

        Ok(())
    }
}

impl Clone for ApprovalRequest {
    fn clone(&self) -> Self {
        Self {
            id: self.id.clone(),
            tool_call: self.tool_call.clone(),
            source: self.source.clone(),
            status: self.status,
            created_at: self.created_at,
            description: self.description.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{FunctionCall, ToolCall};

    fn create_test_tool_call() -> ToolCall {
        ToolCall {
            id: "test-123".to_string(),
            call_type: "function".to_string(),
            function: FunctionCall {
                name: "test_tool".to_string(),
                arguments: r#"{"param": "value"}"#.to_string(),
            },
        }
    }

    #[tokio::test]
    async fn test_create_request() {
        let runtime = InMemoryApprovalRuntime::new();
        let tool_call = create_test_tool_call();
        let source = ApprovalSource::ForegroundTurn {
            turn_id: "turn-123".to_string(),
        };

        let request_id = runtime
            .create_request(&tool_call, source, None)
            .await
            .expect("Failed to create request");

        assert!(!request_id.is_empty());

        let request = runtime.get_request(&request_id).expect("Request not found");
        assert_eq!(request.status, ApprovalStatus::Pending);
        assert_eq!(request.tool_call.id, tool_call.id);
    }

    #[tokio::test]
    async fn test_approve() {
        let runtime = InMemoryApprovalRuntime::new();
        let tool_call = create_test_tool_call();
        let source = ApprovalSource::ForegroundTurn {
            turn_id: "turn-123".to_string(),
        };

        let request_id = runtime
            .create_request(&tool_call, source, None)
            .await
            .expect("Failed to create request");

        // Resolve the request
        runtime
            .resolve(&request_id, ApprovalResponse::Approve)
            .await
            .expect("Failed to resolve");

        // Check the request status
        let request = runtime.get_request(&request_id).expect("Request not found");
        assert_eq!(request.status, ApprovalStatus::Resolved);
    }

    #[tokio::test]
    async fn test_reject() {
        let runtime = InMemoryApprovalRuntime::new();
        let tool_call = create_test_tool_call();
        let source = ApprovalSource::BackgroundAgent {
            task_id: "task-456".to_string(),
            agent_id: "agent-789".to_string(),
        };

        let request_id = runtime
            .create_request(&tool_call, source, None)
            .await
            .expect("Failed to create request");

        // Resolve with reject
        runtime
            .resolve(&request_id, ApprovalResponse::Reject)
            .await
            .expect("Failed to resolve");

        // Verify we can list pending and it's empty
        let pending = runtime.list_pending();
        assert!(pending.is_empty());
    }

    #[tokio::test]
    async fn test_yolo_mode_auto_approve() {
        let inner = InMemoryApprovalRuntime::new();
        let runtime = ModeAwareApprovalRuntime::new(inner, ApprovalMode::Yolo);
        let tool_call = create_test_tool_call();
        let source = ApprovalSource::ForegroundTurn {
            turn_id: "turn-123".to_string(),
        };

        let request_id = runtime
            .create_request(&tool_call, source, None)
            .await
            .expect("Failed to create request");

        // In YOLO mode, wait_for_response should immediately return Approve
        let response = runtime
            .wait_for_response(&request_id)
            .await
            .expect("Failed to wait for response");

        assert_eq!(response, ApprovalResponse::Approve);
    }

    #[tokio::test]
    async fn test_wait_and_resolve() {
        let runtime = std::sync::Arc::new(InMemoryApprovalRuntime::new());
        let tool_call = create_test_tool_call();
        let source = ApprovalSource::ForegroundTurn {
            turn_id: "turn-123".to_string(),
        };

        let request_id = runtime
            .create_request(&tool_call, source, None)
            .await
            .expect("Failed to create request");

        let runtime_clone = runtime.clone();
        let request_id_clone = request_id.clone();

        // Spawn a task that will resolve after a short delay
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            runtime_clone
                .resolve(&request_id_clone, ApprovalResponse::Approve)
                .await
                .expect("Failed to resolve");
        });

        // Wait for the response
        let response = runtime
            .wait_for_response(&request_id)
            .await
            .expect("Failed to wait for response");

        assert_eq!(response, ApprovalResponse::Approve);
    }

    #[tokio::test]
    async fn test_cancel() {
        let runtime = InMemoryApprovalRuntime::new();
        let tool_call = create_test_tool_call();
        let source = ApprovalSource::ForegroundTurn {
            turn_id: "turn-123".to_string(),
        };

        let request_id = runtime
            .create_request(&tool_call, source, None)
            .await
            .expect("Failed to create request");

        runtime.cancel(&request_id).expect("Failed to cancel");

        let request = runtime.get_request(&request_id).expect("Request not found");
        assert_eq!(request.status, ApprovalStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_list_pending() {
        let runtime = InMemoryApprovalRuntime::new();
        let tool_call = create_test_tool_call();

        // Create multiple requests
        for i in 0..3 {
            let source = ApprovalSource::ForegroundTurn {
                turn_id: format!("turn-{}", i),
            };
            runtime
                .create_request(&tool_call, source, None)
                .await
                .expect("Failed to create request");
        }

        let pending = runtime.list_pending();
        assert_eq!(pending.len(), 3);

        // Resolve one
        let request_id = pending[0].id.clone();
        runtime
            .resolve(&request_id, ApprovalResponse::Approve)
            .await
            .expect("Failed to resolve");

        let pending = runtime.list_pending();
        assert_eq!(pending.len(), 2);
    }

    #[tokio::test]
    async fn test_approve_for_session() {
        let inner = InMemoryApprovalRuntime::new();
        let runtime = ModeAwareApprovalRuntime::new(inner, ApprovalMode::Interactive);
        let tool_call = create_test_tool_call();
        let source = ApprovalSource::ForegroundTurn {
            turn_id: "turn-123".to_string(),
        };

        // First request should be pending
        let request_id1 = runtime
            .create_request(&tool_call, source.clone(), None)
            .await
            .expect("Failed to create request");

        let request1 = runtime
            .inner()
            .get_request(&request_id1)
            .expect("Request not found");
        assert_eq!(request1.status, ApprovalStatus::Pending);

        // Resolve with ApproveForSession
        runtime
            .resolve(&request_id1, ApprovalResponse::ApproveForSession)
            .await
            .expect("Failed to resolve");

        // Second request for same session should be auto-approved
        let request_id2 = runtime
            .create_request(&tool_call, source.clone(), None)
            .await
            .expect("Failed to create request");

        let request2 = runtime
            .inner()
            .get_request(&request_id2)
            .expect("Request not found");
        assert_eq!(request2.status, ApprovalStatus::Resolved);

        // wait_for_response should return Approve immediately
        let response = runtime
            .wait_for_response(&request_id2)
            .await
            .expect("Failed to wait for response");
        assert_eq!(response, ApprovalResponse::Approve);
    }

    #[tokio::test]
    async fn test_auto_approved_wait_for_response() {
        let inner = InMemoryApprovalRuntime::new();
        let runtime = ModeAwareApprovalRuntime::new(inner, ApprovalMode::Interactive);
        let tool_call = create_test_tool_call();
        let source = ApprovalSource::ForegroundTurn {
            turn_id: "turn-123".to_string(),
        };

        // First request
        let request_id1 = runtime
            .create_request(&tool_call, source.clone(), None)
            .await
            .expect("Failed to create request");

        // Approve for session
        runtime
            .resolve(&request_id1, ApprovalResponse::ApproveForSession)
            .await
            .expect("Failed to resolve");

        // Second request - wait_for_response should return immediately without blocking
        let request_id2 = runtime
            .create_request(&tool_call, source.clone(), None)
            .await
            .expect("Failed to create request");

        let start = Instant::now();
        let response = runtime
            .wait_for_response(&request_id2)
            .await
            .expect("Failed to wait for response");
        let elapsed = start.elapsed();

        assert_eq!(response, ApprovalResponse::Approve);
        assert!(
            elapsed < std::time::Duration::from_millis(50),
            "wait_for_response should return immediately"
        );
    }
}

pub mod rules;
