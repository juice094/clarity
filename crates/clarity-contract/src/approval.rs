//! Approval runtime contract.
//!
//! The concrete approval runtimes live in `clarity-core::approval`.
//! This module only defines the shared interface and request/response types
//! so that `clarity-subagents` can reference them without pulling in core.

use crate::ToolCall;
use crate::error::AgentError;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Instant;

/// Re-export the approval mode from the tool module so that consumers have a
/// single place to import both mode and runtime.
pub use crate::tool::ApprovalMode;

/// The source of an approval request.
#[derive(Debug, Clone)]
pub enum ApprovalSource {
    /// Foreground turn context.
    ForegroundTurn {
        /// Turn identifier.
        turn_id: String,
        /// User identifier for approval scoping.
        user_id: Option<String>,
    },
    /// Background agent context.
    BackgroundAgent {
        /// Task identifier.
        task_id: String,
        /// Agent identifier.
        agent_id: String,
    },
}

impl ApprovalSource {
    /// Returns a stable session key for this approval source.
    pub fn session_key(&self) -> String {
        match self {
            ApprovalSource::ForegroundTurn { turn_id, .. } => format!("turn:{}", turn_id),
            ApprovalSource::BackgroundAgent { task_id, agent_id } => {
                format!("agent:{}:{}", agent_id, task_id)
            }
        }
    }
}

/// Response to an approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApprovalResponse {
    /// Approve this request.
    Approve,
    /// Approve and remember for this session.
    ApproveForSession,
    /// Reject this request.
    Reject,
}

/// Status of an approval request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApprovalStatus {
    /// Waiting for approval.
    Pending,
    /// Has been resolved.
    Resolved,
    /// Was cancelled.
    Cancelled,
}

/// An approval request record.
#[derive(Debug)]
pub struct ApprovalRequest {
    /// Unique request ID.
    pub id: String,
    /// The tool call being approved.
    pub tool_call: ToolCall,
    /// Source of the request.
    pub source: ApprovalSource,
    /// Current status.
    pub status: ApprovalStatus,
    /// When the request was created.
    pub created_at: Instant,
    /// Optional description highlighting sensitive nature or other concerns.
    pub description: Option<String>,
    /// Optional unified diff preview for file-modifying tools.
    pub diff_preview: Option<String>,
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

impl ApprovalRequest {
    /// Create a new approval request.
    pub fn new(
        tool_call: ToolCall,
        source: ApprovalSource,
        description: Option<String>,
        diff_preview: Option<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            tool_call,
            source,
            status: ApprovalStatus::Pending,
            created_at: Instant::now(),
            description,
            diff_preview,
        }
    }
}

/// Core trait for approval runtimes.
#[async_trait]
pub trait ApprovalRuntime: Send + Sync {
    /// Create an approval request and return its ID.
    async fn create_request(
        &self,
        tool_call: &ToolCall,
        source: ApprovalSource,
        description: Option<String>,
        diff_preview: Option<String>,
    ) -> Result<String, AgentError>;

    /// Wait for a response to an approval request.
    async fn wait_for_response(&self, request_id: &str) -> Result<ApprovalResponse, AgentError>;

    /// Resolve an approval request (typically called by UI).
    async fn resolve(&self, request_id: &str, response: ApprovalResponse)
    -> Result<(), AgentError>;

    /// List all currently pending approval requests.
    fn list_pending(&self) -> Vec<ApprovalRequest> {
        Vec::new()
    }
}
