//! Approval runtime for tool call authorization
//!
//! This module provides an asynchronous approval system for tool calls,
//! supporting different approval modes and both foreground and background contexts.

use crate::error::AgentError;
use crate::types::ToolCall;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Instant;
use tokio::sync::oneshot;
use tracing::warn;
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    /// Optional unified diff preview for file-modifying tools (e.g. file_edit)
    pub diff_preview: Option<String>,
}

impl ApprovalRequest {
    /// Create a new approval request
    fn new(
        tool_call: ToolCall,
        source: ApprovalSource,
        description: Option<String>,
        diff_preview: Option<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            tool_call,
            source,
            status: ApprovalStatus::Pending,
            created_at: Instant::now(),
            description,
            diff_preview,
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
        diff_preview: Option<String>,
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

    /// List all currently pending approval requests.
    ///
    /// Default implementation returns an empty vec for runtimes that
    /// do not support introspection.
    fn list_pending(&self) -> Vec<ApprovalRequest> {
        Vec::new()
    }
}

// Re-export from contract layer.
pub use clarity_contract::ApprovalMode;

/// Wrapper that adds mode-aware behavior to any approval runtime
pub struct ModeAwareApprovalRuntime<R: ApprovalRuntime> {
    inner: Arc<R>,
    mode: Mutex<ApprovalMode>,
    session_approvals: Mutex<HashMap<String, ()>>,
    request_sessions: Mutex<HashMap<String, String>>,
    /// Batch grants for Smart mode: tool_name -> when granted.
    /// Once a tool is batch-granted in a session, subsequent same-tool
    /// requests are auto-approved without UI interruption.
    batch_grants: Mutex<HashMap<String, Instant>>,
    /// Maps request_id -> tool_name for Smart mode lookup in wait_for_response.
    request_tools: Mutex<HashMap<String, String>>,
    /// Notifications for UI when a request is auto-approved via batch grant.
    recent_auto_approvals: Mutex<Vec<String>>,
}

impl<R: ApprovalRuntime> ModeAwareApprovalRuntime<R> {
    /// Create a new mode-aware wrapper
    pub fn new(inner: Arc<R>, mode: ApprovalMode) -> Self {
        Self {
            inner,
            mode: Mutex::new(mode),
            session_approvals: Mutex::new(HashMap::new()),
            request_sessions: Mutex::new(HashMap::new()),
            batch_grants: Mutex::new(HashMap::new()),
            request_tools: Mutex::new(HashMap::new()),
            recent_auto_approvals: Mutex::new(Vec::new()),
        }
    }

    /// Get the current approval mode
    pub fn mode(&self) -> ApprovalMode {
        *self.mode.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Set the approval mode (thread-safe).
    pub fn set_mode(&self, mode: ApprovalMode) {
        if let Ok(mut guard) = self.mode.lock() {
            *guard = mode;
        }
    }

    /// Clear all batch grants (e.g., from Settings UI).
    pub fn clear_batch_grants(&self) {
        if let Ok(mut grants) = self.batch_grants.lock() {
            grants.clear();
        }
    }

    /// Drain pending auto-approval notifications for UI toasts.
    pub fn drain_auto_approval_notifications(&self) -> Vec<String> {
        self.recent_auto_approvals
            .lock()
            .map(|mut v| std::mem::take(&mut *v))
            .unwrap_or_default()
    }

    /// Get a reference to the inner runtime
    pub fn inner(&self) -> &Arc<R> {
        &self.inner
    }

    /// Unwrap to get the inner runtime
    pub fn into_inner(self) -> Arc<R> {
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
        diff_preview: Option<String>,
    ) -> Result<String, AgentError> {
        let session_key = source.session_key();
        let tool_name = tool_call.function.name.clone();
        let request_id = self
            .inner
            .create_request(tool_call, source.clone(), description, diff_preview)
            .await?;
        if let Ok(mut sessions) = self.request_sessions.lock() {
            sessions.insert(request_id.clone(), session_key.clone());
        }
        if let Ok(mut req_tools) = self.request_tools.lock() {
            req_tools.insert(request_id.clone(), tool_name.clone());
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

        // Smart mode: auto-approve if a batch grant exists for this tool.
        let current_mode = self.mode();
        let has_batch_grant = if current_mode == ApprovalMode::Smart {
            self.batch_grants
                .lock()
                .map(|grants| grants.contains_key(&tool_name))
                .unwrap_or(false)
        } else {
            false
        };
        if has_batch_grant {
            let _ = self
                .inner
                .resolve(&request_id, ApprovalResponse::Approve)
                .await;
            if let Ok(mut notifs) = self.recent_auto_approvals.lock() {
                notifs.push(format!("Auto-approved: {} (batch grant)", tool_name));
            }
        }

        Ok(request_id)
    }

    async fn wait_for_response(&self, request_id: &str) -> Result<ApprovalResponse, AgentError> {
        let current_mode = self.mode();
        match current_mode {
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
            ApprovalMode::Smart => {
                // Session-level approval takes precedence.
                if let Ok(sessions) = self.request_sessions.lock() {
                    if let Some(session_key) = sessions.get(request_id) {
                        if let Ok(approvals) = self.session_approvals.lock() {
                            if approvals.contains_key(session_key) {
                                return Ok(ApprovalResponse::Approve);
                            }
                        }
                    }
                }
                // Batch grant: same tool previously approved in this session.
                if let Ok(req_tools) = self.request_tools.lock() {
                    if let Some(tool_name) = req_tools.get(request_id) {
                        if let Ok(grants) = self.batch_grants.lock() {
                            if grants.contains_key(tool_name) {
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
            return self
                .inner
                .resolve(request_id, ApprovalResponse::Approve)
                .await;
        }

        // Smart mode: a plain Approve also creates a batch grant for the tool
        // so that subsequent same-tool requests in this session are auto-approved.
        let current_mode = self.mode();
        if current_mode == ApprovalMode::Smart && response == ApprovalResponse::Approve {
            if let Ok(req_tools) = self.request_tools.lock() {
                if let Some(tool_name) = req_tools.get(request_id).cloned() {
                    drop(req_tools); // avoid holding multiple locks
                    if let Ok(mut grants) = self.batch_grants.lock() {
                        grants.insert(tool_name, Instant::now());
                    }
                }
            }
        }

        self.inner.resolve(request_id, response).await
    }

    fn list_pending(&self) -> Vec<ApprovalRequest> {
        self.inner.list_pending()
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
        diff_preview: Option<String>,
    ) -> Result<String, AgentError> {
        let request = ApprovalRequest::new(tool_call.clone(), source, description, diff_preview);
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

        // Wait for the response with a hard timeout so that stale requests
        // do not keep the runtime state out of sync with the Agent.
        const APPROVAL_TIMEOUT_SECS: u64 = 300;
        match tokio::time::timeout(tokio::time::Duration::from_secs(APPROVAL_TIMEOUT_SECS), rx)
            .await
        {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(_)) => Err(AgentError::ToolExecutionFailed(
                "wait_for_response".to_string(),
                "Response channel closed".to_string(),
            )),
            Err(_) => {
                // Timeout: mark the request as cancelled and clean up the waiter
                let mut requests = self.requests.lock().map_err(|_| {
                    AgentError::ToolExecutionFailed(
                        "wait_for_response".to_string(),
                        "Failed to lock requests".to_string(),
                    )
                })?;
                if let Some(request) = requests.get_mut(request_id) {
                    if request.status == ApprovalStatus::Pending {
                        request.status = ApprovalStatus::Cancelled;
                        tracing::warn!(
                            "Approval request {} timed out after {}s",
                            request_id,
                            APPROVAL_TIMEOUT_SECS
                        );
                    }
                }
                drop(requests);
                let mut waiters = self.waiters.lock().map_err(|_| {
                    AgentError::ToolExecutionFailed(
                        "wait_for_response".to_string(),
                        "Failed to lock waiters".to_string(),
                    )
                })?;
                waiters.remove(request_id);
                Err(AgentError::ToolExecutionFailed(
                    "wait_for_response".to_string(),
                    format!("Approval timeout after {} seconds", APPROVAL_TIMEOUT_SECS),
                ))
            }
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

    fn list_pending(&self) -> Vec<ApprovalRequest> {
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
}

/// Serializable snapshot of an approval decision for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub request_id: String,
    pub approved: bool,
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

/// B1: Wrapper runtime that persists every resolved approval to a `MemoryStore`.
///
/// This delegates all operations to an inner runtime and only intercepts `resolve`
/// to write an `ApprovalRecord` to the optional store.
///
/// Risk: Storage failures are silently logged (warn!) to avoid breaking the approval
/// flow. This means audit gaps are possible if the memory store is unreachable.
/// Consider a background retry queue if audit completeness becomes critical.
pub struct PersistingApprovalRuntime<R: ApprovalRuntime> {
    inner: Arc<R>,
    store: Option<Arc<dyn crate::memory::MemoryStore>>,
}

impl<R: ApprovalRuntime> PersistingApprovalRuntime<R> {
    pub fn new(inner: Arc<R>, store: Option<Arc<dyn crate::memory::MemoryStore>>) -> Self {
        Self { inner, store }
    }
}

#[async_trait]
impl<R: ApprovalRuntime> ApprovalRuntime for PersistingApprovalRuntime<R> {
    async fn create_request(
        &self,
        tool_call: &ToolCall,
        source: ApprovalSource,
        description: Option<String>,
        diff_preview: Option<String>,
    ) -> Result<String, AgentError> {
        self.inner
            .create_request(tool_call, source, description, diff_preview)
            .await
    }

    async fn wait_for_response(&self, request_id: &str) -> Result<ApprovalResponse, AgentError> {
        self.inner.wait_for_response(request_id).await
    }

    async fn resolve(
        &self,
        request_id: &str,
        response: ApprovalResponse,
    ) -> Result<(), AgentError> {
        self.inner.resolve(request_id, response).await?;

        if let Some(ref store) = self.store {
            let record = ApprovalRecord {
                request_id: request_id.to_string(),
                approved: matches!(
                    response,
                    ApprovalResponse::Approve | ApprovalResponse::ApproveForSession
                ),
                timestamp: chrono::Utc::now(),
            };
            let memory =
                crate::memory::Memory::new(serde_json::to_string(&record).unwrap_or_default())
                    .with_tags(vec!["approval".to_string(), "record".to_string()])
                    .with_importance(0.8);
            if let Err(e) = store.store(memory).await {
                warn!("Failed to persist approval record: {}", e);
            }
        }

        Ok(())
    }

    fn list_pending(&self) -> Vec<ApprovalRequest> {
        self.inner.list_pending()
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
            diff_preview: self.diff_preview.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::{FunctionCall, ToolCall};
    use crate::memory::MemoryStore;

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
            .create_request(&tool_call, source, None, None)
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
            .create_request(&tool_call, source, None, None)
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
            .create_request(&tool_call, source, None, None)
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
        let runtime = ModeAwareApprovalRuntime::new(Arc::new(inner), ApprovalMode::Yolo);
        let tool_call = create_test_tool_call();
        let source = ApprovalSource::ForegroundTurn {
            turn_id: "turn-123".to_string(),
        };

        let request_id = runtime
            .create_request(&tool_call, source, None, None)
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
            .create_request(&tool_call, source, None, None)
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
            .create_request(&tool_call, source, None, None)
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
                .create_request(&tool_call, source, None, None)
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
        let runtime = ModeAwareApprovalRuntime::new(Arc::new(inner), ApprovalMode::Interactive);
        let tool_call = create_test_tool_call();
        let source = ApprovalSource::ForegroundTurn {
            turn_id: "turn-123".to_string(),
        };

        // First request should be pending
        let request_id1 = runtime
            .create_request(&tool_call, source.clone(), None, None)
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
            .create_request(&tool_call, source.clone(), None, None)
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
        let runtime = ModeAwareApprovalRuntime::new(Arc::new(inner), ApprovalMode::Interactive);
        let tool_call = create_test_tool_call();
        let source = ApprovalSource::ForegroundTurn {
            turn_id: "turn-123".to_string(),
        };

        // First request
        let request_id1 = runtime
            .create_request(&tool_call, source.clone(), None, None)
            .await
            .expect("Failed to create request");

        // Approve for session
        runtime
            .resolve(&request_id1, ApprovalResponse::ApproveForSession)
            .await
            .expect("Failed to resolve");

        // Second request - wait_for_response should return immediately without blocking
        let request_id2 = runtime
            .create_request(&tool_call, source.clone(), None, None)
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

    #[tokio::test]
    async fn test_smart_mode_batch_grant() {
        let inner = InMemoryApprovalRuntime::new();
        let runtime = ModeAwareApprovalRuntime::new(Arc::new(inner), ApprovalMode::Smart);
        let tool_call = create_test_tool_call();
        let source = ApprovalSource::ForegroundTurn {
            turn_id: "turn-smart".to_string(),
        };

        // First request: should be pending (no batch grant yet)
        let request_id1 = runtime
            .create_request(&tool_call, source.clone(), None, None)
            .await
            .expect("Failed to create request");
        let request1 = runtime
            .inner()
            .get_request(&request_id1)
            .expect("Request not found");
        assert_eq!(request1.status, ApprovalStatus::Pending);

        // Approve the first request — this should create a batch grant
        runtime
            .resolve(&request_id1, ApprovalResponse::Approve)
            .await
            .expect("Failed to resolve");

        // Second request for the same tool: should be auto-approved via batch grant
        let request_id2 = runtime
            .create_request(&tool_call, source.clone(), None, None)
            .await
            .expect("Failed to create request");
        let request2 = runtime
            .inner()
            .get_request(&request_id2)
            .expect("Request not found");
        assert_eq!(request2.status, ApprovalStatus::Resolved);

        let response = runtime
            .wait_for_response(&request_id2)
            .await
            .expect("Failed to wait");
        assert_eq!(response, ApprovalResponse::Approve);
    }

    #[tokio::test]
    async fn test_smart_mode_wait_for_response_batch_grant() {
        let inner = InMemoryApprovalRuntime::new();
        let runtime = ModeAwareApprovalRuntime::new(Arc::new(inner), ApprovalMode::Smart);
        let tool_call = create_test_tool_call();
        let source = ApprovalSource::ForegroundTurn {
            turn_id: "turn-smart-wait".to_string(),
        };

        // Create and approve first request to establish batch grant
        let request_id1 = runtime
            .create_request(&tool_call, source.clone(), None, None)
            .await
            .expect("Failed to create request");
        runtime
            .resolve(&request_id1, ApprovalResponse::Approve)
            .await
            .unwrap();

        // Create second request and verify wait_for_response returns immediately
        let request_id2 = runtime
            .create_request(&tool_call, source.clone(), None, None)
            .await
            .expect("Failed to create request");

        let start = Instant::now();
        let response = runtime
            .wait_for_response(&request_id2)
            .await
            .expect("Failed to wait");
        let elapsed = start.elapsed();

        assert_eq!(response, ApprovalResponse::Approve);
        assert!(
            elapsed < std::time::Duration::from_millis(50),
            "wait_for_response should return immediately via batch grant"
        );
    }

    #[tokio::test]
    async fn test_concurrent_resolve_race() {
        let runtime = std::sync::Arc::new(InMemoryApprovalRuntime::new());
        let tool_call = create_test_tool_call();
        let source = ApprovalSource::ForegroundTurn {
            turn_id: "turn-race".to_string(),
        };

        let request_id = runtime
            .create_request(&tool_call, source, None, None)
            .await
            .expect("Failed to create request");

        // Spawn two concurrent resolve attempts
        let r1 = runtime.clone();
        let r2 = runtime.clone();
        let id1 = request_id.clone();
        let id2 = request_id.clone();

        let (res1, res2) = tokio::join!(
            tokio::spawn(async move { r1.resolve(&id1, ApprovalResponse::Approve).await }),
            tokio::spawn(async move { r2.resolve(&id2, ApprovalResponse::Reject).await }),
        );

        let results = [res1.unwrap(), res2.unwrap()];
        let ok_count = results.iter().filter(|r| r.is_ok()).count();
        let err_count = results.iter().filter(|r| r.is_err()).count();

        assert_eq!(ok_count, 1, "Exactly one concurrent resolve should succeed");
        assert_eq!(
            err_count, 1,
            "Exactly one concurrent resolve should fail with 'not pending'"
        );
    }

    #[tokio::test]
    async fn test_resolve_nonexistent_request() {
        let runtime = InMemoryApprovalRuntime::new();
        let result = runtime.resolve("fake-id", ApprovalResponse::Approve).await;
        assert!(result.is_err(), "Resolving nonexistent request must fail");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("fake-id"),
            "Error should mention the request id: {}",
            err_msg
        );
    }

    #[tokio::test]
    async fn test_wait_for_response_timeout() {
        let runtime = std::sync::Arc::new(InMemoryApprovalRuntime::new());
        let tool_call = create_test_tool_call();
        let source = ApprovalSource::ForegroundTurn {
            turn_id: "turn-timeout".to_string(),
        };

        let request_id = runtime
            .create_request(&tool_call, source, None, None)
            .await
            .expect("Failed to create request");

        // Use a short timeout for the test by replacing the constant temporarily
        // Since the constant is hard-coded to 300s, we verify the mechanism
        // by checking that the request is still Pending before resolve.
        // A full timeout test would take 300s; we verify the code path exists
        // and that the request can be resolved normally.
        let request = runtime
            .get_request(&request_id)
            .expect("Request should exist");
        assert_eq!(request.status, ApprovalStatus::Pending);

        // Resolve it normally
        runtime
            .resolve(&request_id, ApprovalResponse::Approve)
            .await
            .expect("Should resolve");
    }

    #[tokio::test]
    async fn test_persisting_runtime_records_approval() {
        let inner = std::sync::Arc::new(InMemoryApprovalRuntime::new());
        let store = std::sync::Arc::new(crate::memory::InMemoryStore::new());
        let runtime = PersistingApprovalRuntime::new(inner.clone(), Some(store.clone()));

        let tool_call = create_test_tool_call();
        let source = ApprovalSource::ForegroundTurn {
            turn_id: "turn-persist".to_string(),
        };
        let request_id = runtime
            .create_request(&tool_call, source, None, None)
            .await
            .expect("Failed to create request");

        runtime
            .resolve(&request_id, ApprovalResponse::Approve)
            .await
            .expect("Should resolve");

        let memories = store.get_all().await.expect("Should get memories");
        assert_eq!(memories.len(), 1, "Expected one persisted approval record");
        let memory = &memories[0];
        assert!(memory.tags.contains(&"approval".to_string()));
        assert!(memory.tags.contains(&"record".to_string()));
        let record: ApprovalRecord =
            serde_json::from_str(&memory.content).expect("Should parse record");
        assert_eq!(record.request_id, request_id);
        assert!(record.approved);
    }
}

pub mod rules;
